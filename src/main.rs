mod error;
mod handlers;
mod models;
mod store;

use anyhow::Result;
use std::path::PathBuf;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    let images_dir = PathBuf::from("images");
    let base_url = "http://localhost:8000/images";

    let store = store::ImageStore::new("images.db", images_dir.clone(), base_url.to_string())?;
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

    println!("Server started at http://localhost:8000");
    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;

    Ok(())
}
