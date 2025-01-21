use anyhow::Result;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::path::PathBuf;
use uuid::Uuid;
use warp::{Filter, Rejection, Reply};

#[derive(Serialize)]
struct ImageResponse {
    url: String,
}

#[derive(Deserialize)]
struct AddImageRequest {
    path: String,
    #[serde(rename = "type")]
    path_type: PathType,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PathType {
    Url,
    Local,
}

#[derive(Debug)]
enum ImageError {
    PathNotFound(String),
    DatabaseError(String),
    FileError(String),
}

impl warp::reject::Reject for ImageError {}

struct ImageStore {
    pool: Pool<SqliteConnectionManager>,
    images_dir: PathBuf,
    base_url: String,
}

impl ImageStore {
    fn new(db_path: &str, images_dir: PathBuf, base_url: String) -> Result<Self> {
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::new(manager)?;

        std::fs::create_dir_all(&images_dir)?;

        let conn = pool.get()?;
        conn.execute("DROP TABLE IF EXISTS images", [])?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS images (
                id INTEGER PRIMARY KEY,
                filename TEXT NOT NULL UNIQUE
            )",
            [],
        )?;
        
        Ok(Self { pool, images_dir, base_url })
    }

    fn get_random_image(&self) -> Result<String> {
        let conn = self.pool.get()?;
        let filename: String = conn.query_row(
            "SELECT filename FROM images ORDER BY RANDOM() LIMIT 1",
            [],
            |row| row.get(0),
        )?;

        Ok(format!("{}/{}", self.base_url, filename))
    }

    fn add_image(&self, path: &str, path_type: PathType) -> Result<()> {
        match path_type {
            PathType::Local => {
                let src_path = std::path::Path::new(path);
                if !src_path.exists() {
                    return Err(anyhow::anyhow!("Local file not found: {}", path));
                }

                let ext = src_path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let filename = format!("{}.{}", Uuid::new_v4(), ext);
                let dest_path = self.images_dir.join(&filename);

                std::fs::copy(path, &dest_path)?;

                let conn = self.pool.get()?;
                conn.execute(
                    "INSERT INTO images (filename) VALUES (?)",
                    [filename],
                )?;
            },
            PathType::Url => {
                // TODO: implement this
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

// GET /random
async fn get_random_image_handler(store: ImageStore) -> Result<impl Reply, Rejection> {
    match store.get_random_image() {
        Ok(url) => Ok(warp::reply::json(&ImageResponse { url })),
        Err(_) => Err(warp::reject::not_found()),
    }
}

// POST /image
async fn add_image_handler(
    store: ImageStore,
    body: AddImageRequest,
) -> Result<impl Reply, Rejection> {
    match store.add_image(&body.path, body.path_type) {
        Ok(_) => Ok(warp::reply::with_status(
            "Image added successfully",
            warp::http::StatusCode::CREATED,
        )),
        Err(e) => {
            if e.to_string().contains("not found") {
                Err(warp::reject::custom(ImageError::PathNotFound(e.to_string())))
            } else {
                Err(warp::reject::custom(ImageError::DatabaseError(e.to_string())))
            }
        }
    }
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (warp::http::StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(e) = err.find::<ImageError>() {
        match e {
            ImageError::PathNotFound(msg) => (
                warp::http::StatusCode::BAD_REQUEST,
                msg.clone(),
            ),
            ImageError::DatabaseError(msg) => (
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                msg.clone(),
            ),
            ImageError::FileError(msg) => (
                warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                msg.clone(),
            ),
        }
    } else {
        (
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    Ok(warp::reply::with_status(message, code))
}

#[tokio::main]
async fn main() -> Result<()> {
    let images_dir = PathBuf::from("images");
    // TODO: get from env
    let base_url = "http://localhost:8000/images";

    let store = ImageStore::new("images.db", images_dir.clone(), base_url.to_string())?;
    let store = warp::any().map(move || store.clone());

    let random = warp::path("random")
        .and(warp::get())
        .and(store.clone())
        .and_then(get_random_image_handler);

    let add_image = warp::path("image")
        .and(warp::post())
        .and(store)
        .and(warp::body::json())
        .and_then(add_image_handler);

    let images = warp::path("images")
        .and(warp::fs::dir("images"));

    let routes = random
        .or(add_image)
        .or(images)
        .recover(handle_rejection);

    println!("Server started at http://localhost:8000");
    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;

    Ok(())
}