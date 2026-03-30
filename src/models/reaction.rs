use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Reaction {
    pub id: Uuid,
    pub post_id: Uuid,
    pub user_id: Uuid,
    pub reaction_type: String,
    pub content: Option<String>,
    pub rating: Option<i16>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReaction {
    pub user_id: Uuid,
    pub reaction_type: String,
    pub content: Option<String>,
    pub rating: Option<i16>,
}
