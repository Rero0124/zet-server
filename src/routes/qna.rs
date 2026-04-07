use axum::{
    Router,
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, delete},
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::models::question::{Question, CreateQuestion, Answer, CreateAnswer};

pub fn router() -> Router<Db> {
    Router::new()
        .route("/questions", get(list_questions).post(create_question))
        .route("/questions/{id}", get(get_question).delete(delete_question))
        .route("/questions/{id}/answers", get(list_answers).post(create_answer))
        .route("/questions/{question_id}/answers/{id}", delete(delete_answer))
}

#[derive(Debug, Deserialize)]
struct ListQuestionsQuery {
    post_id: Option<Uuid>,
    q: Option<String>,
    cursor: Option<i64>,
    limit: Option<i64>,
}

async fn list_questions(
    State(pool): State<Db>,
    Query(query): Query<ListQuestionsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let limit = query.limit.unwrap_or(20).min(50);
    let offset = query.cursor.unwrap_or(0);

    let questions = match (&query.q, &query.post_id) {
        // 검색 + 게시글 필터
        (Some(q), Some(post_id)) => {
            let pattern = format!("%{}%", q.trim());
            sqlx::query_as::<_, QuestionWithUser>(
                r#"SELECT DISTINCT q.*, u.name as user_name, u.username as user_username
                   FROM questions q
                   JOIN users u ON q.user_id = u.id
                   LEFT JOIN answers a ON a.question_id = q.id
                   WHERE q.post_id = $1
                     AND (q.content ILIKE $2 OR a.content ILIKE $2)
                   ORDER BY q.created_at DESC LIMIT $3 OFFSET $4"#,
            )
            .bind(post_id)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&pool)
            .await
        }
        // 검색만
        (Some(q), None) => {
            let pattern = format!("%{}%", q.trim());
            sqlx::query_as::<_, QuestionWithUser>(
                r#"SELECT DISTINCT q.*, u.name as user_name, u.username as user_username
                   FROM questions q
                   JOIN users u ON q.user_id = u.id
                   LEFT JOIN answers a ON a.question_id = q.id
                   WHERE q.content ILIKE $1 OR a.content ILIKE $1
                   ORDER BY q.created_at DESC LIMIT $2 OFFSET $3"#,
            )
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(&pool)
            .await
        }
        // 게시글 필터만
        (None, Some(post_id)) => {
            sqlx::query_as::<_, QuestionWithUser>(
                r#"SELECT q.*, u.name as user_name, u.username as user_username
                   FROM questions q JOIN users u ON q.user_id = u.id
                   WHERE q.post_id = $1
                   ORDER BY q.created_at DESC LIMIT $2 OFFSET $3"#,
            )
            .bind(post_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&pool)
            .await
        }
        // 전체
        (None, None) => {
            sqlx::query_as::<_, QuestionWithUser>(
                r#"SELECT q.*, u.name as user_name, u.username as user_username
                   FROM questions q JOIN users u ON q.user_id = u.id
                   ORDER BY q.created_at DESC LIMIT $1 OFFSET $2"#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&pool)
            .await
        }
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "questions": questions, "next_cursor": offset + limit })))
}

async fn create_question(
    State(pool): State<Db>,
    Json(body): Json<CreateQuestion>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    // 게시글 존재 확인
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE id = $1)")
        .bind(body.post_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "게시글을 찾을 수 없습니다"}))));
    }

    let question = sqlx::query_as::<_, Question>(
        "INSERT INTO questions (post_id, user_id, content) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(body.post_id)
    .bind(body.user_id)
    .bind(&body.content)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;

    Ok((StatusCode::CREATED, Json(json!({"question": question}))))
}

async fn get_question(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let question = sqlx::query_as::<_, QuestionWithUser>(
        r#"SELECT q.*, u.name as user_name, u.username as user_username
           FROM questions q JOIN users u ON q.user_id = u.id
           WHERE q.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    match question {
        Some(q) => {
            let answers = sqlx::query_as::<_, AnswerWithUser>(
                r#"SELECT a.*, u.name as user_name, u.username as user_username
                   FROM answers a JOIN users u ON a.user_id = u.id
                   WHERE a.question_id = $1
                   ORDER BY a.created_at ASC"#,
            )
            .bind(id)
            .fetch_all(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

            Ok(Json(json!({"question": q, "answers": answers})))
        }
        None => Err((StatusCode::NOT_FOUND, Json(json!({"error": "질문을 찾을 수 없습니다"})))),
    }
}

#[derive(Debug, Deserialize)]
struct DeleteQuery {
    user_id: Uuid,
}

async fn delete_question(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Query(query): Query<DeleteQuery>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM questions WHERE id = $1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if owner != Some(query.user_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인의 질문만 삭제할 수 있습니다"}))));
    }

    sqlx::query("DELETE FROM questions WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_answers(
    State(pool): State<Db>,
    Path(question_id): Path<Uuid>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let answers = sqlx::query_as::<_, AnswerWithUser>(
        r#"SELECT a.*, u.name as user_name, u.username as user_username
           FROM answers a JOIN users u ON a.user_id = u.id
           WHERE a.question_id = $1
           ORDER BY a.created_at ASC"#,
    )
    .bind(question_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "answers": answers })))
}

async fn create_answer(
    State(pool): State<Db>,
    Path(question_id): Path<Uuid>,
    Json(body): Json<CreateAnswer>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM questions WHERE id = $1)")
        .bind(question_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "질문을 찾을 수 없습니다"}))));
    }

    let answer = sqlx::query_as::<_, Answer>(
        "INSERT INTO answers (question_id, user_id, content) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(question_id)
    .bind(body.user_id)
    .bind(&body.content)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))))?;

    Ok((StatusCode::CREATED, Json(json!({"answer": answer}))))
}

async fn delete_answer(
    State(pool): State<Db>,
    Path((_, answer_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DeleteQuery>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM answers WHERE id = $1")
        .bind(answer_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if owner != Some(query.user_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인의 답변만 삭제할 수 있습니다"}))));
    }

    sqlx::query("DELETE FROM answers WHERE id = $1")
        .bind(answer_id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(StatusCode::NO_CONTENT)
}

// JOIN 결과용 확장 구조체
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct QuestionWithUser {
    id: Uuid,
    post_id: Uuid,
    user_id: Uuid,
    content: String,
    answer_count: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    user_name: String,
    user_username: String,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
struct AnswerWithUser {
    id: Uuid,
    question_id: Uuid,
    user_id: Uuid,
    content: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    user_name: String,
    user_username: String,
}
