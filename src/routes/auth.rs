use axum::{
    Router,
    Json,
    extract::State,
    http::StatusCode,
    routing::post,
};
use serde_json::{json, Value};

use crate::db::Db;
use crate::models::user::{CreateUser, LoginRequest, User};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
}

async fn register(
    State(pool): State<Db>,
    Json(body): Json<CreateUser>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let password_hash = format!("hash:{}", body.password);
    let role = if body.is_business.unwrap_or(false) { "business" } else { "user" };

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (email, password_hash, name, birth_date, gender, region, role)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING *"#,
    )
    .bind(&body.email)
    .bind(&password_hash)
    .bind(&body.name)
    .bind(&body.birth_date)
    .bind(&body.gender)
    .bind(&body.region)
    .bind(role)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
    })?;

    // If business user, create company record
    if role == "business" {
        let biz_name = body.business_name.as_deref().unwrap_or(&body.name);
        let reg_no = body.registration_no.as_deref().unwrap_or("");

        sqlx::query(
            r#"INSERT INTO companies (user_id, business_name, registration_no, verified)
               VALUES ($1, $2, $3, false)"#,
        )
        .bind(user.id)
        .bind(biz_name)
        .bind(reg_no)
        .execute(&pool)
        .await
        .map_err(|e| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
        })?;
    }

    Ok((StatusCode::CREATED, Json(json!({"user": user}))))
}

async fn login(
    State(pool): State<Db>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let password_hash = format!("hash:{}", body.password);

    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1 AND password_hash = $2",
    )
    .bind(&body.email)
    .bind(&password_hash)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    match user {
        Some(user) => Ok(Json(json!({"user": user}))),
        None => Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"})))),
    }
}
