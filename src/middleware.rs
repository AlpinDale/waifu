use std::convert::Infallible;
use tracing::info;
use uuid::Uuid;
use warp::{Filter, Reply};

pub fn with_request_id() -> impl Filter<Extract = (String,), Error = Infallible> + Clone {
    warp::any()
        .map(|| Uuid::new_v4().to_string())
        .and_then(|request_id: String| async move {
            info!(request_id = %request_id, "Processing request");
            Ok::<_, Infallible>(request_id)
        })
}

pub fn add_request_id_header<T: Reply>(reply: T, request_id: String) -> impl Reply {
    warp::reply::with_header(reply, "X-Request-ID", request_id)
}
