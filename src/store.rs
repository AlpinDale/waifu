use crate::models::{ImageResponse, PathType};
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use image::{GenericImageView, ImageFormat};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};
use url::Url;
use uuid::Uuid;

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: u32 = 5;

// Allowed content types for images
const ALLOWED_CONTENT_TYPES: [&str; 7] = [
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/x-ms-bmp",      // Some servers use this for BMP
    "binary/octet-stream", // Some servers don't set proper content type
];

const BLOCKED_URL_PATTERNS: [&str; 12] = [
    "localhost",
    "127.",
    "0.0.0.0",
    // Private network ranges (RFC 1918)
    "10.",
    "172.16.",
    "172.17.",
    "172.18.",
    "172.19.",
    "172.20.",
    "172.21.",
    "192.168.",
    // Link-local addresses (RFC 3927)
    "169.254.",
];

const BLOCKED_HOSTNAMES: [&str; 4] = [
    "metadata.google.internal",     // Google Cloud
    "169.254.169.254",              // AWS
    "metadata.azure.internal",      // Azure
    "metadata.platformequinix.com", // Equinix Metal
];

pub struct ImageStore {
    pool: Pool<SqliteConnectionManager>,
    images_dir: PathBuf,
    base_url: String,
}

impl ImageStore {
    pub fn new(
        db_path: &str,
        images_dir: PathBuf,
        host: String,
        port: u16,
        images_path: String,
    ) -> Result<Self> {
        info!("Initializing ImageStore with database at {}", db_path);
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::new(manager)?;

        std::fs::create_dir_all(&images_dir)?;
        info!("Ensuring images directory exists at {:?}", images_dir);

        let conn = pool.get()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS images (
                id INTEGER PRIMARY KEY,
                filename TEXT NOT NULL UNIQUE,
                hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )?;

        let store = Self {
            pool,
            images_dir,
            base_url: format!("http://{}:{}{}", host, port, images_path),
        };

        info!("Syncing database with existing images...");
        store.sync_database()?;

        Ok(store)
    }

    fn sync_database(&self) -> Result<()> {
        let conn = self.pool.get()?;
        let entries = std::fs::read_dir(&self.images_dir)?;
        let mut count = 0;

        for entry in entries {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            match conn.execute(
                "INSERT OR IGNORE INTO images (filename) VALUES (?)",
                [filename_str.as_ref()],
            ) {
                Ok(_) => count += 1,
                Err(e) => warn!("Failed to sync file {}: {}", filename_str, e),
            }
        }

        info!("Synced {} images with database", count);
        Ok(())
    }

