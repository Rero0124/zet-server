use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub username: String,
    pub name: String,
    pub birth_date: Option<NaiveDate>,
    pub gender: Option<String>,
    pub region: Option<String>,
    pub role: String,
    pub points: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub email: String,
    pub password: String,
    pub username: String,
    pub name: String,
    pub birth_date: Option<NaiveDate>,
    pub gender: Option<String>,
    pub region: Option<String>,
    pub is_business: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
