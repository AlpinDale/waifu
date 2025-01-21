#[derive(Debug)]
pub enum ImageError {
    PathNotFound(String),
    DatabaseError(String),
}

impl warp::reject::Reject for ImageError {}

pub async fn handle_rejection(
    err: warp::Rejection,
) -> Result<impl warp::Reply, std::convert::Infallible> {
    let (code, message) = if err.is_not_found() {
        (warp::http::StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(e) = err.find::<ImageError>() {
        match e {
            ImageError::PathNotFound(msg) => (warp::http::StatusCode::BAD_REQUEST, msg.clone()),
            ImageError::DatabaseError(msg) => {
                (warp::http::StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
        }
    } else {
        (
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    Ok(warp::reply::with_status(message, code))
}
