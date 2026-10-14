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
    pub media_urls: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// `Reaction` + 작성자 요약. `REACTION_VIEW_SELECT` 와
/// `FROM reactions r JOIN users u ON u.id = r.user_id` 조합으로 조회한다.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ReactionView {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub reaction: Reaction,
    pub user_name: String,
    pub user_username: String,
    pub user_avatar_url: Option<String>,
}

pub const REACTION_VIEW_SELECT: &str =
    "r.*, u.name AS user_name, u.username AS user_username, u.avatar_url AS user_avatar_url";

#[derive(Debug, Deserialize)]
pub struct CreateReaction {
    pub user_id: Uuid,
    pub reaction_type: String,
    pub content: Option<String>,
    pub rating: Option<i16>,
    pub media_urls: Option<Vec<String>>,
}
