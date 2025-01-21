use governor::clock::DefaultClock;
use governor::state::keyed::DashMapStateStore;
use governor::{Quota, RateLimiter};
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::str::FromStr;
use std::sync::Arc;
use warp::http::HeaderMap;

pub type RateLimiterState = RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock>;

#[derive(Clone)]
pub struct IpRateLimiter {
    limiter: Arc<RateLimiterState>,
}

impl IpRateLimiter {
    pub fn new(requests: u32, window_secs: u64) -> Self {
        let quota = Quota::with_period(std::time::Duration::from_secs(window_secs))
            .unwrap()
            .allow_burst(NonZeroU32::new(requests).unwrap());

        let limiter = RateLimiter::dashmap(quota);

        Self {
            limiter: Arc::new(limiter),
        }
    }

    pub fn check_headers(&self, headers: &HeaderMap) -> bool {
        let ip = self.extract_ip(headers);
        self.check(ip)
    }

    pub fn check(&self, ip: IpAddr) -> bool {
        self.limiter.check_key(&ip).is_ok()
    }

    fn extract_ip(&self, headers: &HeaderMap) -> IpAddr {
        if let Some(ip) = headers
            .get("CF-Connecting-IP")
            .and_then(|h| h.to_str().ok())
            .and_then(|ip| IpAddr::from_str(ip).ok())
        {
            return ip;
        }

        if let Some(ip) = headers
            .get("X-Forwarded-For")
            .and_then(|h| h.to_str().ok())
            .and_then(|ip| ip.split(',').next())
            .and_then(|ip| ip.trim().parse().ok())
        {
            return ip;
        }

        IpAddr::from_str("127.0.0.1").unwrap()
    }
}
