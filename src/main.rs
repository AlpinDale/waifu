mod auth;
mod cache;
mod config;
mod error;
mod handlers;
mod limiter;
mod models;
mod store;

use crate::cache::ImageCache;
use crate::limiter::IpRateLimiter;
use crate::models::{AddImageRequest, GenerateApiKeyRequest, RemoveApiKeyRequest};
use crate::store::ImageStore;
use anyhow::Result;
use auth::Auth;
use std::net::SocketAddr;
use std::path::PathBuf;
use time::macros::format_description;
use tracing::info;
use warp::cors::Cors;
use warp::http::HeaderMap;
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

    let rate_limiter =
        IpRateLimiter::new(config.rate_limit_requests, config.rate_limit_window_secs);

    let cache = ImageCache::new(config.cache_size, config.cache_ttl());

    let images_dir = PathBuf::from("images");

    info!("Initializing image store...");
    let store = store::ImageStore::new(
        "images.db",
        images_dir.clone(),
        config.host.clone(),
        config.port,
        config.images_path.clone(),
    )?;

    let auth = Auth::new(config.admin_key, store.clone());

    let store = warp::any().map(move || store.clone());
    let cache = warp::any().map(move || cache.clone());
    let rate_limiter = warp::any().map(move || rate_limiter.clone());

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

    let random = warp::path("random")
        .and(warp::get())
        .and(store.clone())
        .and(cache.clone())
        .and(rate_limiter.clone())
        .and(warp::filters::header::headers_cloned())
        .and(auth.require_auth())
        .map(|store, cache, limiter, headers, ()| (store, cache, limiter, headers))
        .and_then(
            |args: (ImageStore, ImageCache, IpRateLimiter, HeaderMap)| async move {
                handlers::get_random_image_handler(args.0, args.1, args.2, args.3).await
            },
        );

    let add_image = warp::path("image")
        .and(warp::post())
        .and(store.clone())
        .and(warp::body::json())
        .and(auth.require_auth())
        .map(|store, body, ()| (store, body))
        .and_then(|args: (ImageStore, AddImageRequest)| async move {
            handlers::add_image_handler(args.0, args.1).await
        });

    let images = warp::path("images").and(warp::fs::dir("images"));

    let image = warp::path!("images" / String)
        .and(warp::get())
        .and(store.clone())
        .and(cache.clone())
        .and(rate_limiter.clone())
        .and(warp::filters::header::headers_cloned())
        .and(auth.require_auth())
        .map(|filename, store, cache, limiter, headers, ()| {
            (filename, store, cache, limiter, headers)
        })
        .and_then(
            |args: (String, ImageStore, ImageCache, IpRateLimiter, HeaderMap)| async move {
                handlers::get_image_by_filename_handler(args.0, args.1, args.2, args.3, args.4)
                    .await
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

    let routes = random
        .or(add_image)
        .or(images)
        .or(image)
        .or(api_key_routes)
        .or(warp::options()
            .and(warp::path::full())
            .map(|_| warp::reply()))
        .with(cors())
        .recover(error::handle_rejection);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    info!("Server started at http://{}:{}", config.host, config.port);
    warp::serve(routes).run(addr).await;

    Ok(())
}
