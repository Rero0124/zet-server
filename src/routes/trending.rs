use axum::{
    Router,
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::Db;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/trending/keywords", get(trending_keywords))
        .route("/trending/keywords/{keyword}", get(keyword_detail))
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

#[derive(Debug, Serialize, sqlx::FromRow)]
struct KeywordTrend {
    keyword: String,
    count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct TimePoint {
    date: String,
    count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct RegionInterest {
    region: String,
    count: i64,
}

/// Trending keywords with sparkline data (count per day over period)
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

    let has_demo = query.age_group.is_some() || query.gender.is_some() || query.region.is_some();

    // Get top keywords by interaction count
    let keywords = if has_demo {
        sqlx::query_as::<_, KeywordTrend>(
            r#"SELECT unnest(p.tags) AS keyword, COUNT(DISTINCT r.id) AS count
               FROM posts p
               JOIN reactions r ON r.post_id = p.id
               JOIN users u ON u.id = r.user_id
               WHERE p.active = true
                 AND r.created_at > now() - $1::interval
                 AND ($2::text IS NULL OR age_group_from_birth(u.birth_date) = $2)
                 AND ($3::text IS NULL OR u.gender = $3)
                 AND ($4::text IS NULL OR u.region = $4)
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
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // For each keyword, get daily time series for sparkline
    let mut results = Vec::new();
    for kw in &keywords {
        let series = get_keyword_series(&pool, &kw.keyword, period_interval).await;
        results.push(json!({
            "keyword": kw.keyword,
            "count": kw.count,
            "series": series,
        }));
    }

    Ok(Json(json!({ "keywords": results })))
}

/// Keyword detail: time series, regional interest, related topics, related searches
async fn keyword_detail(
    State(pool): State<Db>,
    Path(keyword): Path<String>,
    Query(query): Query<TrendingQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let period_interval = match query.period.as_deref() {
        Some("day") => "1 day",
        Some("week") => "7 days",
        _ => "30 days", // default to month for detail view
    };

    // Time series
    let series = get_keyword_series(&pool, &keyword, period_interval).await;

    // Regional interest
    let regions = sqlx::query_as::<_, RegionInterest>(
        r#"SELECT u.region AS region, COUNT(DISTINCT r.id) AS count
           FROM reactions r
           JOIN users u ON u.id = r.user_id
           JOIN posts p ON p.id = r.post_id
           WHERE p.active = true
             AND $1 = ANY(p.tags)
             AND r.created_at > now() - $2::interval
             AND u.region IS NOT NULL AND u.region != ''
           GROUP BY u.region
           ORDER BY count DESC
           LIMIT 10"#,
    )
    .bind(&keyword)
    .bind(period_interval)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Related topics: other tags that appear with this keyword
    let related_topics = sqlx::query_as::<_, KeywordTrend>(
        r#"SELECT tag AS keyword, COUNT(DISTINCT p.id) AS count
           FROM posts p, unnest(p.tags) AS tag
           WHERE p.active = true
             AND $1 = ANY(p.tags)
             AND p.created_at > now() - $2::interval
             AND tag != $1
           GROUP BY tag
           ORDER BY count DESC
           LIMIT 10"#,
    )
    .bind(&keyword)
    .bind(period_interval)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Related searches: keywords from posts that users who interacted with this keyword also interacted with
    let related_searches = sqlx::query_as::<_, KeywordTrend>(
        r#"SELECT unnest(p2.tags) AS keyword, COUNT(DISTINCT r2.user_id) AS count
           FROM reactions r1
           JOIN posts p1 ON p1.id = r1.post_id
           JOIN reactions r2 ON r2.user_id = r1.user_id AND r2.post_id != r1.post_id
           JOIN posts p2 ON p2.id = r2.post_id
           WHERE p1.active = true AND p2.active = true
             AND $1 = ANY(p1.tags)
             AND r1.created_at > now() - $2::interval
             AND NOT ($1 = ANY(p2.tags))
           GROUP BY keyword
           ORDER BY count DESC
           LIMIT 10"#,
    )
    .bind(&keyword)
    .bind(period_interval)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Total count
    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(DISTINCT r.id)
           FROM reactions r
           JOIN posts p ON p.id = r.post_id
           WHERE p.active = true AND $1 = ANY(p.tags)
             AND r.created_at > now() - $2::interval"#,
    )
    .bind(&keyword)
    .bind(period_interval)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    Ok(Json(json!({
        "keyword": keyword,
        "total_interactions": total,
        "period": query.period.as_deref().unwrap_or("month"),
        "series": series,
        "regions": regions,
        "related_topics": related_topics,
        "related_searches": related_searches,
    })))
}

async fn get_keyword_series(pool: &crate::db::Db, keyword: &str, period_interval: &str) -> Vec<TimePoint> {
    sqlx::query_as::<_, TimePoint>(
        r#"SELECT to_char(r.created_at::date, 'YYYY-MM-DD') AS date, COUNT(DISTINCT r.id) AS count
           FROM reactions r
           JOIN posts p ON p.id = r.post_id
           WHERE p.active = true
             AND $1 = ANY(p.tags)
             AND r.created_at > now() - $2::interval
           GROUP BY r.created_at::date
           ORDER BY r.created_at::date"#,
    )
    .bind(keyword)
    .bind(period_interval)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}
