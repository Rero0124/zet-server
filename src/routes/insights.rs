use axum::{
    Router,
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;

pub fn router() -> Router<Db> {
    Router::new()
        .route("/posts/{id}/insights", get(post_insights))
}

#[derive(Debug, Deserialize)]
struct InsightsQuery {
    author_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct PostCounters {
    author_id: Option<Uuid>,
    impressions: i64,
    clicks: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct Engagement {
    cta_clicks: i64,
    avg_dwell_ms: i64,
    likes: i64,
    bookmarks: i64,
    reviews: i64,
    questions: i64,
    avg_rating: Option<f64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SeriesPoint {
    date: String,
    impressions: i64,
    clicks: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct AudienceRow {
    dim: String,
    label: String,
    count: i64,
}

/// 게시글 성과 (작성자 본인만). 노출/클릭/CTA/체류시간/반응/최근 14일 추이/오디언스 분포.
async fn post_insights(
    State(pool): State<Db>,
    Path(id): Path<Uuid>,
    Query(query): Query<InsightsQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let post = sqlx::query_as::<_, PostCounters>(
        "SELECT author_id, impressions, clicks FROM posts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let Some(post) = post else {
        return Err((StatusCode::NOT_FOUND, Json(json!({"error": "Post not found"}))));
    };
    if post.author_id != Some(query.author_id) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "본인 게시글의 성과만 볼 수 있습니다"}))));
    }

    let eng = sqlx::query_as::<_, Engagement>(
        r#"SELECT
             (SELECT COUNT(DISTINCT user_id) FROM interactions WHERE post_id = $1 AND interaction_type = 'cta') AS cta_clicks,
             (SELECT COALESCE(AVG(duration_ms), 0)::bigint FROM interactions WHERE post_id = $1 AND interaction_type = 'dwell') AS avg_dwell_ms,
             (SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND reaction_type = 'like') AS likes,
             (SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND reaction_type = 'bookmark') AS bookmarks,
             (SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND reaction_type = 'review') AS reviews,
             (SELECT COUNT(*) FROM questions WHERE post_id = $1) AS questions,
             (SELECT AVG(rating)::float8 FROM reactions WHERE post_id = $1 AND reaction_type = 'review' AND rating IS NOT NULL) AS avg_rating"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // 최근 14일(오늘 포함) 일별 노출/클릭, 빈 날은 0 으로 채움
    let series = sqlx::query_as::<_, SeriesPoint>(
        r#"SELECT to_char(d.day, 'YYYY-MM-DD') AS date,
                  COALESCE(s.impressions, 0) AS impressions,
                  COALESCE(s.clicks, 0) AS clicks
           FROM generate_series(CURRENT_DATE - 13, CURRENT_DATE, '1 day'::interval) AS d(day)
           LEFT JOIN (
             SELECT created_at::date AS day,
                    COUNT(DISTINCT user_id) FILTER (WHERE interaction_type = 'impression') AS impressions,
                    COUNT(DISTINCT user_id) FILTER (WHERE interaction_type = 'click') AS clicks
             FROM interactions
             WHERE post_id = $1 AND created_at >= CURRENT_DATE - 13
             GROUP BY 1
           ) s ON s.day = d.day::date
           ORDER BY d.day"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    // 이 게시글에 인터랙션 또는 반응을 남긴 고유 사용자의 인구통계 분포
    let audience = sqlx::query_as::<_, AudienceRow>(
        r#"WITH aud AS (
             SELECT user_id FROM interactions WHERE post_id = $1
             UNION
             SELECT user_id FROM reactions WHERE post_id = $1
           ),
           au AS (SELECT u.birth_date, u.gender, u.region FROM aud JOIN users u ON u.id = aud.user_id)
           SELECT 'age' AS dim,
                  CASE WHEN birth_date IS NULL THEN '미입력' ELSE age_group_from_birth(birth_date) END AS label,
                  COUNT(*) AS count
           FROM au GROUP BY 2
           UNION ALL
           SELECT 'gender', COALESCE(NULLIF(gender, ''), '미입력'), COUNT(*) FROM au GROUP BY 2
           UNION ALL
           SELECT 'region', COALESCE(NULLIF(region, ''), '미입력'), COUNT(*) FROM au GROUP BY 2
           ORDER BY dim, count DESC, label"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))))?;

    let bucket = |dim: &str| -> Vec<Value> {
        audience.iter()
            .filter(|r| r.dim == dim)
            .map(|r| json!({ "label": r.label, "count": r.count }))
            .collect()
    };

    let ctr = if post.impressions > 0 {
        (post.clicks as f64 * 100.0 / post.impressions as f64 * 10.0).round() / 10.0
    } else {
        0.0
    };

    Ok(Json(json!({
        "insights": {
            "impressions": post.impressions,
            "clicks": post.clicks,
            "cta_clicks": eng.cta_clicks,
            "ctr": ctr,
            "avg_dwell_ms": eng.avg_dwell_ms,
            "likes": eng.likes,
            "bookmarks": eng.bookmarks,
            "reviews": eng.reviews,
            "questions": eng.questions,
            "avg_rating": eng.avg_rating,
            "series": series,
            "audience": {
                "age": bucket("age"),
                "gender": bucket("gender"),
                "region": bucket("region"),
            },
        }
    })))
}
