use crate::error::ImageError;
use crate::store::ImageStore;
use std::sync::Arc;
use warp::{Filter, Rejection};

#[derive(Clone)]
pub struct Auth {
    admin_key: Arc<String>,
    store: ImageStore,
}

impl Auth {
    pub fn new(admin_key: String, store: ImageStore) -> Self {
        Self {
            admin_key: Arc::new(admin_key),
            store,
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

    pub fn check_api_key(&self, auth_header: Option<String>) -> Result<(), Rejection> {
        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let key = header.trim_start_matches("Bearer ").trim();

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

    pub fn require_admin(&self) -> impl Filter<Extract = ((),), Error = Rejection> + Clone {
        let auth = self.clone();
        warp::header::optional::<String>("authorization").and_then(move |header: Option<String>| {
            let auth = auth.clone();
            async move { auth.check_admin(header) }
        })
    }

    pub fn require_auth(&self) -> impl Filter<Extract = ((),), Error = Rejection> + Clone {
        let auth = self.clone();
        warp::header::optional::<String>("authorization").and_then(move |header: Option<String>| {
            let auth = auth.clone();
            async move { auth.check_api_key(header) }
        })
    }
}