    fn calculate_file_hash(path: &std::path::Path) -> Result<String> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 1024];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn get_random_image(&self) -> Result<ImageResponse> {
        let conn = self.pool.get()?;
        let (filename, hash, created_at, modified_at): (String, String, String, String) = conn
            .query_row(
            "SELECT filename, hash, created_at, modified_at FROM images ORDER BY RANDOM() LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        let file_path = self.images_dir.join(&filename);

        let metadata = std::fs::metadata(&file_path)?;

        let img = image::open(&file_path)?;
        let dimensions = img.dimensions();

        let format = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        Ok(ImageResponse {
            url: format!("{}/{}", self.base_url, filename),
            filename,
            format,
            width: dimensions.0,
            height: dimensions.1,
            size_bytes: metadata.len(),
            hash,
            created_at: OffsetDateTime::parse(&created_at, &Rfc3339)?,
            modified_at: OffsetDateTime::parse(&modified_at, &Rfc3339)?,
        })
    }

    async fn validate_url(&self, url: &str) -> Result<Url> {
        let parsed_url = Url::parse(url).map_err(|e| anyhow!("Invalid URL: {}", e))?;

        if !["http", "https"].contains(&parsed_url.scheme()) {
            return Err(anyhow!("Only HTTP(S) URLs are supported"));
        }

        let host_str = parsed_url.host_str().unwrap_or_default();

        for pattern in BLOCKED_URL_PATTERNS {
            if host_str.contains(pattern) {
                return Err(anyhow!("URL contains blocked pattern: {}", pattern));
            }
        }

        for hostname in BLOCKED_HOSTNAMES {
            if host_str.eq_ignore_ascii_case(hostname) {
                return Err(anyhow!("URL hostname is blocked: {}", hostname));
            }
        }

        if let Some(port) = parsed_url.port() {
            match port {
                22 | 23 | 25 | 445 | 3306 | 5432 | 27017 => {
                    return Err(anyhow!("Port {} is not allowed", port));
                }
                _ => {}
            }
        }

        Ok(parsed_url)
    }

    async fn check_content_type(&self, client: &reqwest::Client, url: &Url) -> Result<()> {
        info!("Checking content type for URL: {}", url);

        let response = client.head(url.as_str()).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("URL returned status code: {}", response.status()));
        }

        if let Some(length) = response.content_length() {
            if length > MAX_FILE_SIZE {
                return Err(anyhow!(
                    "File too large: {} bytes (max {} bytes)",
                    length,
                    MAX_FILE_SIZE
                ));
            }
            info!("Content length: {} bytes", length);
        }

        if let Some(content_type) = response.headers().get("content-type") {
            let content_type = content_type
                .to_str()
                .map_err(|_| anyhow!("Invalid content type header"))?
                .to_lowercase();

            if !ALLOWED_CONTENT_TYPES
                .iter()
                .any(|&t| content_type.contains(t))
            {
                return Err(anyhow!("Unsupported content type: {}", content_type));
            }
            info!("Content type: {}", content_type);
        } else {
            warn!("No content type header present");
        }

        Ok(())
    }

    async fn download_image(&self, url: &str) -> Result<PathBuf> {
        let url = self.validate_url(url).await?;

        let client = reqwest::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS as usize))
            .build()?;

        self.check_content_type(&client, &url).await?;

        let temp_path = self.images_dir.join(format!("temp_{}", Uuid::new_v4()));
        info!("Downloading to temporary file: {:?}", temp_path);

        let response = client.get(url.as_str()).send().await?;

        let mut file = tokio::fs::File::create(&temp_path).await?;
        let mut downloaded_size: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            downloaded_size += chunk.len() as u64;

            if downloaded_size > MAX_FILE_SIZE {
                file.shutdown().await?;
                tokio::fs::remove_file(&temp_path).await?;
                return Err(anyhow!(
                    "File too large: {} bytes (max {} bytes)",
                    downloaded_size,
                    MAX_FILE_SIZE
                ));
            }

            file.write_all(&chunk).await?;
        }

        file.shutdown().await?;
        info!("Download completed: {} bytes", downloaded_size);

        Ok(temp_path)
    }

    pub async fn add_image(&self, path: &str, path_type: PathType) -> Result<()> {
        match path_type {
            PathType::Local => {
                let src_path = std::path::Path::new(path);
                info!("Validating local file: {}", path);

                if !src_path.exists() {
                    error!("File not found: {}", path);
                    return Err(anyhow!("Local file not found: {}", path));
                }

                let metadata = std::fs::metadata(src_path)?;
                let size_mb = metadata.len() as f64 / 1024.0 / 1024.0;
                info!("File size: {:.2} MiB", size_mb);

                if metadata.len() > MAX_FILE_SIZE {
                    error!(
                        "File too large: {:.2} MiB (max {:.2} MiB)",
                        size_mb,
                        MAX_FILE_SIZE as f64 / 1024.0 / 1024.0
                    );
                    return Err(anyhow!(
                        "File too large: {} bytes (max {} bytes)",
                        metadata.len(),
                        MAX_FILE_SIZE
                    ));
                }

                info!("Checking image format...");
                let img_file = std::fs::File::open(src_path)?;
                let format = image::io::Reader::new(std::io::BufReader::new(img_file))
                    .with_guessed_format()?
                    .format();

                let format = match format {
                    Some(fmt) => match fmt {
                        ImageFormat::Png
                        | ImageFormat::Jpeg
                        | ImageFormat::Gif
                        | ImageFormat::WebP
                        | ImageFormat::Bmp => {
                            info!("Detected image format: {:?}", fmt);
                            fmt
                        }
                        unsupported => {
                            error!("Unsupported image format: {:?}", unsupported);
                            return Err(anyhow!("Unsupported image format: {:?}", unsupported));
                        }
                    },
                    None => {
                        error!("Could not determine image format");
                        return Err(anyhow!("Could not determine image format"));
                    }
                };

                let ext = format.extensions_str()[0];
                let filename = format!("{}.{}", Uuid::new_v4(), ext);
                let dest_path = self.images_dir.join(&filename);

                info!("Copying file to: {:?}", dest_path);
                std::fs::copy(path, &dest_path)?;

                info!("Verifying image integrity...");
                match image::open(&dest_path) {
                    Ok(img) => {
                        let dimensions = img.dimensions();
                        info!(
                            "Successfully validated image: {} ({}x{} pixels, format: {:?})",
                            filename, dimensions.0, dimensions.1, format
                        );
                        let now = OffsetDateTime::now_utc();
                        let now_str = now.format(&Rfc3339)?;
                        let hash = Self::calculate_file_hash(&dest_path)?;

                        info!("File hash: {}", hash);

                        let conn = self.pool.get()?;
                        conn.execute(
                            "INSERT INTO images (filename, hash, created_at, modified_at) 
                             VALUES (?, ?, ?, ?)",
                            [&filename, &hash, &now_str, &now_str],
                        )?;
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to validate image: {}", e);
                        if dest_path.exists() {
                            warn!("Cleaning up invalid file: {:?}", dest_path);
                            let _ = std::fs::remove_file(&dest_path);
                        }
                        Err(anyhow!("Invalid image file: {}", e))
                    }
                }
            }
            PathType::Url => {
                info!("Processing URL: {}", path);
                let temp_path = self.download_image(path).await?;

                // Validate image format
                info!("Checking image format...");
                let format = image::io::Reader::new(std::io::BufReader::new(std::fs::File::open(
                    &temp_path,
                )?))
                .with_guessed_format()?
                .format();

                let format = match format {
                    Some(fmt) => match fmt {
                        ImageFormat::Png
                        | ImageFormat::Jpeg
                        | ImageFormat::Gif
                        | ImageFormat::WebP
                        | ImageFormat::Bmp => {
                            info!("Detected image format: {:?}", fmt);
                            fmt
                        }
                        unsupported => {
                            tokio::fs::remove_file(&temp_path).await?;
                            error!("Unsupported image format: {:?}", unsupported);
                            return Err(anyhow!("Unsupported image format: {:?}", unsupported));
                        }
                    },
                    None => {
                        tokio::fs::remove_file(&temp_path).await?;
                        error!("Could not determine image format");
                        return Err(anyhow!("Could not determine image format"));
                    }
                };

                let ext = format.extensions_str()[0];
                let filename = format!("{}.{}", Uuid::new_v4(), ext);
                let dest_path = self.images_dir.join(&filename);

                tokio::fs::rename(&temp_path, &dest_path).await?;

                info!("Verifying image integrity...");
                match image::open(&dest_path) {
                    Ok(img) => {
                        let dimensions = img.dimensions();
                        info!(
                            "Successfully validated image: {} ({}x{} pixels, format: {:?})",
                            filename, dimensions.0, dimensions.1, format
                        );

                        let now = OffsetDateTime::now_utc();
                        let now_str = now.format(&Rfc3339)?;
                        let hash = Self::calculate_file_hash(&dest_path)?;

                        info!("File hash: {}", hash);

                        let conn = self.pool.get()?;
                        conn.execute(
                            "INSERT INTO images (filename, hash, created_at, modified_at) 
                             VALUES (?, ?, ?, ?)",
                            [&filename, &hash, &now_str, &now_str],
                        )?;

                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to validate image: {}", e);
                        tokio::fs::remove_file(&dest_path).await?;
                        Err(anyhow!("Invalid image file: {}", e))
                    }
                }
            }
        }
    }

    pub fn get_image_by_filename(&self, filename: &str) -> Result<ImageResponse> {
        let conn = self.pool.get()?;
        let (hash, created_at, modified_at): (String, String, String) = conn.query_row(
            "SELECT hash, created_at, modified_at FROM images WHERE filename = ?",
            [filename],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        let file_path = self.images_dir.join(filename);

        let metadata = std::fs::metadata(&file_path)?;

        let img = image::open(&file_path)?;
        let dimensions = img.dimensions();

        let format = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        Ok(ImageResponse {
            url: format!("{}/{}", self.base_url, filename),
            filename: filename.to_string(),
            format,
            width: dimensions.0,
            height: dimensions.1,
            size_bytes: metadata.len(),
            hash,
            created_at: OffsetDateTime::parse(&created_at, &Rfc3339)?,
            modified_at: OffsetDateTime::parse(&modified_at, &Rfc3339)?,
        })
    }
}

impl Clone for ImageStore {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            images_dir: self.images_dir.clone(),
            base_url: self.base_url.clone(),
        }
    }
}
