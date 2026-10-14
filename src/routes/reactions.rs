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
use crate::models::reaction::{CreateReaction, REACTION_VIEW_SELECT, Reaction, ReactionView};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/posts/{post_id}/reactions", post(create_reaction).get(list_reactions))
        .route("/posts/{post_id}/reactions/{reaction_id}", axum::routing::put(update_reaction).delete(delete_reaction))
        .route("/posts/{post_id}/like", post(toggle_like))
        .route("/posts/{post_id}/bookmark", post(toggle_bookmark))
        .route("/posts/{post_id}/status", get(reaction_status))
        .route("/users/{user_id}/reviews", get(user_reviews))
}

async fn create_reaction(
    State(pool): State<Db>,
    Path(post_id): Path<Uuid>,
    Json(body): Json<CreateReaction>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let post_author: Option<Option<Uuid>> = sqlx::query_scalar("SELECT author_id FROM posts WHERE id = $1")
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let Some(post_author) = post_author else {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "게시글을 찾을 수 없습니다"}))));
    };

    let mut content = body.content.clone();
    if body.reaction_type == "review" {
        match body.rating {
            Some(r) if (1..=5).contains(&r) => {}
            _ => return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "별점은 1~5 사이여야 합니다"})))),
        }
        let trimmed = content.as_deref().map(str::trim).unwrap_or("");
        if trimmed.is_empty() {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "리뷰 내용을 입력해주세요"}))));
        }
        if post_author == Some(body.user_id) {
            return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인 게시글에는 리뷰를 남길 수 없습니다"}))));
        }
        content = Some(trimmed.to_string());
    }

    let sql = format!(
        r#"WITH r AS (
             INSERT INTO reactions (post_id, user_id, reaction_type, content, rating, media_urls)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (post_id, user_id, reaction_type) DO UPDATE
             SET content = EXCLUDED.content, rating = EXCLUDED.rating, media_urls = EXCLUDED.media_urls
             RETURNING *
           )
           SELECT {REACTION_VIEW_SELECT} FROM r JOIN users u ON u.id = r.user_id"#
    );
    let reaction = sqlx::query_as::<_, ReactionView>(&sql)
        .bind(post_id)
        .bind(body.user_id)
        .bind(&body.reaction_type)
        .bind(&content)
        .bind(body.rating)
        .bind(&body.media_urls.unwrap_or_default())
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
        })?;

    if body.reaction_type == "review" {
        let _ = sqlx::query(
            "UPDATE posts SET review_count = (SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND reaction_type = 'review') WHERE id = $1"
        ).bind(post_id).execute(&pool).await;
    }

    Ok((StatusCode::CREATED, Json(json!({"reaction": reaction}))))
}

async fn list_reactions(
    State(pool): State<Db>,
    Path(post_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let sql = format!(
        r#"SELECT {REACTION_VIEW_SELECT} FROM reactions r
           JOIN users u ON u.id = r.user_id
           WHERE r.post_id = $1 ORDER BY r.created_at DESC"#
    );
    let reactions = sqlx::query_as::<_, ReactionView>(&sql)
        .bind(post_id)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

    Ok(Json(json!({"reactions": reactions})))
}

#[derive(Debug, Deserialize)]
struct UpdateReaction {
    user_id: Uuid,
    content: Option<String>,
    rating: Option<i16>,
    media_urls: Option<Vec<String>>,
}

async fn update_reaction(
    State(pool): State<Db>,
    Path((_post_id, reaction_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateReaction>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM reactions WHERE id = $1")
        .bind(reaction_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if owner != Some(body.user_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인의 리뷰만 수정할 수 있습니다"}))));
    }

    if let Some(r) = body.rating {
        if !(1..=5).contains(&r) {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "별점은 1~5 사이여야 합니다"}))));
        }
    }

    let sql = format!(
        r#"WITH r AS (
             UPDATE reactions SET
               content = COALESCE($2, content),
               rating = COALESCE($3, rating),
               media_urls = COALESCE($4, media_urls)
             WHERE id = $1 RETURNING *
           )
           SELECT {REACTION_VIEW_SELECT} FROM r JOIN users u ON u.id = r.user_id"#
    );
    let reaction = sqlx::query_as::<_, ReactionView>(&sql)
        .bind(reaction_id)
        .bind(&body.content)
        .bind(body.rating)
        .bind(&body.media_urls)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({"reaction": reaction})))
}

#[derive(Debug, Deserialize)]
struct DeleteReactionQuery {
    user_id: Uuid,
}

