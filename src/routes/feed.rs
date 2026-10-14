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
use crate::models::post::{PostView, post_view_select};

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
    /// true 면 뷰어가 팔로우한 작성자의 게시글만
    pub following: Option<bool>,
}

async fn feed(
    State(pool): State<Db>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let cursor = query.cursor.unwrap_or(0);
    let following_only = query.following.unwrap_or(false);

    // 팔로잉 피드는 뷰어가 있어야 의미가 있다
    if following_only && query.user_id.is_none() {
        return Ok(Json(json!({ "posts": [], "next_cursor": cursor })));
    }

    let (w_demo, w_recency, w_popular, w_affinity, w_follow) = match query.sort.as_deref() {
        Some("latest") => (1.0_f64, 5.0_f64, 1.0_f64, 1.0_f64, 0.5_f64),
        Some("popular") => (1.0_f64, 1.0_f64, 5.0_f64, 1.0_f64, 0.5_f64),
        _ => (3.0_f64, 1.0_f64, 1.0_f64, 3.0_f64, 2.0_f64),
    };

    let sql = format!(
        r#"SELECT {select} FROM posts p
           LEFT JOIN users au ON au.id = p.author_id
           LEFT JOIN users u ON u.id = $1
           LEFT JOIN follows f ON f.follower_id = $1 AND f.followee_id = p.author_id
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
             AND ($9::bool = false OR f.follower_id IS NOT NULL)
           ORDER BY
             $3 * (
               (CASE WHEN u.birth_date IS NOT NULL AND p.target_age = age_group_from_birth(u.birth_date) THEN 3.0 ELSE 0.0 END)
               + (CASE WHEN u.gender IS NOT NULL AND p.target_gender = u.gender THEN 2.0 ELSE 0.0 END)
               + (CASE WHEN u.region IS NOT NULL AND p.target_region = u.region THEN 2.0 ELSE 0.0 END)
             )
             + $4 * GREATEST(0, 10.0 - EXTRACT(EPOCH FROM (now() - p.created_at)) / 60480)
             + $5 * (p.score + LN(GREATEST(p.like_count, 1)) + LN(GREATEST(p.review_count, 1)))
             + $6 * LN(GREATEST(COALESCE(aff.cnt, 0), 1))
             + $10 * (CASE WHEN f.follower_id IS NOT NULL THEN 3.0 ELSE 0.0 END)
           DESC,
           p.created_at DESC
           LIMIT $7 OFFSET $8"#,
        select = post_view_select("$1::uuid"),
    );

    let posts = sqlx::query_as::<_, PostView>(&sql)
        .bind(&query.user_id)
        .bind(&query.category)
        .bind(w_demo)
        .bind(w_recency)
        .bind(w_popular)
        .bind(w_affinity)
        .bind(limit)
        .bind(cursor)
        .bind(following_only)
        .bind(w_follow)
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
