use crate::error::ImageError;
use crate::limiter::ApiKeyRateLimiter;
use crate::store::ImageStore;
use std::sync::Arc;
use tracing::error;
use warp::{Filter, Rejection};

#[derive(Clone)]
pub struct Auth {
    admin_key: Arc<String>,
    store: ImageStore,
    rate_limiter: ApiKeyRateLimiter,
}

impl Auth {
    pub fn new(admin_key: String, store: ImageStore, rate_limiter: ApiKeyRateLimiter) -> Self {
        Self {
            admin_key: Arc::new(admin_key),
            store,
            rate_limiter,
        }
    }

    fn truncate_key(key: &str) -> String {
        key.chars().take(8).collect::<String>() + "..."
    }

    pub async fn check_api_key(&self, auth_header: Option<String>) -> Result<(), Rejection> {
        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let key = header.trim_start_matches("Bearer ").trim();

                if !self.rate_limiter.check_rate_limit(key).await {
                    error!(
                        api_key = %Self::truncate_key(key),
                        "Rate limit exceeded for API key"
                    );
                    return Err(warp::reject::custom(ImageError::RateLimitExceeded));
                }

                // admin is almighty
                if key == self.admin_key.as_str() {
                    return Ok(());
                }

                match self.store.validate_api_key(key) {
                    Ok(true) => Ok(()),
                    Ok(false) => Err(warp::reject::custom(ImageError::Unauthorized)),
                    Err(_) => Err(warp::reject::custom(ImageError::Unauthorized)),
                }
            }
            _ => Err(warp::reject::custom(ImageError::Unauthorized)),
        }
    }

    pub fn check_admin(&self, auth_header: Option<String>) -> Result<(), Rejection> {
        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let key = header.trim_start_matches("Bearer ").trim();
                if key == self.admin_key.as_str() {
                    Ok(())
                } else {
                    Err(warp::reject::custom(ImageError::Unauthorized))
                }
            }
            _ => Err(warp::reject::custom(ImageError::Unauthorized)),
        }
    }

    pub fn require_auth(&self) -> impl Filter<Extract = ((),), Error = Rejection> + Clone {
        let auth = self.clone();
        warp::header::optional::<String>("authorization").and_then(move |header: Option<String>| {
            let auth = auth.clone();
            async move { auth.check_api_key(header).await }
        })
    }

    pub fn require_admin(&self) -> impl Filter<Extract = ((),), Error = Rejection> + Clone {
        let auth = self.clone();
        warp::header::optional::<String>("authorization").and_then(move |header: Option<String>| {
            let auth = auth.clone();
            async move { auth.check_admin(header) }
        })
    }
}
