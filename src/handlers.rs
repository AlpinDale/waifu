use crate::error::ImageError;
use crate::models::AddImageRequest;
use crate::store::ImageStore;
use tracing::{error, info};
use warp::{Rejection, Reply};

pub async fn get_random_image_handler(store: ImageStore) -> Result<impl Reply, Rejection> {
    match store.get_random_image() {
        Ok(response) => {
            info!(
                "Retrieved random image: {} ({}x{} pixels, {} bytes)",
                response.filename, response.width, response.height, response.size_bytes
            );
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            error!("Failed to get random image: {}", e);
            Err(warp::reject::not_found())
        }
    }
}

pub async fn add_image_handler(
    store: ImageStore,
    body: AddImageRequest,
) -> Result<impl Reply, Rejection> {
    info!("Adding new image from {}", body.path);
    match store.add_image(&body.path, body.path_type).await {
        Ok(_) => {
            info!("Successfully added image from {}", body.path);
            Ok(warp::reply::with_status(
                "Image added successfully",
                warp::http::StatusCode::CREATED,
            ))
        }
        Err(e) => {
            error!("Failed to add image: {}", e);
            let err = if e.to_string().contains("not found") {
                ImageError::PathNotFound(e.to_string())
            } else if e.to_string().contains("too large") {
                ImageError::FileTooLarge(e.to_string())
            } else if e.to_string().contains("Invalid image")
                || e.to_string().contains("Unsupported image format")
            {
                ImageError::InvalidImage(e.to_string())
            } else {
                ImageError::DatabaseError(e.to_string())
            };
            Err(warp::reject::custom(err))
        }
    }
}
