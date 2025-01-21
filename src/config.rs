use anyhow::Result;
use std::env;

#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub images_path: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8000".to_string())
            .parse()?;
        let images_path = env::var("IMAGES_PATH").unwrap_or_else(|_| "/images".to_string());

        Ok(Self {
            host,
            port,
            images_path,
        })
    }
}
