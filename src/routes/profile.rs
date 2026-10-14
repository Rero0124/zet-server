use axum::{
    Router,
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::models::user::User;
use crate::util::{blank_to_null, db_err, deserialize_some, escape_like};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/users/search", get(search_users))
        .route("/users/by-username/{username}", get(get_user_by_username))
        .route("/users/{id}", get(get_user).put(update_user))
        .route("/users/{id}/follow", post(toggle_follow))
}

#[derive(Debug, Deserialize)]
struct SearchUsersQuery {
    q: String,
    limit: Option<i64>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct UserSummary {
    id: Uuid,
    username: String,
    name: String,
    role: String,
    avatar_url: Option<String>,
    bio: Option<String>,
}

/// 아이디는 접두 일치, 이름은 부분 일치. 기업 계정 우선.
async fn search_users(
    State(pool): State<Db>,
    Query(query): Query<SearchUsersQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(10).min(20);
    let q = escape_like(query.q.trim());
    let username_pattern = format!("{q}%");
    let name_pattern = format!("%{q}%");

    let users = sqlx::query_as::<_, UserSummary>(
        r#"SELECT id, username, name, role, avatar_url, bio FROM users
           WHERE username ILIKE $1 OR name ILIKE $2
           ORDER BY (role = 'business') DESC, username ASC LIMIT $3"#,
    )
    .bind(&username_pattern)
    .bind(&name_pattern)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "users": users })))
}

#[derive(Debug, Deserialize)]
struct ViewerQuery {
    viewer_id: Option<Uuid>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProfileStats {
    posts: i64,
    reviews: i64,
    followers: i64,
    following: i64,
    is_following: bool,
}

/// `{ user, stats, is_following }` 응답 조립
async fn profile_response(
    pool: &Db,
    user: User,
    viewer_id: Option<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let stats = sqlx::query_as::<_, ProfileStats>(
        r#"SELECT
             (SELECT COUNT(*) FROM posts WHERE author_id = $1) AS posts,
             (SELECT COUNT(*) FROM reactions WHERE user_id = $1 AND reaction_type = 'review') AS reviews,
             (SELECT COUNT(*) FROM follows WHERE followee_id = $1) AS followers,
             (SELECT COUNT(*) FROM follows WHERE follower_id = $1) AS following,
             EXISTS(SELECT 1 FROM follows WHERE follower_id = $2::uuid AND followee_id = $1) AS is_following"#,
    )
    .bind(user.id)
    .bind(viewer_id)
    .fetch_one(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({
        "user": user,
        "stats": {
            "posts": stats.posts,
            "reviews": stats.reviews,
            "followers": stats.followers,
            "following": stats.following,
        },
        "is_following": stats.is_following,
    })))
}

async fn get_user_by_username(
    State(pool): State<Db>,
    Path(username): Path<String>,
    Query(query): Query<ViewerQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = $1")
        .bind(&username)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match user {
        Some(user) => profile_response(&pool, user, query.viewer_id).await,
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))),
    }
}

async fn get_user(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Query(query): Query<ViewerQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

    match user {
        Some(user) => profile_response(&pool, user, query.viewer_id).await,
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
    /// 생략 → 유지, null/빈 문자열 → NULL
    #[serde(default, deserialize_with = "deserialize_some")]
    bio: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_some")]
    avatar_url: Option<Option<String>>,
}

async fn update_user(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUser>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let bio = blank_to_null(body.bio);
    let avatar_url = blank_to_null(body.avatar_url);

    if let Some(Some(b)) = &bio {
        if b.chars().count() > 300 {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "소개는 300자 이내여야 합니다"}))));
        }
    }

    let user = sqlx::query_as::<_, User>(
        r#"UPDATE users SET
            username = COALESCE($2, username),
            name = COALESCE($3, name),
            birth_date = COALESCE($4, birth_date),
            gender = COALESCE($5, gender),
            region = COALESCE($6, region),
            bio = CASE WHEN $7::bool THEN $8 ELSE bio END,
            avatar_url = CASE WHEN $9::bool THEN $10 ELSE avatar_url END,
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
    .bind(bio.is_some())
    .bind(bio.flatten())
    .bind(avatar_url.is_some())
    .bind(avatar_url.flatten())
    .fetch_one(&pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))),
        e => db_err(e, StatusCode::BAD_REQUEST),
    })?;

    Ok(Json(json!({"user": user})))
}

#[derive(Debug, Deserialize)]
struct FollowBody {
    /// 팔로우하는 쪽(뷰어)
    user_id: Uuid,
}

/// 팔로우 토글. 이미 팔로우 중이면 해제.
async fn toggle_follow(
    State(pool): State<Db>,
    Path(followee_id): Path<Uuid>,
    Json(body): Json<FollowBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if body.user_id == followee_id {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "자기 자신은 팔로우할 수 없습니다"}))));
    }

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(followee_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;
    if !exists {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "사용자를 찾을 수 없습니다"}))));
    }

    let deleted = sqlx::query("DELETE FROM follows WHERE follower_id = $1 AND followee_id = $2")
        .bind(body.user_id)
        .bind(followee_id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?
        .rows_affected();

    let following = if deleted > 0 {
        false
    } else {
        sqlx::query("INSERT INTO follows (follower_id, followee_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(body.user_id)
            .bind(followee_id)
            .execute(&pool)
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;
        true
    };

    let follower_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM follows WHERE followee_id = $1")
        .bind(followee_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    Ok(Json(json!({ "following": following, "follower_count": follower_count })))
}
