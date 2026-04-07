use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Question {
    pub id: Uuid,
    pub post_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub answer_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateQuestion {
    pub post_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub content: String,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Answer {
    pub id: Uuid,
    pub question_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAnswer {
    pub user_id: Uuid,
    pub content: String,
}
