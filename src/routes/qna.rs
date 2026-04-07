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
    user_id: Option<Uuid>,
    tag: Option<String>,
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

    // 동적 쿼리 빌드
    let mut conditions: Vec<String> = Vec::new();
    let mut idx = 0usize;

    // post_id 필터
    if query.post_id.is_some() {
        idx += 1;
        conditions.push(format!("q.post_id = ${idx}"));
    }

    // user_id 필터
    if query.user_id.is_some() {
        idx += 1;
        conditions.push(format!("q.user_id = ${idx}"));
    }

    // 태그 필터
    if query.tag.is_some() {
        idx += 1;
        conditions.push(format!("${idx} = ANY(q.tags)"));
    }

    // 검색어 (제목 + 태그)
    let has_search = query.q.as_ref().is_some_and(|s| !s.trim().is_empty());
    let search_pattern = query.q.as_ref().map(|s| format!("%{}%", s.trim()));
    if has_search {
        idx += 1;
        // 제목 ILIKE 또는 태그 배열에 포함
        conditions.push(format!("(q.title ILIKE ${idx} OR EXISTS (SELECT 1 FROM unnest(q.tags) t WHERE t ILIKE ${idx}))"));
    }

    let distinct = "";
    let answer_join = "";
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    idx += 1;
    let limit_idx = idx;
    idx += 1;
    let offset_idx = idx;

    let sql = format!(
        r#"SELECT {distinct}q.*, u.name as user_name, u.username as user_username
           FROM questions q
           JOIN users u ON q.user_id = u.id
           {answer_join}
           {where_clause}
           ORDER BY q.created_at DESC LIMIT ${limit_idx} OFFSET ${offset_idx}"#,
    );

    // sqlx는 동적 바인드가 까다로우니 query_as + manual bind
    let mut qb = sqlx::query_as::<_, QuestionWithUser>(&sql);
    if let Some(post_id) = &query.post_id {
        qb = qb.bind(post_id);
    }
    if let Some(user_id) = &query.user_id {
        qb = qb.bind(user_id);
    }
    if let Some(tag) = &query.tag {
        qb = qb.bind(tag);
    }
    if let Some(pattern) = &search_pattern {
        qb = qb.bind(pattern);
    }
    qb = qb.bind(limit).bind(offset);

    let questions = qb.fetch_all(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    Ok(Json(json!({ "questions": questions, "next_cursor": offset + limit })))
}

async fn create_question(
    State(pool): State<Db>,
    Json(body): Json<CreateQuestion>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE id = $1)")
        .bind(body.post_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "게시글을 찾을 수 없습니다"}))));
    }

    let question = sqlx::query_as::<_, Question>(
        "INSERT INTO questions (post_id, user_id, title, content, tags) VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(body.post_id)
    .bind(body.user_id)
    .bind(&body.title)
    .bind(&body.content)
    .bind(&body.tags.unwrap_or_default())
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
    title: String,
    content: String,
    tags: Vec<String>,
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
