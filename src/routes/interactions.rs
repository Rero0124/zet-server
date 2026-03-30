use axum::{
    Router,
    Json,
    extract::State,
    http::StatusCode,
    routing::post,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/interactions", post(track))
        .route("/interactions/batch", post(track_batch))
}

#[derive(Debug, Deserialize)]
struct Interaction {
    post_id: Uuid,
    user_id: Uuid,
    interaction_type: String, // impression, dwell, click
    duration_ms: Option<i32>,
}

async fn track(
    State(pool): State<Db>,
    Json(body): Json<Interaction>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    sqlx::query(
        "INSERT INTO interactions (post_id, user_id, interaction_type, duration_ms) VALUES ($1, $2, $3, $4)",
    )
    .bind(body.post_id)
    .bind(body.user_id)
    .bind(&body.interaction_type)
    .bind(body.duration_ms)
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;

    // Update impression/click counts on post
    match body.interaction_type.as_str() {
        "impression" => {
            let _ = sqlx::query("UPDATE posts SET impressions = impressions + 1 WHERE id = $1")
                .bind(body.post_id).execute(&pool).await;
        }
        "click" => {
            let _ = sqlx::query("UPDATE posts SET clicks = clicks + 1 WHERE id = $1")
                .bind(body.post_id).execute(&pool).await;
        }
        _ => {}
    }

    Ok(StatusCode::CREATED)
}

#[derive(Debug, Deserialize)]
struct BatchInteractions {
    interactions: Vec<Interaction>,
}

async fn track_batch(
    State(pool): State<Db>,
    Json(body): Json<BatchInteractions>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    for item in &body.interactions {
        let _ = sqlx::query(
            "INSERT INTO interactions (post_id, user_id, interaction_type, duration_ms) VALUES ($1, $2, $3, $4)",
        )
        .bind(item.post_id)
        .bind(item.user_id)
        .bind(&item.interaction_type)
        .bind(item.duration_ms)
        .execute(&pool)
        .await;

        match item.interaction_type.as_str() {
            "impression" => {
                let _ = sqlx::query("UPDATE posts SET impressions = impressions + 1 WHERE id = $1")
                    .bind(item.post_id).execute(&pool).await;
            }
            "click" => {
                let _ = sqlx::query("UPDATE posts SET clicks = clicks + 1 WHERE id = $1")
                    .bind(item.post_id).execute(&pool).await;
            }
            _ => {}
        }
    }

    Ok(StatusCode::CREATED)
}
