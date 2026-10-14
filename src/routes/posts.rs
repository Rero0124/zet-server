use axum::{
    Router,
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::models::post::{CreatePost, Post, PostView, post_view_select};
use crate::util::{blank_to_null, deserialize_some, non_blank};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/posts", post(create_post))
        .route("/posts/{id}", get(get_post).put(update_post).delete(delete_post))
        .route("/users/{user_id}/posts", get(user_posts))
        .route("/users/{user_id}/bookmarks", get(user_bookmarks))
}

fn validate_link_url(url: Option<&str>) -> Result<(), (StatusCode, Json<Value>)> {
    if let Some(u) = url {
        if !(u.starts_with("http://") || u.starts_with("https://")) {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "링크는 http(s)://로 시작해야 합니다"}))));
        }
    }
    Ok(())
}

fn validate_price(price: Option<i64>) -> Result<(), (StatusCode, Json<Value>)> {
    if let Some(p) = price {
        if p < 0 {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "가격은 0 이상이어야 합니다"}))));
        }
    }
    Ok(())
}

fn validate_cta_label(label: Option<&str>) -> Result<(), (StatusCode, Json<Value>)> {
    if let Some(l) = label {
        if l.chars().count() > 20 {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "버튼 문구는 20자 이내여야 합니다"}))));
        }
    }
    Ok(())
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

    let link_url = non_blank(body.link_url);
    let cta_label = non_blank(body.cta_label);
    validate_link_url(link_url.as_deref())?;
    validate_cta_label(cta_label.as_deref())?;
    validate_price(body.price)?;

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
        r#"INSERT INTO posts (author_id, content, blocks, media_urls, category, tags, target_age, target_gender, target_region, pricing_model, budget, link_url, cta_label, price)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
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
    .bind(&link_url)
    .bind(&cta_label)
    .bind(body.price)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
    })?;

    Ok((StatusCode::CREATED, Json(json!({"post": post}))))
}

#[derive(Debug, Deserialize)]
struct ViewerQuery {
    viewer_id: Option<Uuid>,
}

async fn get_post(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Query(query): Query<ViewerQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let sql = format!(
        "SELECT {} FROM posts p LEFT JOIN users au ON au.id = p.author_id WHERE p.id = $1",
        post_view_select("$2::uuid"),
    );
    let post = sqlx::query_as::<_, PostView>(&sql)
        .bind(id)
        .bind(&query.viewer_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match post {
        Some(post) => Ok(Json(json!({"post": post}))),
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "Post not found"})))),
    }
}

/// 생략 → 유지, null/빈 문자열 → NULL, 값 → 설정 (link_url 등 지울 수 있는 필드)
#[derive(Debug, Deserialize)]
struct UpdatePost {
    author_id: Uuid,
    blocks: Option<Vec<crate::models::post::ContentBlock>>,
    content: Option<String>,
    category: Option<String>,
    tags: Option<Vec<String>>,
    media_urls: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    target_age: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    target_gender: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    target_region: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    link_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    cta_label: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    price: Option<Option<i64>>,
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

    let target_age = blank_to_null(body.target_age);
    let target_gender = blank_to_null(body.target_gender);
    let target_region = blank_to_null(body.target_region);
    let link_url = blank_to_null(body.link_url);
    let cta_label = blank_to_null(body.cta_label);
    let price = body.price;

    validate_link_url(link_url.as_ref().and_then(|v| v.as_deref()))?;
    validate_cta_label(cta_label.as_ref().and_then(|v| v.as_deref()))?;
    validate_price(price.flatten())?;

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

    // 지울 수 있는 필드는 (set 여부, 값) 두 파라미터로 바인딩
    let post = sqlx::query_as::<_, Post>(
        r#"UPDATE posts SET
            content = COALESCE($2, content),
            blocks = COALESCE($3, blocks),
            category = COALESCE($4, category),
            tags = COALESCE($5, tags),
            media_urls = COALESCE($6, media_urls),
            target_age = CASE WHEN $7::bool THEN $8 ELSE target_age END,
            target_gender = CASE WHEN $9::bool THEN $10 ELSE target_gender END,
            target_region = CASE WHEN $11::bool THEN $12 ELSE target_region END,
            link_url = CASE WHEN $13::bool THEN $14 ELSE link_url END,
            cta_label = CASE WHEN $15::bool THEN $16 ELSE cta_label END,
            price = CASE WHEN $17::bool THEN $18::bigint ELSE price END,
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
    .bind(target_age.is_some())
    .bind(target_age.flatten())
    .bind(target_gender.is_some())
    .bind(target_gender.flatten())
    .bind(target_region.is_some())
    .bind(target_region.flatten())
    .bind(link_url.is_some())
    .bind(link_url.flatten())
    .bind(cta_label.is_some())
    .bind(cta_label.flatten())
    .bind(price.is_some())
    .bind(price.flatten())
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
    viewer_id: Option<Uuid>,
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

    let sql = format!(
        r#"SELECT {} FROM posts p
           LEFT JOIN users au ON au.id = p.author_id
           WHERE p.author_id = $1
           ORDER BY p.created_at DESC LIMIT $2 OFFSET $3"#,
        post_view_select("$4::uuid"),
    );
    let posts = sqlx::query_as::<_, PostView>(&sql)
        .bind(user_id)
        .bind(limit)
        .bind(cursor)
        .bind(&query.viewer_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "posts": posts, "next_cursor": cursor + limit })))
}

/// 저장(북마크)한 게시글. 본인만 조회 가능, 북마크한 시각 역순.
async fn user_bookmarks(
    State(pool): State<Db>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<UserPostsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if query.viewer_id != Some(user_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "저장한 게시글은 본인만 볼 수 있습니다"}))));
    }

    let limit = query.limit.unwrap_or(20).min(50);
    let cursor = query.cursor.unwrap_or(0);

    let sql = format!(
        r#"SELECT {} FROM reactions b
           JOIN posts p ON p.id = b.post_id
           LEFT JOIN users au ON au.id = p.author_id
           WHERE b.user_id = $1 AND b.reaction_type = 'bookmark'
           ORDER BY b.created_at DESC LIMIT $2 OFFSET $3"#,
        post_view_select("$1::uuid"),
    );
    let posts = sqlx::query_as::<_, PostView>(&sql)
        .bind(user_id)
        .bind(limit)
        .bind(cursor)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "posts": posts, "next_cursor": cursor + limit })))
}
