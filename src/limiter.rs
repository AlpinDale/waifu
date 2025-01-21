use crate::store::ImageStore;
use std::collections::HashMap;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;
use tracing::{debug, warn};

#[derive(Clone)]
pub struct ApiKeyRateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<OffsetDateTime>>>>,
    store: ImageStore,
    default_max_requests: u32,
    window_size: Duration,
}

impl ApiKeyRateLimiter {
    pub fn new(store: ImageStore, default_max_requests: u32, window_size: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            store,
            default_max_requests,
            window_size,
        }
    }

    pub async fn check_rate_limit(&self, api_key: &str) -> bool {
        let rate_limit = match self.store.get_api_key(api_key) {
            Ok(key_info) => {
                if key_info.requests_per_second.is_none() {
                    debug!("API key has unlimited rate limit");
                    return true;
                }
                key_info
                    .requests_per_second
                    .unwrap_or(self.default_max_requests)
            }
            Err(e) => {
                warn!("Failed to get API key info: {}, using default limit", e);
                self.default_max_requests
            }
        };

        let now = OffsetDateTime::now_utc();
        let window_start = now - self.window_size;

        let mut requests = self.requests.lock().await;
        let request_times = requests.entry(api_key.to_string()).or_default();
        request_times.retain(|&time| time > window_start);

        if request_times.len() >= rate_limit as usize {
            warn!(
                "Rate limit exceeded: {}/{} requests in {:?}",
                request_times.len(),
                rate_limit,
                self.window_size
            );
            return false;
        }

        request_times.push(now);
        true
    }
}

impl Default for ApiKeyRateLimiter {
    fn default() -> Self {
        unimplemented!("ApiKeyRateLimiter requires store instance")
    }
}
