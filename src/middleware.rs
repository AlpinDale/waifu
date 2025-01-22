use tracing::info;
use uuid::Uuid;
use warp::http::HeaderValue;
use warp::{Filter, Rejection, Reply};

pub fn with_request_id() -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
    warp::any()
        .map(|| Uuid::new_v4().to_string())
        .and_then(|request_id: String| async move {
            info!(request_id = %request_id, "Processing request");
            Ok::<String, Rejection>(request_id)
        })
}

pub fn add_request_id_header<T: Reply>(reply: T, request_id: String) -> impl Reply {
    let mut response = reply.into_response();
    response.headers_mut().insert(
        "X-Request-ID",
        HeaderValue::from_str(&request_id).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    response
}
