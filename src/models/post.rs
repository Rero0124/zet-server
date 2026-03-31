use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
    pub company_id: Option<Uuid>,
    pub author_id: Option<Uuid>,
    pub content: String,
    pub blocks: Option<serde_json::Value>,
    pub media_urls: Vec<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub target_age: Option<String>,
    pub target_gender: Option<String>,
    pub target_region: Option<String>,
    pub pricing_model: String,
    pub budget: i64,
    pub spent: i64,
    pub impressions: i64,
    pub clicks: i64,
    pub score: f64,
    pub like_count: i64,
    pub review_count: i64,
    pub bookmark_count: i64,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePost {
    pub author_id: Uuid,
    pub blocks: Option<Vec<ContentBlock>>,
    pub content: Option<String>, // fallback for plain text
    pub media_urls: Option<Vec<String>>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub target_age: Option<String>,
    pub target_gender: Option<String>,
    pub target_region: Option<String>,
    pub pricing_model: Option<String>,
    pub budget: Option<i64>,
}
