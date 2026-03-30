use axum::{
    Router,
    Json,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::models::post::Post;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/feed", get(feed))
}

#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    pub user_id: Option<Uuid>,
    pub category: Option<String>,
    pub sort: Option<String>,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

async fn feed(
    State(pool): State<Db>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let cursor = query.cursor.unwrap_or(0);

    let (w_demo, w_recency, w_popular, w_affinity) = match query.sort.as_deref() {
        Some("latest") => (1.0_f64, 5.0_f64, 1.0_f64, 1.0_f64),
        Some("popular") => (1.0_f64, 1.0_f64, 5.0_f64, 1.0_f64),
        _ => (3.0_f64, 1.0_f64, 1.0_f64, 3.0_f64),
    };

    let posts = sqlx::query_as::<_, Post>(
        r#"SELECT p.* FROM posts p
           LEFT JOIN users u ON u.id = $1
           -- Category affinity: sum of user's past interactions per category
           LEFT JOIN (
             SELECT category, SUM(cnt) AS cnt FROM (
               SELECT p2.category, COUNT(*) AS cnt
               FROM interactions i2
               JOIN posts p2 ON p2.id = i2.post_id
               WHERE i2.user_id = $1
               GROUP BY p2.category
               UNION ALL
               SELECT p3.category, COUNT(*) * 2 AS cnt
               FROM reactions r2
               JOIN posts p3 ON p3.id = r2.post_id
               WHERE r2.user_id = $1
               GROUP BY p3.category
             ) sub
             GROUP BY category
           ) aff ON aff.category = p.category
           WHERE p.active = true
             AND ($2::text IS NULL OR p.category = $2)
           ORDER BY
             $3 * (
               (CASE WHEN u.birth_date IS NOT NULL AND p.target_age = age_group_from_birth(u.birth_date) THEN 3.0 ELSE 0.0 END)
               + (CASE WHEN u.gender IS NOT NULL AND p.target_gender = u.gender THEN 2.0 ELSE 0.0 END)
               + (CASE WHEN u.region IS NOT NULL AND p.target_region = u.region THEN 2.0 ELSE 0.0 END)
             )
             + $4 * GREATEST(0, 10.0 - EXTRACT(EPOCH FROM (now() - p.created_at)) / 60480)
             + $5 * (p.score + LN(GREATEST(p.like_count, 1)) + LN(GREATEST(p.review_count, 1)))
             + $6 * LN(GREATEST(COALESCE(aff.cnt, 0), 1))
           DESC,
           p.created_at DESC
           LIMIT $7 OFFSET $8"#,
    )
    .bind(&query.user_id)
    .bind(&query.category)
    .bind(w_demo)
    .bind(w_recency)
    .bind(w_popular)
    .bind(w_affinity)
    .bind(limit)
    .bind(cursor)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(json!({
        "posts": posts,
        "next_cursor": cursor + limit
    })))
}
