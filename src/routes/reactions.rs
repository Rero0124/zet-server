use axum::{
    Router,
    Json,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::models::reaction::{CreateReaction, Reaction};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/posts/{post_id}/reactions", post(create_reaction).get(list_reactions))
        .route("/posts/{post_id}/reactions/{reaction_id}", axum::routing::put(update_reaction).delete(delete_reaction))
        .route("/posts/{post_id}/like", post(toggle_like))
        .route("/posts/{post_id}/bookmark", post(toggle_bookmark))
}

async fn create_reaction(
    State(pool): State<Db>,
    Path(post_id): Path<Uuid>,
    Json(body): Json<CreateReaction>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let reaction = sqlx::query_as::<_, Reaction>(
        r#"INSERT INTO reactions (post_id, user_id, reaction_type, content, rating)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (post_id, user_id, reaction_type) DO UPDATE
           SET content = EXCLUDED.content, rating = EXCLUDED.rating
           RETURNING *"#,
    )
    .bind(post_id)
    .bind(body.user_id)
    .bind(&body.reaction_type)
    .bind(&body.content)
    .bind(body.rating)
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
    let reactions = sqlx::query_as::<_, Reaction>(
        "SELECT * FROM reactions WHERE post_id = $1 ORDER BY created_at DESC",
    )
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

    let reaction = sqlx::query_as::<_, Reaction>(
        "UPDATE reactions SET content = COALESCE($2, content), rating = COALESCE($3, rating) WHERE id = $1 RETURNING *",
    )
    .bind(reaction_id)
    .bind(&body.content)
    .bind(body.rating)
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
    axum::extract::Query(query): axum::extract::Query<DeleteReactionQuery>,
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
