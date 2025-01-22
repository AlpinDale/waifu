use serde::Serialize;
use tracing::error;
use warp::{http::StatusCode, reject::Reject, Rejection, Reply};

#[derive(Debug)]
pub enum ImageError {
    PathNotFound(String),
    DatabaseError(String),
    InvalidImage(String),
    FileTooLarge(String),
    RateLimitExceeded,
    UsernameExists(String),
    Unauthorized,
    InactiveKey,
    UsernameNotFound(String),
    DuplicateImage(String),
    MissingTags,
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    message: String,
}

impl Reject for ImageError {}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    error!("Request rejected: {:?}", err);

    let (code, message) = if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        if e.to_string().contains("missing field `tags`") {
            (
                StatusCode::BAD_REQUEST,
                "The 'tags' field is required when uploading an image".to_string(),
            )
        } else {
            (
                StatusCode::BAD_REQUEST,
                "Invalid request format. Please check the API documentation for required fields."
                    .to_string(),
            )
        }
    } else if let Some(e) = err.find::<ImageError>() {
        match e {
            ImageError::PathNotFound(msg) => (
                StatusCode::NOT_FOUND,
                format!("The specified image path was not found: {}", msg),
            ),
            ImageError::DatabaseError(msg) => {
                error!("Database error details: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred".to_string(),
                )
            }
            ImageError::InvalidImage(msg) => (
                StatusCode::BAD_REQUEST,
                format!("The provided file is not a valid image: {}", msg),
            ),
            ImageError::FileTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("The image file exceeds the maximum allowed size: {}", msg),
            ),
            ImageError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded. Please try again later.".to_string(),
            ),
            ImageError::UsernameExists(username) => (
                StatusCode::CONFLICT,
                format!("The username '{}' is already in use", username),
            ),
            ImageError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Invalid or missing API key".to_string(),
            ),
            ImageError::InactiveKey => (
                StatusCode::UNAUTHORIZED,
                "This API key has been deactivated. Please contact the administrator.".to_string(),
            ),
            ImageError::UsernameNotFound(username) => (
                StatusCode::NOT_FOUND,
                format!("The username '{}' was not found", username),
            ),
            ImageError::DuplicateImage(msg) => (
                StatusCode::CONFLICT,
                format!("This image has already been uploaded: {}", msg),
            ),
            ImageError::MissingTags => (
                StatusCode::BAD_REQUEST,
                "At least one tag is required when uploading an image".to_string(),
            ),
        }
    } else if err.is_not_found() {
        (
            StatusCode::NOT_FOUND,
            "The requested resource was not found".to_string(),
        )
    } else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
        (
            StatusCode::METHOD_NOT_ALLOWED,
            "This method is not allowed for this endpoint".to_string(),
        )
    } else {
        error!("Unhandled rejection: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "An internal error occurred".to_string(),
        )
    };

    let json = warp::reply::json(&ErrorResponse {
        code: code.as_u16(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}
