use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Serialize, Clone)]
pub struct ImageResponse {
    pub url: String,
    pub filename: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub hash: String,
    pub tags: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct AddImageRequest {
    pub path: String,
    #[serde(rename = "type")]
    pub path_type: PathType,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PathType {
    Url,
    Local,
}

#[derive(Debug, Deserialize)]
pub struct GenerateApiKeyRequest {
    pub username: String,
    pub requests_per_second: Option<u32>, // none = unlimited
    pub max_batch_size: Option<u32>,      // none = no batching allowed (default=1)
}

#[derive(Deserialize)]
pub struct RemoveApiKeyRequest {
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyRequest {
    pub requests_per_second: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct BatchAddImageRequest {
    pub images: Vec<AddImageRequest>,
}

#[derive(Debug, Deserialize)]
pub struct BatchRandomRequest {
    pub count: u32,
}

#[derive(Debug, Serialize)]
pub struct BatchImageResponse {
    pub images: Vec<ImageResponse>,
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
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
    pub max_batch_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyStatusRequest {
    pub is_active: bool,
}

#[derive(Debug)]
pub struct ImageFilters {
    pub tags: Option<Vec<String>>,
    pub width: Option<DimensionFilter>,
    pub height: Option<DimensionFilter>,
    pub size: Option<SizeFilter>,
}

#[derive(Debug)]
pub enum DimensionFilter {
    Exact(u32),
    Range(u32, u32),
}

#[derive(Debug)]
pub enum SizeFilter {
    Exact(u64),
    Range(u64, u64),
}

impl ImageFilters {
    pub fn from_query(params: &std::collections::HashMap<String, String>) -> Self {
        let tags = params.get("tags").map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .collect::<Vec<String>>()
        });

        let width = Self::parse_dimension(
            params.get("width"),
            params.get("width_min"),
            params.get("width_max"),
        );
        let height = Self::parse_dimension(
            params.get("height"),
            params.get("height_min"),
            params.get("height_max"),
        );
        let size = Self::parse_size(
            params.get("size"),
            params.get("size_min"),
            params.get("size_max"),
        );

        Self {
            tags,
            width,
            height,
            size,
        }
    }

    fn parse_dimension(
        exact: Option<&String>,
        min: Option<&String>,
        max: Option<&String>,
    ) -> Option<DimensionFilter> {
        if let Some(exact) = exact {
            exact.parse().ok().map(DimensionFilter::Exact)
        } else if let (Some(min), Some(max)) = (min, max) {
            match (min.parse(), max.parse()) {
                (Ok(min), Ok(max)) if min <= max => Some(DimensionFilter::Range(min, max)),
                _ => None,
            }
        } else {
            None
        }
    }

    fn parse_size(
        exact: Option<&String>,
        min: Option<&String>,
        max: Option<&String>,
    ) -> Option<SizeFilter> {
        if let Some(exact) = exact {
            exact.parse().ok().map(SizeFilter::Exact)
        } else if let (Some(min), Some(max)) = (min, max) {
            match (min.parse(), max.parse()) {
                (Ok(min), Ok(max)) if min <= max => Some(SizeFilter::Range(min, max)),
                _ => None,
            }
        } else {
            None
        }
    }
}
