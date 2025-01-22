use crate::cache::ImageCache;
use crate::error::ImageError;
use crate::models::{
    AddImageRequest, GenerateApiKeyRequest, RemoveApiKeyRequest, UpdateApiKeyRequest,
    UpdateApiKeyStatusRequest,
};
use crate::store::ImageStore;
use serde_json::json;
use tracing::{error, info};
use warp::{http::HeaderMap, Rejection, Reply};

pub async fn get_random_image_handler(
    store: ImageStore,
    cache: ImageCache,
    tags: Option<String>,
    _headers: HeaderMap,
    _: (),
) -> Result<impl Reply, Rejection> {
    let tags = tags.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>()
    });

    match store.get_random_image_with_tags(tags.as_deref()) {
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
    if body.tags.is_empty() {
        error!("Attempt to upload image without tags");
        return Err(warp::reject::custom(ImageError::MissingTags));
    }

    info!(
        "Adding new image from {} with tags: {:?}",
        body.path, body.tags
    );
    match store.add_image(&body.path, body.path_type).await {
        Ok(hash) => {
            match store.add_tags(&hash, &body.tags) {
                Ok(_) => info!("Successfully added tags: {:?}", body.tags),
                Err(e) => {
                    error!("Failed to add tags: {}", e);
                    return Err(warp::reject::custom(ImageError::DatabaseError(format!(
                        "Failed to add tags: {}",
                        e
                    ))));
                }
            }
            info!("Successfully added image from {}", body.path);
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({
                    "message": "Image added successfully",
                    "hash": hash,
                    "tags": body.tags
                })),
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
            } else if e.to_string().contains("already exists") {
                ImageError::DuplicateImage(e.to_string())
            } else {
                error!("Unexpected error: {}", e);
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
    _headers: HeaderMap,
) -> Result<impl Reply, Rejection> {
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

pub async fn generate_api_key_handler(
    _: (),
    store: ImageStore,
    body: GenerateApiKeyRequest,
) -> Result<impl Reply, Rejection> {
    match store.generate_api_key(&body.username, body.requests_per_second) {
        Ok(api_key) => {
            info!(
                username = %body.username,
                rate_limit = ?body.requests_per_second,
                "Generated new API key"
            );
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({
                    "username": body.username,
                    "api_key": api_key,
                    "rate_limit": body.requests_per_second.map(|r| format!("{} requests/second", r))
                        .unwrap_or_else(|| "unlimited".to_string())
                })),
                warp::http::StatusCode::CREATED,
            ))
        }
        Err(e) if e.to_string().contains("UNIQUE constraint failed") => {
            error!("Username already exists: {}", body.username);
            Err(warp::reject::custom(ImageError::UsernameExists(
                body.username,
            )))
        }
        Err(e) => {
            error!("Failed to generate API key: {}", e);
            Err(warp::reject::custom(ImageError::DatabaseError(
                e.to_string(),
            )))
        }
    }
}

pub async fn remove_api_key_handler(
    _: (),
    store: ImageStore,
    body: RemoveApiKeyRequest,
) -> Result<impl Reply, Rejection> {
    match store.remove_api_key(&body.username) {
        Ok(true) => Ok(warp::reply::json(&serde_json::json!({
            "message": format!("API key for user '{}' was successfully removed", body.username)
        }))),
        Ok(false) => Err(warp::reject::custom(ImageError::UsernameNotFound(
            body.username,
        ))),
        Err(e) => Err(warp::reject::custom(ImageError::DatabaseError(
            e.to_string(),
        ))),
    }
}

pub async fn list_api_keys_handler(_: (), store: ImageStore) -> Result<impl Reply, Rejection> {
    match store.list_api_keys() {
        Ok(keys) => {
            info!("Listed {} API keys", keys.len());
            Ok(warp::reply::json(&keys))
        }
        Err(e) => {
            error!("Failed to list API keys: {}", e);
            Err(warp::reject::custom(ImageError::DatabaseError(
                e.to_string(),
            )))
        }
    }
}

pub async fn update_api_key_handler(
    username: String,
    _: (), // Admin auth result
    store: ImageStore,
    body: UpdateApiKeyRequest,
) -> Result<impl Reply, Rejection> {
    match store.update_api_key_rate_limit(&username, body.requests_per_second) {
        Ok(()) => {
            info!(
                username = %username,
                new_rate_limit = ?body.requests_per_second,
                "Updated API key rate limit"
            );
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({
                    "message": "API key updated successfully",
                    "username": username,
                    "rate_limit": body.requests_per_second.map(|r| format!("{} requests/second", r))
                        .unwrap_or_else(|| "unlimited".to_string())
                })),
                warp::http::StatusCode::OK,
            ))
        }
        Err(e) => {
            error!("Failed to update API key: {}", e);
            Err(warp::reject::custom(ImageError::DatabaseError(
                e.to_string(),
            )))
        }
    }
}

pub async fn update_api_key_status_handler(
    username: String,
    _: (), // Admin auth result
    store: ImageStore,
    body: UpdateApiKeyStatusRequest,
) -> Result<impl Reply, Rejection> {
    match store.update_api_key_status(&username, body.is_active) {
        Ok(()) => {
            info!(
                username = %username,
                is_active = body.is_active,
                "Updated API key status"
            );
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({
                    "message": "API key status updated successfully",
                    "username": username,
                    "is_active": body.is_active
                })),
                warp::http::StatusCode::OK,
            ))
        }
        Err(e) => {
            error!("Failed to update API key status: {}", e);
            if e.to_string().contains("No API key found") {
                Err(warp::reject::custom(ImageError::UsernameNotFound(username)))
            } else {
                Err(warp::reject::custom(ImageError::DatabaseError(
                    e.to_string(),
                )))
            }
        }
    }
}

pub async fn remove_image_handler(
    filename: String,
    store: ImageStore,
    _: (), // Admin auth result
) -> Result<impl Reply, Rejection> {
    match store.remove_image(&filename) {
        Ok(()) => {
            info!("Successfully removed image: {}", filename);
            Ok(warp::reply::with_status(
                warp::reply::json(&json!({
                    "message": format!("Image '{}' was successfully removed", filename)
                })),
                warp::http::StatusCode::OK,
            ))
        }
        Err(e) => {
            error!("Failed to remove image {}: {}", filename, e);
            Err(warp::reject::custom(ImageError::DatabaseError(
                e.to_string(),
            )))
        }
    }
}

pub async fn remove_image_tags_handler(
    filename: String,
    store: ImageStore,
    tags: Vec<String>,
    _: (), // Admin auth result
) -> Result<impl Reply, Rejection> {
    let image = match store.get_image_by_filename(&filename) {
        Ok(img) => img,
        Err(e) => {
            error!("Failed to get image {}: {}", filename, e);
            return Err(warp::reject::not_found());
        }
    };

    match store.remove_tags(&image.hash, &tags) {
        Ok(()) => {
            info!(
                "Successfully removed tags {:?} from image: {}",
                tags, filename
            );
            Ok(warp::reply::with_status(
                warp::reply::json(&serde_json::json!({
                    "message": format!("Tags removed successfully from image '{}'", filename),
                    "removed_tags": tags
                })),
                warp::http::StatusCode::OK,
            ))
        }
        Err(e) => {
            error!("Failed to remove tags from image {}: {}", filename, e);
            Err(warp::reject::custom(ImageError::DatabaseError(
                e.to_string(),
            )))
        }
    }
}
