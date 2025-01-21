use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ImageResponse {
    pub url: String,
    pub filename: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
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
