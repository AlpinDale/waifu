use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Clone)]
pub struct ImageResponse {
    pub url: String,
    pub filename: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub hash: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified_at: OffsetDateTime,
}

#[derive(Deserialize)]
pub struct AddImageRequest {
    pub path: String,
    #[serde(rename = "type")]
    pub path_type: PathType,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PathType {
    Url,
    Local,
}

#[derive(Debug, Deserialize)]
pub struct GenerateApiKeyRequest {
    pub username: String,
    pub requests_per_second: Option<u32>, // none = unlimited
}

#[derive(Deserialize)]
pub struct RemoveApiKeyRequest {
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub requests_per_second: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ApiKey {
    pub key: String,
    pub username: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_used_at: Option<OffsetDateTime>,
    pub is_active: bool,
    pub requests_per_second: Option<u32>,
}
