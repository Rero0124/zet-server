use axum::{
    Router,
    Json,
    extract::State,
    http::StatusCode,
    routing::post,
};
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::{SaltString, rand_core::OsRng}};
use serde_json::{json, Value};

use crate::db::Db;
use crate::models::user::{CreateUser, LoginRequest, User};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
}

fn hash_password(password: &str) -> Result<String, (StatusCode, Json<Value>)> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })
}

fn verify_password(password: &str, hash: &str) -> bool {
    // 기존 hash: prefix 방식 하위호환
    if hash.starts_with("hash:") {
        return hash == format!("hash:{}", password);
    }
    let Ok(parsed) = argon2::PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

async fn register(
    State(pool): State<Db>,
    Json(body): Json<CreateUser>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let password_hash = hash_password(&body.password)?;
    let role = if body.is_business.unwrap_or(false) { "business" } else { "user" };

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (email, password_hash, username, name, birth_date, gender, region, role)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING *"#,
    )
    .bind(&body.email)
    .bind(&password_hash)
    .bind(&body.username)
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

    Ok((StatusCode::CREATED, Json(json!({"user": user}))))
}

async fn login(
    State(pool): State<Db>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
    })?;

    let Some(user) = user else {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"}))));
    };

    if !verify_password(&body.password, &user.password_hash) {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"}))));
    }

    // 기존 hash: prefix 비밀번호를 argon2로 자동 마이그레이션
    if user.password_hash.starts_with("hash:") {
        if let Ok(new_hash) = hash_password(&body.password) {
            let _ = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
                .bind(&new_hash)
                .bind(user.id)
                .execute(&pool)
                .await;
        }
    }

    Ok(Json(json!({"user": user})))
}
