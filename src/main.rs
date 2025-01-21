mod config;
mod error;
mod handlers;
mod models;
mod store;

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

    let random = warp::path("random")
        .and(warp::get())
        .and(store.clone())
        .and_then(handlers::get_random_image_handler);

    let add_image = warp::path("image")
        .and(warp::post())
        .and(store)
        .and(warp::body::json())
        .and_then(handlers::add_image_handler);

    let images = warp::path("images").and(warp::fs::dir("images"));

    let routes = random
        .or(add_image)
        .or(images)
        .recover(error::handle_rejection);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    info!("Server started at http://{}:{}", config.host, config.port);
    warp::serve(routes).run(addr).await;

    Ok(())
}
