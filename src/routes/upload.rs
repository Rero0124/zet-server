use axum::{
    Router,
    Json,
    extract::State,
    http::StatusCode,
    routing::post,
};
use axum_extra::extract::Multipart;
use serde_json::{json, Value};
use uuid::Uuid;
use std::sync::Arc;

use crate::storage::Storage;

type AppState = (crate::db::Db, Arc<dyn Storage>);

const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50MB upload limit
const MAX_IMAGE_DIMENSION: u32 = 1920; // Downscale images larger than this

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload))
}

async fn upload(
    State((_pool, storage)): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut urls: Vec<String> = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
    })? {
        let content_type = field.content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        if !content_type.starts_with("image/") && !content_type.starts_with("video/") {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "이미지 또는 동영상만 업로드 가능합니다"}))));
        }

        let original_name = field.file_name()
            .unwrap_or("file")
            .to_string();

        let data = field.bytes().await.map_err(|e| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
        })?;

        if data.len() > MAX_FILE_SIZE {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "파일 크기는 50MB 이하여야 합니다"}))));
        }

        let is_image = content_type.starts_with("image/");

        // Process image: downscale if too large
        let (final_data, final_ext, final_content_type) = if is_image {
            match downscale_image(&data, &content_type) {
                Ok((processed, ext, ct)) => (processed, ext, ct),
                Err(_) => {
                    // If image processing fails, store original
                    let ext = original_name.rsplit('.').next().unwrap_or("jpg").to_string();
                    (data.to_vec(), ext, content_type.clone())
                }
            }
        } else {
            let ext = original_name.rsplit('.').next().unwrap_or("mp4").to_string();
            (data.to_vec(), ext, content_type.clone())
        };

        let filename = format!("{}.{}", Uuid::new_v4(), final_ext);

        let url = storage.upload(&filename, &final_data, &final_content_type).await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e})))
        })?;

        urls.push(url);
    }

    if urls.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "파일이 없습니다"}))));
    }

    Ok(Json(json!({ "urls": urls })))
}

fn downscale_image(data: &[u8], content_type: &str) -> Result<(Vec<u8>, String, String), String> {
    let img = image::load_from_memory(data).map_err(|e| e.to_string())?;

    let (w, h) = (img.width(), img.height());

    let img = if w > MAX_IMAGE_DIMENSION || h > MAX_IMAGE_DIMENSION {
        img.resize(
            MAX_IMAGE_DIMENSION,
            MAX_IMAGE_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    // Encode as WebP if possible, fallback to JPEG
    let mut buf = Vec::new();
    let (ext, ct) = if content_type == "image/png" && w <= MAX_IMAGE_DIMENSION && h <= MAX_IMAGE_DIMENSION {
        // Keep PNG if small enough and already PNG
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        ("png".to_string(), "image/png".to_string())
    } else {
        // Convert to JPEG for compression
        let mut cursor = std::io::Cursor::new(&mut buf);
        img.write_to(&mut cursor, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
        ("jpg".to_string(), "image/jpeg".to_string())
    };

    Ok((buf, ext, ct))
}