async fn delete_reaction(
    State(pool): State<Db>,
    Path((post_id, reaction_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DeleteReactionQuery>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM reactions WHERE id = $1")
        .bind(reaction_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if owner != Some(query.user_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인의 리뷰만 삭제할 수 있습니다"}))));
    }

    let rtype: Option<String> = sqlx::query_scalar("SELECT reaction_type FROM reactions WHERE id = $1")
        .bind(reaction_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    sqlx::query("DELETE FROM reactions WHERE id = $1")
        .bind(reaction_id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // Update counts
    if let Some(rt) = rtype {
        let col = match rt.as_str() {
            "like" => "like_count",
            "review" => "review_count",
            "bookmark" => "bookmark_count",
            _ => "",
        };
        if !col.is_empty() {
            let q = format!(
                "UPDATE posts SET {col} = (SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND reaction_type = $2) WHERE id = $1"
            );
            let _ = sqlx::query(&q).bind(post_id).bind(&rt).execute(&pool).await;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct ToggleBody {
    user_id: Uuid,
}

async fn toggle_like(
    State(pool): State<Db>,
    Path(post_id): Path<Uuid>,
    Json(body): Json<ToggleBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = sqlx::query_as::<_, Reaction>(
        "SELECT * FROM reactions WHERE post_id = $1 AND user_id = $2 AND reaction_type = 'like'",
    )
    .bind(post_id)
    .bind(body.user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let liked = if existing.is_some() {
        sqlx::query("DELETE FROM reactions WHERE post_id = $1 AND user_id = $2 AND reaction_type = 'like'")
            .bind(post_id).bind(body.user_id).execute(&pool).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
        false
    } else {
        sqlx::query("INSERT INTO reactions (post_id, user_id, reaction_type) VALUES ($1, $2, 'like')")
            .bind(post_id).bind(body.user_id).execute(&pool).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
        true
    };

    let _ = sqlx::query(
        "UPDATE posts SET like_count = (SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND reaction_type = 'like') WHERE id = $1"
    ).bind(post_id).execute(&pool).await;

    Ok(Json(json!({"liked": liked})))
}

async fn toggle_bookmark(
    State(pool): State<Db>,
    Path(post_id): Path<Uuid>,
    Json(body): Json<ToggleBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = sqlx::query_as::<_, Reaction>(
        "SELECT * FROM reactions WHERE post_id = $1 AND user_id = $2 AND reaction_type = 'bookmark'",
    )
    .bind(post_id)
    .bind(body.user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let bookmarked = if existing.is_some() {
        sqlx::query("DELETE FROM reactions WHERE post_id = $1 AND user_id = $2 AND reaction_type = 'bookmark'")
            .bind(post_id).bind(body.user_id).execute(&pool).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
        false
    } else {
        sqlx::query("INSERT INTO reactions (post_id, user_id, reaction_type) VALUES ($1, $2, 'bookmark')")
            .bind(post_id).bind(body.user_id).execute(&pool).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
        true
    };

    let _ = sqlx::query(
        "UPDATE posts SET bookmark_count = (SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND reaction_type = 'bookmark') WHERE id = $1"
    ).bind(post_id).execute(&pool).await;

    Ok(Json(json!({"bookmarked": bookmarked})))
}

#[derive(Debug, Deserialize)]
struct StatusQuery {
    user_id: Uuid,
}

async fn reaction_status(
    State(pool): State<Db>,
    Path(post_id): Path<Uuid>,
    Query(query): Query<StatusQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let liked = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND user_id = $2 AND reaction_type = 'like'",
    )
    .bind(post_id)
    .bind(query.user_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0) > 0;

    let bookmarked = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND user_id = $2 AND reaction_type = 'bookmark'",
    )
    .bind(post_id)
    .bind(query.user_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0) > 0;

    Ok(Json(json!({ "liked": liked, "bookmarked": bookmarked })))
}

#[derive(Debug, Deserialize)]
struct UserReviewsQuery {
    cursor: Option<i64>,
    limit: Option<i64>,
}

/// 유저가 남긴 리뷰 + 대상 게시글 요약
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct UserReviewItem {
    #[sqlx(flatten)]
    #[serde(flatten)]
    review: ReactionView,
    post_content: String,
    post_category: Option<String>,
    post_author_name: Option<String>,
    post_author_username: Option<String>,
}

async fn user_reviews(
    State(pool): State<Db>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<UserReviewsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let cursor = query.cursor.unwrap_or(0);

    let sql = format!(
        r#"SELECT {REACTION_VIEW_SELECT},
                  LEFT(p.content, 140) AS post_content,
                  p.category AS post_category,
                  pa.name AS post_author_name,
                  pa.username AS post_author_username
           FROM reactions r
           JOIN users u ON u.id = r.user_id
           JOIN posts p ON p.id = r.post_id
           LEFT JOIN users pa ON pa.id = p.author_id
           WHERE r.user_id = $1 AND r.reaction_type = 'review'
           ORDER BY r.created_at DESC LIMIT $2 OFFSET $3"#
    );
    let reviews = sqlx::query_as::<_, UserReviewItem>(&sql)
        .bind(user_id)
        .bind(limit)
        .bind(cursor)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "reviews": reviews, "next_cursor": cursor + limit })))
}
