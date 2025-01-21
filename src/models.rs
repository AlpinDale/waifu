use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ImageResponse {
    pub url: String,
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
