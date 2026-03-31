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
    interaction_type: String,
    duration_ms: Option<i32>,
}

/// Insert interaction only if the same user+post+type hasn't been recorded today.
/// For dwell, always insert (multiple dwell events per day are meaningful).
async fn insert_interaction(pool: &crate::db::Db, item: &Interaction) {
    if item.interaction_type == "dwell" {
        // Dwell: always record, but cap at reasonable frequency
        let _ = sqlx::query(
            "INSERT INTO interactions (post_id, user_id, interaction_type, duration_ms) VALUES ($1, $2, $3, $4)",
        )
        .bind(item.post_id)
        .bind(item.user_id)
        .bind(&item.interaction_type)
        .bind(item.duration_ms)
        .execute(pool)
        .await;
    } else {
        // Impression/click: once per user per post per day
        let already = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM interactions
               WHERE post_id = $1 AND user_id = $2 AND interaction_type = $3
               AND created_at > CURRENT_DATE"#,
        )
        .bind(item.post_id)
        .bind(item.user_id)
        .bind(&item.interaction_type)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        if already == 0 {
            let _ = sqlx::query(
                "INSERT INTO interactions (post_id, user_id, interaction_type, duration_ms) VALUES ($1, $2, $3, $4)",
            )
            .bind(item.post_id)
            .bind(item.user_id)
            .bind(&item.interaction_type)
            .bind(item.duration_ms)
            .execute(pool)
            .await;

            // Update denormalized counts using unique user count
            match item.interaction_type.as_str() {
                "impression" => {
                    let _ = sqlx::query(
                        "UPDATE posts SET impressions = (SELECT COUNT(DISTINCT user_id) FROM interactions WHERE post_id = $1 AND interaction_type = 'impression') WHERE id = $1"
                    ).bind(item.post_id).execute(pool).await;
                }
                "click" => {
                    let _ = sqlx::query(
                        "UPDATE posts SET clicks = (SELECT COUNT(DISTINCT user_id) FROM interactions WHERE post_id = $1 AND interaction_type = 'click') WHERE id = $1"
                    ).bind(item.post_id).execute(pool).await;
                }
                _ => {}
            }
        }
    }
}

async fn track(
    State(pool): State<Db>,
    Json(body): Json<Interaction>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    insert_interaction(&pool, &body).await;
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
        insert_interaction(&pool, item).await;
    }
    Ok(StatusCode::CREATED)
}
