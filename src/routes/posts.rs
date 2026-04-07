use axum::{
    Router,
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::models::post::{CreatePost, Post};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/posts", post(create_post))
        .route("/posts/{id}", get(get_post).put(update_post).delete(delete_post))
        .route("/users/{user_id}/posts", get(user_posts))
}

async fn create_post(
    State(pool): State<Db>,
    Json(body): Json<CreatePost>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let user_role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
        .bind(body.author_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match user_role.as_deref() {
        Some("business") => {}
        Some(_) => return Err((StatusCode::FORBIDDEN, Json(json!({"error": "기업 회원만 게시글을 작성할 수 있습니다"})))),
        None => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "사용자를 찾을 수 없습니다"})))),
    }

    // Build content from blocks (extract text for plain content field)
    let blocks_json = body.blocks.as_ref().map(|b| serde_json::to_value(b).unwrap_or_default());
    let content = if let Some(blocks) = &body.blocks {
        blocks.iter()
            .filter(|b| b.block_type == "text")
            .map(|b| b.value.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        body.content.clone().unwrap_or_default()
    };

    let post = sqlx::query_as::<_, Post>(
        r#"INSERT INTO posts (author_id, content, blocks, media_urls, category, tags, target_age, target_gender, target_region, pricing_model, budget)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
           RETURNING *"#,
    )
    .bind(body.author_id)
    .bind(&content)
    .bind(&blocks_json)
    .bind(&body.media_urls.unwrap_or_default())
    .bind(&body.category)
    .bind(&body.tags.unwrap_or_default())
    .bind(&body.target_age)
    .bind(&body.target_gender)
    .bind(&body.target_region)
    .bind(body.pricing_model.as_deref().unwrap_or("cpm"))
    .bind(body.budget.unwrap_or(0))
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
    })?;

    Ok((StatusCode::CREATED, Json(json!({"post": post}))))
}

async fn get_post(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match post {
        Some(post) => Ok(Json(json!({"post": post}))),
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "Post not found"})))),
    }
}

#[derive(Debug, Deserialize)]
struct UpdatePost {
    author_id: Uuid,
    blocks: Option<Vec<crate::models::post::ContentBlock>>,
    content: Option<String>,
    category: Option<String>,
    tags: Option<Vec<String>>,
    media_urls: Option<Vec<String>>,
    target_age: Option<String>,
    target_gender: Option<String>,
    target_region: Option<String>,
}

async fn update_post(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePost>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Verify ownership
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT author_id FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?
        .flatten();

    if owner != Some(body.author_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인의 게시글만 수정할 수 있습니다"}))));
    }

    // Build content from blocks if provided
    let blocks_json = body.blocks.as_ref().map(|b| serde_json::to_value(b).unwrap_or_default());
    let content = if let Some(blocks) = &body.blocks {
        Some(blocks.iter()
            .filter(|b| b.block_type == "text")
            .map(|b| b.value.as_str())
            .collect::<Vec<_>>()
            .join("\n"))
    } else {
        body.content.clone()
    };

    let post = sqlx::query_as::<_, Post>(
        r#"UPDATE posts SET
            content = COALESCE($2, content),
            blocks = COALESCE($3, blocks),
            category = COALESCE($4, category),
            tags = COALESCE($5, tags),
            media_urls = COALESCE($6, media_urls),
            target_age = COALESCE($7, target_age),
            target_gender = COALESCE($8, target_gender),
            target_region = COALESCE($9, target_region),
            updated_at = now()
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(&content)
    .bind(&blocks_json)
    .bind(&body.category)
    .bind(&body.tags)
    .bind(&body.media_urls)
    .bind(&body.target_age)
    .bind(&body.target_gender)
    .bind(&body.target_region)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({"post": post})))
}

#[derive(Debug, Deserialize)]
struct DeleteQuery {
    author_id: Uuid,
}

async fn delete_post(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Query(query): Query<DeleteQuery>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT author_id FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?
        .flatten();

    if owner != Some(query.author_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인의 게시글만 삭제할 수 있습니다"}))));
    }

    sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct UserPostsQuery {
    cursor: Option<i64>,
    limit: Option<i64>,
}

async fn user_posts(
    State(pool): State<Db>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<UserPostsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let cursor = query.cursor.unwrap_or(0);

    let posts = sqlx::query_as::<_, Post>(
        "SELECT * FROM posts WHERE author_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(limit)
    .bind(cursor)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "posts": posts, "next_cursor": cursor + limit })))
}
