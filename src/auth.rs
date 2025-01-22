use crate::error::ImageError;
use crate::limiter::ApiKeyRateLimiter;
use crate::models::ApiKey;
use crate::store::ImageStore;
use std::sync::Arc;
use time::OffsetDateTime;
use tracing::warn;
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

                // admin is almighty and we don't track usage
                if key == self.admin_key.as_str() {
                    return Ok(());
                }

                if !self.rate_limiter.check_rate_limit(key).await {
                    warn!(
                        api_key = %Self::truncate_key(key),
                        "Rate limit exceeded for API key"
                    );
                    return Err(warp::reject::custom(ImageError::RateLimitExceeded));
                }

                if let Err(e) = self.store.update_key_last_used(key) {
                    warn!(
                        api_key = %Self::truncate_key(key),
                        error = %e,
                        "Failed to update last_used_at timestamp"
                    );
                }

                match self.store.validate_api_key(key) {
                    Ok(true) => Ok(()),
                    Ok(false) => Err(warp::reject::custom(ImageError::Unauthorized)),
                    Err(e) if e.to_string().contains("inactive_key") => {
                        Err(warp::reject::custom(ImageError::InactiveKey))
                    }
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

    pub fn require_auth_info(&self) -> impl Filter<Extract = (ApiKey,), Error = Rejection> + Clone {
        let auth = self.clone();
        warp::header::optional::<String>("authorization").and_then(move |header: Option<String>| {
            let auth = auth.clone();
            async move {
                match header {
                    Some(header) if header.starts_with("Bearer ") => {
                        let key = header.trim_start_matches("Bearer ").trim();

                        // Check if it's the admin key
                        if key == auth.admin_key.as_str() {
                            return Ok(ApiKey {
                                key: key.to_string(),
                                username: "admin".to_string(),
                                created_at: OffsetDateTime::now_utc(),
                                last_used_at: None,
                                is_active: true,
                                requests_per_second: None, // unlimited
                                max_batch_size: None,      // unlimited
                            });
                        }

                        // Check rate limit
                        if !auth.rate_limiter.check_rate_limit(key).await {
                            return Err(warp::reject::custom(ImageError::RateLimitExceeded));
                        }

                        // Update last used timestamp
                        if let Err(e) = auth.store.update_key_last_used(key) {
                            warn!(
                                api_key = %Self::truncate_key(key),
                                error = %e,
                                "Failed to update last_used_at timestamp"
                            );
                        }

                        // Get API key info
                        match auth.store.get_api_key(key) {
                            Ok(api_key) if api_key.is_active => Ok(api_key),
                            Ok(_) => Err(warp::reject::custom(ImageError::InactiveKey)),
                            Err(_) => Err(warp::reject::custom(ImageError::Unauthorized)),
                        }
                    }
                    _ => Err(warp::reject::custom(ImageError::Unauthorized)),
                }
            }
        })
    }
}
