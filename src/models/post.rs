use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
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
    pub link_url: Option<String>,
    pub cta_label: Option<String>,
    pub price: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 피드/검색/상세 등 읽기 전용 응답용. `Post` 전체 + 작성자 요약 + 뷰어 기준 상태.
/// `post_view_select()` 가 만든 select 목록과
/// `FROM posts p LEFT JOIN users au ON au.id = p.author_id` 조합으로 조회한다.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PostView {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub post: Post,
    pub author_name: Option<String>,
    pub author_username: Option<String>,
    pub author_role: Option<String>,
    pub author_avatar_url: Option<String>,
    pub avg_rating: Option<f64>,
    pub question_count: i64,
    pub liked: bool,
    pub bookmarked: bool,
}

/// `PostView` 용 select 목록. `viewer` 는 `$1::uuid` 같은 플레이스홀더로,
/// NULL 이면 liked/bookmarked 는 false 가 된다.
pub fn post_view_select(viewer: &str) -> String {
    format!(
        r#"p.*,
           au.name AS author_name,
           au.username AS author_username,
           au.role AS author_role,
           au.avatar_url AS author_avatar_url,
           (SELECT AVG(rv.rating)::float8 FROM reactions rv
             WHERE rv.post_id = p.id AND rv.reaction_type = 'review' AND rv.rating IS NOT NULL) AS avg_rating,
           (SELECT COUNT(*) FROM questions qq WHERE qq.post_id = p.id) AS question_count,
           EXISTS(SELECT 1 FROM reactions rl
             WHERE rl.post_id = p.id AND rl.user_id = {viewer} AND rl.reaction_type = 'like') AS liked,
           EXISTS(SELECT 1 FROM reactions rb
             WHERE rb.post_id = p.id AND rb.user_id = {viewer} AND rb.reaction_type = 'bookmark') AS bookmarked"#
    )
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
    pub link_url: Option<String>,
    pub cta_label: Option<String>,
    pub price: Option<i64>,
}
