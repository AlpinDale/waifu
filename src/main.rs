mod auth;
mod cache;
mod config;
mod error;
mod handlers;
mod limiter;
mod middleware;
mod models;
mod store;

use crate::cache::ImageCache;
use crate::limiter::ApiKeyRateLimiter;
use crate::models::{AddImageRequest, GenerateApiKeyRequest, RemoveApiKeyRequest};
use crate::store::ImageStore;
use anyhow::Result;
use auth::Auth;
use chrono;
use middleware::{add_request_id_header, with_request_id};
use serde_json;
use std::net::SocketAddr;
use std::path::PathBuf;
use time::macros::format_description;
use time::Duration;
use tracing::info;
use warp::cors::Cors;
use warp::http::HeaderMap;
use warp::multipart::form;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("waifu=debug,warp=info")
        .with_timer(tracing_subscriber::fmt::time::LocalTime::new(
            format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
        ))
        .init();

    info!("Starting waifu server...");

    let config = config::Config::from_env()?;

    let images_dir = PathBuf::from("images");

    info!("Initializing image store...");
    let store = store::ImageStore::new("images.db", images_dir.clone(), &config)?;

    let rate_limiter = ApiKeyRateLimiter::new(
        store.clone(),
        config.rate_limit_requests,
        Duration::seconds(config.rate_limit_window_secs as i64),
    );

    let cache = ImageCache::new(config.cache_size, config.cache_ttl());

    let auth = Auth::new(config.admin_key, store.clone(), rate_limiter);

    let store = warp::any().map(move || store.clone());
    let cache = warp::any().map(move || cache.clone());

    fn cors() -> Cors {
        warp::cors()
            .allow_any_origin()
            .allow_headers(vec![
                "Authorization",
                "Content-Type",
                "User-Agent",
                "Sec-Fetch-Mode",
                "Referer",
                "Origin",
                "Access-Control-Request-Method",
                "Access-Control-Request-Headers",
            ])
            .allow_methods(vec!["GET", "POST", "DELETE"])
            .max_age(3600)
            .build()
    }

    let health = warp::path("health").and(warp::get()).map(|| {
        warp::reply::json(&serde_json::json!({
            "status": "ok",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    });

    let random_get = warp::path("random")
        .and(warp::get())
        .and(store.clone())
        .and(cache.clone())
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(warp::filters::header::headers_cloned())
        .and(auth.require_auth_info())
        .and_then(handlers::get_random_image_handler);

    let random_post = warp::path("random")
        .and(warp::post())
        .and(store.clone())
        .and(cache.clone())
        .and(warp::filters::header::headers_cloned())
        .and(auth.require_auth_info())
        .and(warp::body::json())
        .and_then(handlers::batch_random_images_handler);

    let add_image = warp::path("image")
        .and(warp::post())
        .and(warp::body::json())
        .and(store.clone())
        .and(auth.require_auth())
        .map(|body, store, ()| (store, body))
        .and_then(|args: (ImageStore, AddImageRequest)| async move {
            handlers::add_image_handler(args.0, args.1).await
        });

    let batch_add_images = warp::path("images")
        .and(warp::post())
        .and(store.clone())
        .and(warp::body::json())
        .and(auth.require_auth_info())
        .and_then(handlers::batch_add_images_handler);

    let remove_image = warp::path!("images" / String)
        .and(warp::delete())
        .and(store.clone())
        .and(auth.require_admin())
        .and_then(handlers::remove_image_handler);

    let remove_image_tags = warp::path!("images" / String / "tags")
        .and(warp::delete())
        .and(store.clone())
        .and(warp::body::json())
        .and(auth.require_admin())
        .and_then(handlers::remove_image_tags_handler);

    let add_image_tags = warp::path!("images" / String / "tags")
        .and(warp::post())
        .and(store.clone())
        .and(warp::body::json())
        .and(auth.require_admin())
        .and_then(handlers::add_image_tags_handler);

    let get_all_tags = warp::path("tags")
        .and(warp::get())
        .and(store.clone())
        .and(auth.require_auth())
        .and_then(handlers::get_all_tags_handler);

    let images = warp::path("images").and(warp::fs::dir("images"));

    let image = warp::path!("images" / String)
        .and(warp::get())
        .and(store.clone())
        .and(cache.clone())
        .and(warp::filters::header::headers_cloned())
        .and(auth.require_auth())
        .map(|filename, store, cache, headers, ()| (filename, store, cache, headers))
        .and_then(
            |args: (String, ImageStore, ImageCache, HeaderMap)| async move {
                handlers::get_image_by_filename_handler(args.0, args.1, args.2, args.3).await
            },
        );

    let api_key_routes = warp::path("api-keys")
        .and(warp::post())
        .and(store.clone())
        .and(warp::body::json())
        .and(auth.require_admin())
        .map(|store, body, ()| ((), store, body))
        .and_then(|args: ((), ImageStore, GenerateApiKeyRequest)| async move {
            handlers::generate_api_key_handler(args.0, args.1, args.2).await
        })
        .or(warp::path("api-keys")
            .and(warp::delete())
            .and(store.clone())
            .and(warp::body::json())
            .and(auth.require_admin())
            .map(|store, body, ()| ((), store, body))
            .and_then(|args: ((), ImageStore, RemoveApiKeyRequest)| async move {
                handlers::remove_api_key_handler(args.0, args.1, args.2).await
            }))
        .or(warp::path("api-keys")
            .and(warp::get())
            .and(store.clone())
            .and(auth.require_admin())
            .map(|store, ()| ((), store))
            .and_then(|args: ((), ImageStore)| async move {
                handlers::list_api_keys_handler(args.0, args.1).await
            }));

    let update_api_key = warp::path!("api-keys" / String)
        .and(warp::put())
        .and(auth.require_admin())
        .and(store.clone())
        .and(warp::body::json())
        .and_then(handlers::update_api_key_handler);

    let update_api_key_status = warp::path!("api-keys" / String / "status")
        .and(warp::patch())
        .and(auth.require_admin())
        .and(store.clone())
        .and(warp::body::json())
        .and_then(handlers::update_api_key_status_handler);

    let upload = warp::path("upload")
        .and(warp::post())
        .and(form().max_length(10 * 1024 * 1024)) // 10MB limit
        .and(store.clone())
        .and(auth.require_auth())
        .and_then(handlers::upload_image_handler);

    let api = health
        .or(random_get)
        .or(random_post)
        .or(add_image)
        .or(batch_add_images)
        .or(remove_image)
        .or(remove_image_tags)
        .or(add_image_tags)
        .or(get_all_tags)
        .or(images)
        .or(image)
        .or(api_key_routes)
        .or(update_api_key)
        .or(update_api_key_status)
        .or(upload)
        .or(warp::options()
            .and(warp::path::full())
            .map(|_| warp::reply()))
        .and(with_request_id())
        .map(|reply, request_id| add_request_id_header(reply, request_id))
        .recover(error::handle_rejection)
        .with(cors());

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    info!("Server started at http://{}:{}", config.host, config.port);
    warp::serve(api).run(addr).await;

    Ok(())
}
