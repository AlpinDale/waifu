use crate::models::ImageResponse;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct ImageCache {
    cache: Arc<Cache<String, ImageResponse>>,
}

impl ImageCache {
    pub fn new(max_capacity: usize, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity as u64)
            .time_to_live(ttl)
            .build();

        Self {
            cache: Arc::new(cache),
        }
    }

    pub async fn get(&self, key: &str) -> Option<ImageResponse> {
        self.cache.get(key).await
    }

    pub async fn insert(&self, key: String, value: ImageResponse) {
        self.cache.insert(key, value).await;
    }
}
