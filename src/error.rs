use serde::Serialize;
use warp::{http::StatusCode, reject::Reject, Rejection, Reply};

#[derive(Debug)]
pub enum ImageError {
    PathNotFound(String),
    DatabaseError(String),
    InvalidImage(String),
    FileTooLarge(String),
    RateLimitExceeded,
    UsernameExists(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    message: String,
}

impl Reject for ImageError {}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    let (code, message) = if err.is_not_found() {
        (
            StatusCode::NOT_FOUND,
            "The requested resource was not found".to_string(),
        )
    } else if let Some(e) = err.find::<ImageError>() {
        match e {
            ImageError::PathNotFound(msg) => (StatusCode::NOT_FOUND, msg.to_string()),
            ImageError::DatabaseError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", msg),
            ),
            ImageError::InvalidImage(msg) => {
                (StatusCode::BAD_REQUEST, format!("Invalid image: {}", msg))
            }
            ImageError::FileTooLarge(msg) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("File too large: {}", msg),
            ),
            ImageError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded. Please try again later.".to_string(),
            ),
            ImageError::UsernameExists(username) => (
                StatusCode::CONFLICT,
                format!("API key already exists for username: {}", username),
            ),
        }
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        )
    };

    let json = warp::reply::json(&ErrorResponse {
        code: code.as_u16(),
        message,
    });

    Ok(warp::reply::with_status(json, code))
}
