use std::collections::HashMap;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ApiKeyRateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<OffsetDateTime>>>>,
    max_requests: u32,
    window_size: Duration,
}

impl ApiKeyRateLimiter {
    pub fn new(max_requests: u32, window_size: Duration) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_size,
        }
    }

    pub async fn check_rate_limit(&self, api_key: &str) -> bool {
        let now = OffsetDateTime::now_utc();
        let window_start = now - self.window_size;

        let mut requests = self.requests.lock().await;

        let request_times = requests.entry(api_key.to_string()).or_default();
        request_times.retain(|&time| time > window_start);

        if request_times.len() >= self.max_requests as usize {
            return false;
        }

        request_times.push(now);
        true
    }
}

impl Default for ApiKeyRateLimiter {
    fn default() -> Self {
        Self::new(10, Duration::seconds(1))
    }
}
