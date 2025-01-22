use serde::Serialize;
use std::fmt;
use tracing::error;
use uuid::Uuid;
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
    BatchSizeExceeded(u32),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::PathNotFound(msg) => write!(f, "Path not found: {}", msg),
            ImageError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            ImageError::InvalidImage(msg) => write!(f, "Invalid image: {}", msg),
            ImageError::FileTooLarge(msg) => write!(f, "File too large: {}", msg),
            ImageError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            ImageError::UsernameExists(username) => {
                write!(f, "Username already exists: {}", username)
            }
            ImageError::Unauthorized => write!(f, "Unauthorized"),
            ImageError::InactiveKey => write!(f, "API key is inactive"),
            ImageError::UsernameNotFound(username) => write!(f, "Username not found: {}", username),
            ImageError::DuplicateImage(msg) => write!(f, "Duplicate image: {}", msg),
            ImageError::MissingTags => write!(f, "Missing tags"),
            ImageError::BatchSizeExceeded(max) => {
                write!(f, "Batch size exceeds maximum of {}", max)
            }
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    message: String,
    request_id: String,
}

impl Reject for ImageError {}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    let request_id = Uuid::new_v4().to_string();
    error!(request_id = %request_id, "Request rejected: {:?}", err);

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
            ImageError::PathNotFound(msg) => (StatusCode::NOT_FOUND, msg.to_string()),
            ImageError::DatabaseError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", msg),
            ),
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
            ImageError::BatchSizeExceeded(max) => (
                StatusCode::BAD_REQUEST,
                format!("Batch size exceeds maximum allowed size of {}", max),
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
        error!(request_id = %request_id, "Unhandled rejection: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "An internal error occurred".to_string(),
        )
    };

    let json = warp::reply::json(&ErrorResponse {
        code: code.as_u16(),
        message,
        request_id,
    });

    Ok(warp::reply::with_status(json, code))
}
