use crate::models::{ImageResponse, PathType};
use anyhow::{anyhow, Result};
use image::{GenericImageView, ImageFormat};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::PathBuf;
use tracing::{error, info, warn};
use uuid::Uuid;

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB

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
                filename TEXT NOT NULL UNIQUE
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

    pub fn get_random_image(&self) -> Result<ImageResponse> {
        let conn = self.pool.get()?;
        let filename: String = conn.query_row(
            "SELECT filename FROM images ORDER BY RANDOM() LIMIT 1",
            [],
            |row| row.get(0),
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
        })
    }

    pub fn add_image(&self, path: &str, path_type: PathType) -> Result<()> {
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
                        let conn = self.pool.get()?;
                        conn.execute("INSERT INTO images (filename) VALUES (?)", [filename])?;
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
                error!("URL support not implemented yet");
                return Err(anyhow!("URL support not implemented yet"));
            }
        }
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
