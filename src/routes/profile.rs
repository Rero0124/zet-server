use axum::{
    Router,
    Json,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::models::user::User;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/users/{id}", get(get_user).put(update_user))
}

async fn get_user(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

    match user {
        Some(user) => Ok(Json(json!({"user": user}))),
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))),
    }
}

#[derive(Debug, Deserialize)]
struct UpdateUser {
    username: Option<String>,
    name: Option<String>,
    birth_date: Option<NaiveDate>,
    gender: Option<String>,
    region: Option<String>,
}

async fn update_user(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUser>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = sqlx::query_as::<_, User>(
        r#"UPDATE users SET
            username = COALESCE($2, username),
            name = COALESCE($3, name),
            birth_date = COALESCE($4, birth_date),
            gender = COALESCE($5, gender),
            region = COALESCE($6, region),
            updated_at = now()
           WHERE id = $1
           RETURNING *"#,
    )
    .bind(id)
    .bind(&body.username)
    .bind(&body.name)
    .bind(&body.birth_date)
    .bind(&body.gender)
    .bind(&body.region)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
    })?;

    Ok(Json(json!({"user": user})))
}
