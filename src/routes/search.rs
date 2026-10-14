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
use crate::util::{build_tsquery, escape_like};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/search", get(search))
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub category: Option<String>,
    pub viewer_id: Option<Uuid>,
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

/// 전문 검색(prefix tsquery) + 부분 일치(본문/태그/작성자) 를 합친 검색.
/// 특수문자만 있는 입력("(" 등)도 500 없이 처리된다.
async fn search(
    State(pool): State<Db>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let cursor = query.cursor.unwrap_or(0);

    let q = query.q.trim();
    let q = q.strip_prefix('#').unwrap_or(q).trim();
    if q.is_empty() {
        return Ok(Json(json!({ "posts": [], "next_cursor": cursor })));
    }

    let tsquery = build_tsquery(q);
    let like_pattern = format!("%{}%", escape_like(q));

    let sql = format!(
        r#"SELECT {select},
                  CASE WHEN $1::text <> '' THEN ts_rank(p.search_vector, to_tsquery('simple', $1)) ELSE 0 END AS rank
           FROM posts p
           LEFT JOIN users au ON au.id = p.author_id
           WHERE p.active = true
             AND ($2::text IS NULL OR p.category = $2)
             AND (
               ($1::text <> '' AND p.search_vector @@ to_tsquery('simple', $1))
               OR p.content ILIKE $3
               OR EXISTS (SELECT 1 FROM unnest(p.tags) t WHERE t ILIKE $3)
               OR au.name ILIKE $3
               OR au.username ILIKE $3
             )
           ORDER BY rank DESC, p.like_count DESC, p.created_at DESC
           LIMIT $4 OFFSET $5"#,
        select = post_view_select("$6::uuid"),
    );

    let posts = sqlx::query_as::<_, PostView>(&sql)
        .bind(&tsquery)
        .bind(&query.category)
        .bind(&like_pattern)
        .bind(limit)
        .bind(cursor)
        .bind(&query.viewer_id)
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
