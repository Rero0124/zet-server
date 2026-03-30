use axum::{
    Router,
    Json,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::Db;
use crate::models::post::Post;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/search", get(search))
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub category: Option<String>,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

async fn search(
    State(pool): State<Db>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let cursor = query.cursor.unwrap_or(0);
    let tsquery = query.q.split_whitespace().collect::<Vec<_>>().join(" | ");

    let posts = if let Some(category) = &query.category {
        sqlx::query_as::<_, Post>(
            r#"SELECT * FROM posts
               WHERE active = true
                 AND category = $1
                 AND search_vector @@ to_tsquery('simple', $2)
               ORDER BY ts_rank(search_vector, to_tsquery('simple', $2)) DESC, score DESC
               LIMIT $3 OFFSET $4"#,
        )
        .bind(category)
        .bind(&tsquery)
        .bind(limit)
        .bind(cursor)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as::<_, Post>(
            r#"SELECT * FROM posts
               WHERE active = true
                 AND search_vector @@ to_tsquery('simple', $1)
               ORDER BY ts_rank(search_vector, to_tsquery('simple', $1)) DESC, score DESC
               LIMIT $2 OFFSET $3"#,
        )
        .bind(&tsquery)
        .bind(limit)
        .bind(cursor)
        .fetch_all(&pool)
        .await
    }
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(json!({
        "posts": posts,
        "next_cursor": cursor + limit
    })))
}
