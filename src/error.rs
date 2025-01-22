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
        error!("Deserialization error details: {}", e);
        (
            StatusCode::BAD_REQUEST,
            "Invalid request format. Please check the API documentation for required fields."
                .to_string(),
        )
    } else if let Some(e) = err.find::<ImageError>() {
        match e {
            ImageError::PathNotFound(_) => (
                StatusCode::NOT_FOUND,
                "The specified image path was not found".to_string(),
            ),
            ImageError::DatabaseError(msg) => {
                error!("Database error details: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred".to_string(),
                )
            }
            ImageError::InvalidImage(_) => (
                StatusCode::BAD_REQUEST,
                "The provided file is not a valid image".to_string(),
            ),
            ImageError::FileTooLarge(_) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "The image file exceeds the maximum allowed size".to_string(),
            ),
            ImageError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded. Please try again later.".to_string(),
            ),
            ImageError::UsernameExists(_) => (
                StatusCode::CONFLICT,
                "The specified username is already in use".to_string(),
            ),
            ImageError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Invalid or missing API key".to_string(),
            ),
            ImageError::UsernameNotFound(_) => (
                StatusCode::NOT_FOUND,
                "The specified username was not found".to_string(),
            ),
            ImageError::DuplicateImage(_) => (
                StatusCode::CONFLICT,
                "This image has already been uploaded".to_string(),
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
