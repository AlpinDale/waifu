use crate::models::PathType;
use anyhow::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::PathBuf;
use uuid::Uuid;

pub struct ImageStore {
    pool: Pool<SqliteConnectionManager>,
    images_dir: PathBuf,
    base_url: String,
}

impl ImageStore {
    pub fn new(db_path: &str, images_dir: PathBuf, base_url: String) -> Result<Self> {
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::new(manager)?;

        std::fs::create_dir_all(&images_dir)?;

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
            base_url,
        };

        store.sync_database()?;

        Ok(store)
    }

    fn sync_database(&self) -> Result<()> {
        let conn = self.pool.get()?;

        let entries = std::fs::read_dir(&self.images_dir)?;

        for entry in entries {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            conn.execute(
                "INSERT OR IGNORE INTO images (filename) VALUES (?)",
                [filename_str.as_ref()],
            )?;
        }

        Ok(())
    }

    pub fn get_random_image(&self) -> Result<String> {
        let conn = self.pool.get()?;
        let filename: String = conn.query_row(
            "SELECT filename FROM images ORDER BY RANDOM() LIMIT 1",
            [],
            |row| row.get(0),
        )?;

        Ok(format!("{}/{}", self.base_url, filename))
    }

    pub fn add_image(&self, path: &str, path_type: PathType) -> Result<()> {
        match path_type {
            PathType::Local => {
                let src_path = std::path::Path::new(path);
                if !src_path.exists() {
                    return Err(anyhow::anyhow!("Local file not found: {}", path));
                }

                let ext = src_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let filename = format!("{}.{}", Uuid::new_v4(), ext);
                let dest_path = self.images_dir.join(&filename);

                std::fs::copy(path, &dest_path)?;

                let conn = self.pool.get()?;
                conn.execute("INSERT INTO images (filename) VALUES (?)", [filename])?;
            }
            PathType::Url => {
                return Err(anyhow::anyhow!("URL support not implemented yet"));
            }
        }

        Ok(())
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
