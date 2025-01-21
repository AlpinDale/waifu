mod cache;
mod config;
mod error;
mod handlers;
mod limiter;
mod models;
mod store;
mod auth;

use crate::cache::ImageCache;
use crate::limiter::IpRateLimiter;
use auth::Auth;
use anyhow::Result;
use std::net::SocketAddr;
use std::path::PathBuf;
use time::macros::format_description;
use tracing::info;
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
    let store = warp::any().map(move || store.clone());

    let cache = cache.clone();
    let cache = warp::any().map(move || cache.clone());

    let rate_limiter = rate_limiter.clone();
    let rate_limiter = warp::any().map(move || rate_limiter.clone());

    let random = warp::path("random")
        .and(warp::get())
        .and(store.clone())
        .and(cache.clone())
        .and(rate_limiter.clone())
        .and(warp::filters::header::headers_cloned())
        .and_then(handlers::get_random_image_handler);

    let add_image = warp::path("image")
        .and(warp::post())
        .and(store.clone())
        .and(warp::body::json())
        .and_then(handlers::add_image_handler);

    let images = warp::path("images").and(warp::fs::dir("images"));

    let image = warp::path!("images" / String)
        .and(warp::get())
        .and(store.clone())
        .and(cache.clone())
        .and(rate_limiter.clone())
        .and(warp::filters::header::headers_cloned())
        .and_then(handlers::get_image_by_filename_handler);

    let auth = Auth::new(config.admin_key);

    let api_key_routes = warp::path("api-keys")
        .and(warp::post())
        .and(auth.require_admin())
        .and(store.clone())
        .and(warp::body::json())
        .and_then(handlers::generate_api_key_handler)
        .or(warp::path("api-keys")
            .and(warp::delete())
            .and(auth.require_admin())
            .and(store.clone())
            .and(warp::body::json())
            .and_then(handlers::remove_api_key_handler))
        .or(warp::path("api-keys")
            .and(warp::get())
            .and(auth.require_admin())
            .and(store.clone())
            .and_then(handlers::list_api_keys_handler));

    let routes = random
        .or(add_image)
        .or(images)
        .or(image)
        .or(api_key_routes)
        .recover(error::handle_rejection);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    info!("Server started at http://{}:{}", config.host, config.port);
    warp::serve(routes).run(addr).await;

    Ok(())
}
