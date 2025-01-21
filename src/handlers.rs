use crate::cache::ImageCache;
use crate::error::ImageError;
use crate::limiter::IpRateLimiter;
use crate::models::AddImageRequest;
use crate::store::ImageStore;
use tracing::{error, info};
use warp::{http::HeaderMap, Rejection, Reply};

pub async fn get_random_image_handler(
    store: ImageStore,
    cache: ImageCache,
    rate_limiter: IpRateLimiter,
    headers: HeaderMap,
) -> Result<impl Reply, Rejection> {
    if !rate_limiter.check_headers(&headers) {
        error!("Rate limit exceeded");
        return Err(warp::reject::custom(ImageError::RateLimitExceeded));
    }

    match store.get_random_image() {
        Ok(response) => {
            info!(
                "Retrieved random image: {} ({}x{} pixels, {} bytes)",
                response.filename, response.width, response.height, response.size_bytes
            );
            cache
                .insert(response.filename.clone(), response.clone())
                .await;
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

pub async fn get_image_by_filename_handler(
    filename: String,
    store: ImageStore,
    cache: ImageCache,
    rate_limiter: IpRateLimiter,
    headers: HeaderMap,
) -> Result<impl Reply, Rejection> {
    if !rate_limiter.check_headers(&headers) {
        error!("Rate limit exceeded");
        return Err(warp::reject::custom(ImageError::RateLimitExceeded));
    }

    if let Some(cached) = cache.get(&filename).await {
        info!("Cache hit for image: {}", filename);
        return Ok(warp::reply::json(&cached));
    }

    match store.get_image_by_filename(&filename) {
        Ok(response) => {
            info!(
                "Retrieved image: {} ({}x{} pixels, {} bytes)",
                response.filename, response.width, response.height, response.size_bytes
            );
            cache.insert(filename, response.clone()).await;
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            error!("Failed to get image {}: {}", filename, e);
            Err(warp::reject::not_found())
        }
    }
}
