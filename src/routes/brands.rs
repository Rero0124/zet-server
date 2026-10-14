use axum::{
    Router,
    Json,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/brands", get(list_brands))
}

#[derive(Debug, Deserialize)]
struct BrandsQuery {
    viewer_id: Option<Uuid>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct BrandSummary {
    id: Uuid,
    username: String,
    name: String,
    bio: Option<String>,
    avatar_url: Option<String>,
    post_count: i64,
    follower_count: i64,
    total_likes: i64,
    is_following: bool,
}

/// 활성 게시글이 1개 이상인 기업 계정 목록. 팔로워 → 좋아요 합 → 게시글 수 순.
async fn list_brands(
    State(pool): State<Db>,
    Query(query): Query<BrandsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(10).clamp(1, 30);

    let brands = sqlx::query_as::<_, BrandSummary>(
        r#"SELECT u.id, u.username, u.name, u.bio, u.avatar_url,
                  (SELECT COUNT(*) FROM posts p WHERE p.author_id = u.id AND p.active = true) AS post_count,
                  (SELECT COUNT(*) FROM follows f WHERE f.followee_id = u.id) AS follower_count,
                  (SELECT COALESCE(SUM(p.like_count), 0)::bigint FROM posts p WHERE p.author_id = u.id AND p.active = true) AS total_likes,
                  EXISTS(SELECT 1 FROM follows f WHERE f.follower_id = $1::uuid AND f.followee_id = u.id) AS is_following
           FROM users u
           WHERE u.role = 'business'
             AND EXISTS (SELECT 1 FROM posts p WHERE p.author_id = u.id AND p.active = true)
           ORDER BY follower_count DESC, total_likes DESC, post_count DESC, u.created_at ASC
           LIMIT $2"#,
    )
    .bind(&query.viewer_id)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "brands": brands })))
}
