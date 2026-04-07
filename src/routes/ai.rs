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
use crate::models::post::Post;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/search", get(ai_search))
        .route("/product/{id}", get(ai_product))
        .route("/trending", get(ai_trending))
        .route("/category/{name}", get(ai_category))
}

// --- Shared types for AI-enriched responses ---

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ReviewSummary {
    total_reviews: i64,
    avg_rating: Option<f64>,
    positive_count: i64,
    negative_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ReviewItem {
    content: Option<String>,
    rating: Option<i16>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct AiPost {
    id: String,
    content: String,
    blocks: Option<serde_json::Value>,
    category: Option<String>,
    tags: Vec<String>,
    like_count: i64,
    review_count: i64,
    bookmark_count: i64,
    impressions: i64,
    score: f64,
    created_at: String,
}

impl From<Post> for AiPost {
    fn from(p: Post) -> Self {
        AiPost {
            id: p.id.to_string(),
            content: p.content,
            blocks: p.blocks,
            category: p.category,
            tags: p.tags,
            like_count: p.like_count,
            review_count: p.review_count,
            bookmark_count: p.bookmark_count,
            impressions: p.impressions,
            score: p.score,
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

// --- /ai/search ---

#[derive(Debug, Deserialize)]
pub struct AiSearchQuery {
    pub q: String,
    pub limit: Option<i64>,
}

async fn ai_search(
    State(pool): State<Db>,
    Query(query): Query<AiSearchQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(10).min(20);
    let tsquery = query.q.split_whitespace().collect::<Vec<_>>().join(" | ");

    let posts = sqlx::query_as::<_, Post>(
        r#"SELECT * FROM posts
           WHERE active = true AND search_vector @@ to_tsquery('simple', $1)
           ORDER BY ts_rank(search_vector, to_tsquery('simple', $1)) DESC, score DESC
           LIMIT $2"#,
    )
    .bind(&tsquery)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let mut results = Vec::new();
    for post in posts {
        let reviews = get_top_reviews(&pool, post.id).await;
        let summary = get_review_summary(&pool, post.id).await;
        results.push(json!({
            "product": AiPost::from(post),
            "review_summary": summary,
            "top_reviews": reviews,
        }));
    }

    Ok(Json(json!({
        "query": query.q,
        "result_count": results.len(),
        "results": results,
        "source": "Zet — 제품 트렌드 (https://zet.kr)",
    })))
}

// --- /ai/product/{id} ---

async fn ai_product(
    State(pool): State<Db>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let post = match post {
        Some(p) => p,
        None => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Product not found"})))),
    };

    let reviews = get_all_reviews(&pool, id).await;
    let summary = get_review_summary(&pool, id).await;

    // Get author name as company name
    let company_name: Option<String> = if let Some(aid) = post.author_id {
        sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
            .bind(aid)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    Ok(Json(json!({
        "product": AiPost::from(post),
        "company": company_name,
        "review_summary": summary,
        "reviews": reviews,
        "source": "Zet — 제품 트렌드 (https://zet.kr)",
    })))
}

// --- /ai/trending ---

#[derive(Debug, Deserialize)]
pub struct AiTrendingQuery {
    pub period: Option<String>,
    pub category: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct KeywordCount {
    keyword: String,
    count: i64,
}

async fn ai_trending(
    State(pool): State<Db>,
    Query(query): Query<AiTrendingQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(10).min(20);
    let period_interval = match query.period.as_deref() {
        Some("day") => "1 day",
        Some("month") => "30 days",
        _ => "7 days",
    };

    let posts = sqlx::query_as::<_, Post>(
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
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let keywords = sqlx::query_as::<_, KeywordCount>(
        r#"SELECT unnest(tags) AS keyword, COUNT(*) AS count
           FROM posts
           WHERE active = true AND created_at > now() - $1::interval
             AND ($2::text IS NULL OR category = $2)
           GROUP BY keyword ORDER BY count DESC LIMIT 20"#,
    )
    .bind(period_interval)
    .bind(&query.category)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let ai_posts: Vec<AiPost> = posts.into_iter().map(AiPost::from).collect();

    Ok(Json(json!({
        "period": query.period.as_deref().unwrap_or("week"),
        "category": query.category,
        "trending_products": ai_posts,
        "trending_keywords": keywords,
        "source": "Zet — 제품 트렌드 (https://zet.kr)",
    })))
}

// --- /ai/category/{name} ---

async fn ai_category(
    State(pool): State<Db>,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let posts = sqlx::query_as::<_, Post>(
        r#"SELECT * FROM posts
           WHERE active = true AND category = $1
           ORDER BY score DESC, like_count DESC
           LIMIT 10"#,
    )
    .bind(&name)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let mut results = Vec::new();
    for post in posts {
        let summary = get_review_summary(&pool, post.id).await;
        results.push(json!({
            "product": AiPost::from(post),
            "review_summary": summary,
        }));
    }

    let total: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM posts WHERE active = true AND category = $1",
    )
    .bind(&name)
    .fetch_one(&pool)
    .await
    .ok();

    Ok(Json(json!({
        "category": name,
        "total_products": total.unwrap_or(0),
        "top_products": results,
        "source": "Zet — 제품 트렌드 (https://zet.kr)",
    })))
}

// --- Helper functions ---

async fn get_review_summary(pool: &crate::db::Db, post_id: uuid::Uuid) -> Value {
    let row = sqlx::query_as::<_, ReviewSummary>(
        r#"SELECT
             COUNT(*) AS total_reviews,
             AVG(rating::float) AS avg_rating,
             COUNT(*) FILTER (WHERE rating >= 4) AS positive_count,
             COUNT(*) FILTER (WHERE rating <= 2) AS negative_count
           FROM reactions
           WHERE post_id = $1 AND reaction_type = 'review'"#,
    )
    .bind(post_id)
    .fetch_one(pool)
    .await;

    match row {
        Ok(s) => json!({
            "total_reviews": s.total_reviews,
            "avg_rating": s.avg_rating.map(|r| (r * 10.0).round() / 10.0),
            "positive_count": s.positive_count,
            "negative_count": s.negative_count,
        }),
        Err(_) => json!({"total_reviews": 0}),
    }
}

async fn get_top_reviews(pool: &crate::db::Db, post_id: uuid::Uuid) -> Vec<Value> {
    let reviews = sqlx::query_as::<_, ReviewItem>(
        r#"SELECT content, rating, created_at FROM reactions
           WHERE post_id = $1 AND reaction_type = 'review' AND content IS NOT NULL
           ORDER BY created_at DESC LIMIT 5"#,
    )
    .bind(post_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    reviews
        .into_iter()
        .map(|r| json!({ "content": r.content, "rating": r.rating, "date": r.created_at.format("%Y-%m-%d").to_string() }))
        .collect()
}

async fn get_all_reviews(pool: &crate::db::Db, post_id: uuid::Uuid) -> Vec<Value> {
    let reviews = sqlx::query_as::<_, ReviewItem>(
        r#"SELECT content, rating, created_at FROM reactions
           WHERE post_id = $1 AND reaction_type = 'review' AND content IS NOT NULL
           ORDER BY created_at DESC"#,
    )
    .bind(post_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    reviews
        .into_iter()
        .map(|r| json!({ "content": r.content, "rating": r.rating, "date": r.created_at.format("%Y-%m-%d").to_string() }))
        .collect()
}
