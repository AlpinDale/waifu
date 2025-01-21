use crate::error::ImageError;
use std::sync::Arc;
use warp::{Filter, Rejection};

#[derive(Clone)]
pub struct Auth {
    admin_key: Arc<String>,
}

impl Auth {
    pub fn new(admin_key: String) -> Self {
        Self {
            admin_key: Arc::new(admin_key),
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

    pub fn require_admin(
        &self,
    ) -> impl Filter<Extract = (), Error = Rejection> + Clone {
        let auth = self.clone();
        warp::header::optional::<String>("authorization")
            .and_then(move |header: Option<String>| {
                let auth = auth.clone();
                async move { 
                    auth.check_admin(header).map(|_| ((),)) 
                }
            })
            .map(|_| ())
    }
}