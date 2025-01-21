use anyhow::Result;
use clap::Parser;
use std::time::Duration;

#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    pub host: String,

    #[arg(long, env = "PORT", default_value = "8000")]
    pub port: u16,

    #[arg(long, env = "IMAGES_PATH", default_value = "/images")]
    pub images_path: String,

    #[arg(long, env = "RATE_LIMIT_REQUESTS", default_value = "2")]
    pub rate_limit_requests: u32,

    #[arg(long, env = "RATE_LIMIT_WINDOW_SECS", default_value = "1")]
    pub rate_limit_window_secs: u64,

    #[arg(long, env = "CACHE_SIZE", default_value = "100")]
    pub cache_size: usize,

    #[arg(long, env = "CACHE_TTL_SECS", default_value = "300")]
    pub cache_ttl_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self::parse())
    }

    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.cache_ttl_secs)
    }
}
