use anyhow::Result;
use clap::Parser;

#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    pub host: String,

    #[arg(long, env = "PORT", default_value = "8000")]
    pub port: u16,

    #[arg(long, env = "IMAGES_PATH", default_value = "/images")]
    pub images_path: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self::parse())
    }
}
