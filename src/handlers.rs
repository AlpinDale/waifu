use crate::error::ImageError;
use crate::models::{AddImageRequest, ImageResponse};
use crate::store::ImageStore;
use warp::{Rejection, Reply};

pub async fn get_random_image_handler(store: ImageStore) -> Result<impl Reply, Rejection> {
    match store.get_random_image() {
        Ok(url) => Ok(warp::reply::json(&ImageResponse { url })),
        Err(_) => Err(warp::reject::not_found()),
    }
}

pub async fn add_image_handler(
    store: ImageStore,
    body: AddImageRequest,
) -> Result<impl Reply, Rejection> {
    match store.add_image(&body.path, body.path_type) {
        Ok(_) => Ok(warp::reply::with_status(
            "Image added successfully",
            warp::http::StatusCode::CREATED,
        )),
        Err(e) => {
            if e.to_string().contains("not found") {
                Err(warp::reject::custom(ImageError::PathNotFound(
                    e.to_string(),
                )))
            } else {
                Err(warp::reject::custom(ImageError::DatabaseError(
                    e.to_string(),
                )))
            }
        }
    }
}
