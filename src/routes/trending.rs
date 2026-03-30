use axum::{
    Router,
    Json,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::Db;
use crate::models::post::Post;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/trending", get(trending))
        .route("/trending/keywords", get(trending_keywords))
}

#[derive(Debug, Deserialize)]
pub struct TrendingQuery {
    pub age_group: Option<String>,
    pub gender: Option<String>,
    pub region: Option<String>,
    pub category: Option<String>,
    pub period: Option<String>,
    pub limit: Option<i64>,
}

/// Trending posts based on all user interactions (reactions + impressions/dwell/clicks).
/// Weighted: like=3, review=4, bookmark=2, click=2, dwell=1 per second (capped at 5), impression=0.1
async fn trending(
    State(pool): State<Db>,
    Query(query): Query<TrendingQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let period_interval = match query.period.as_deref() {
        Some("day") => "1 day",
        Some("month") => "30 days",
        _ => "7 days",
    };

    let has_demo_filter = query.age_group.is_some() || query.gender.is_some() || query.region.is_some();

    let posts = if has_demo_filter {
        sqlx::query_as::<_, Post>(
            r#"SELECT p.*, COALESCE(eng.engagement, 0) AS _eng
               FROM posts p
               LEFT JOIN (
                 -- Weighted engagement from reactions
                 SELECT post_id, SUM(
                   CASE reaction_type
                     WHEN 'like' THEN 3
                     WHEN 'review' THEN 4
                     WHEN 'bookmark' THEN 2
                     ELSE 1
                   END
                 ) AS engagement
                 FROM reactions r
                 JOIN users u ON u.id = r.user_id
                 WHERE r.created_at > now() - $1::interval
                   AND ($2::text IS NULL OR age_group_from_birth(u.birth_date) = $2)
                   AND ($3::text IS NULL OR u.gender = $3)
                   AND ($4::text IS NULL OR u.region = $4)
                 GROUP BY post_id
               ) eng ON eng.post_id = p.id
               LEFT JOIN (
                 -- Weighted engagement from interactions (click/dwell/impression)
                 SELECT post_id, SUM(
                   CASE interaction_type
                     WHEN 'click' THEN 2
                     WHEN 'dwell' THEN LEAST(COALESCE(duration_ms, 0) / 1000.0, 5)
                     WHEN 'impression' THEN 0.1
                     ELSE 0
                   END
                 ) AS engagement
                 FROM interactions i
                 JOIN users u ON u.id = i.user_id
                 WHERE i.created_at > now() - $1::interval
                   AND ($2::text IS NULL OR age_group_from_birth(u.birth_date) = $2)
                   AND ($3::text IS NULL OR u.gender = $3)
                   AND ($4::text IS NULL OR u.region = $4)
                 GROUP BY post_id
               ) ieng ON ieng.post_id = p.id
               WHERE p.active = true
                 AND ($5::text IS NULL OR p.category = $5)
                 AND (COALESCE(eng.engagement, 0) + COALESCE(ieng.engagement, 0)) > 0
               ORDER BY (COALESCE(eng.engagement, 0) + COALESCE(ieng.engagement, 0)) DESC, p.score DESC
               LIMIT $6"#,
        )
        .bind(period_interval)
        .bind(&query.age_group)
        .bind(&query.gender)
        .bind(&query.region)
        .bind(&query.category)
        .bind(limit)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as::<_, Post>(
            r#"SELECT * FROM posts
               WHERE active = true
                 AND created_at > now() - $1::interval
                 AND ($2::text IS NULL OR category = $2)
               ORDER BY score DESC, like_count DESC, review_count DESC
               LIMIT $3"#,
        )
        .bind(period_interval)
        .bind(&query.category)
        .bind(limit)
        .fetch_all(&pool)
        .await
    }
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(json!({ "posts": posts })))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct KeywordTrend {
    keyword: String,
    count: i64,
}

async fn trending_keywords(
    State(pool): State<Db>,
    Query(query): Query<TrendingQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let period_interval = match query.period.as_deref() {
        Some("day") => "1 day",
        Some("month") => "30 days",
        _ => "7 days",
    };

    let has_demo_filter = query.age_group.is_some() || query.gender.is_some() || query.region.is_some();

    let keywords = if has_demo_filter {
        sqlx::query_as::<_, KeywordTrend>(
            r#"SELECT unnest(p.tags) AS keyword, COUNT(DISTINCT r.id) + COUNT(DISTINCT i.id) AS count
               FROM posts p
               LEFT JOIN reactions r ON r.post_id = p.id
                 AND r.created_at > now() - $1::interval
               LEFT JOIN users ru ON ru.id = r.user_id
                 AND ($2::text IS NULL OR age_group_from_birth(ru.birth_date) = $2)
                 AND ($3::text IS NULL OR ru.gender = $3)
                 AND ($4::text IS NULL OR ru.region = $4)
               LEFT JOIN interactions i ON i.post_id = p.id
                 AND i.created_at > now() - $1::interval
               LEFT JOIN users iu ON iu.id = i.user_id
                 AND ($2::text IS NULL OR age_group_from_birth(iu.birth_date) = $2)
                 AND ($3::text IS NULL OR iu.gender = $3)
                 AND ($4::text IS NULL OR iu.region = $4)
               WHERE p.active = true
                 AND (r.id IS NOT NULL OR i.id IS NOT NULL)
               GROUP BY keyword
               ORDER BY count DESC
               LIMIT $5"#,
        )
        .bind(period_interval)
        .bind(&query.age_group)
        .bind(&query.gender)
        .bind(&query.region)
        .bind(limit)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as::<_, KeywordTrend>(
            r#"SELECT unnest(p.tags) AS keyword, COUNT(DISTINCT r.id) AS count
               FROM posts p
               JOIN reactions r ON r.post_id = p.id
               WHERE p.active = true
                 AND r.created_at > now() - $1::interval
               GROUP BY keyword
               ORDER BY count DESC
               LIMIT $2"#,
        )
        .bind(period_interval)
        .bind(limit)
        .fetch_all(&pool)
        .await
    }
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(json!({ "keywords": keywords })))
}
