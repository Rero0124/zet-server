//! 여러 라우트가 함께 쓰는 작은 헬퍼 모음.

use axum::{Json, http::StatusCode};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

pub type ApiError = (StatusCode, Json<Value>);

/// 필드 생략(None) / null(Some(None)) / 값(Some(Some(v))) 을 구분하는 serde 헬퍼.
/// `#[serde(default, deserialize_with = "deserialize_some")]` 와 함께 쓴다.
pub fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// 공백뿐인 문자열은 NULL 로 취급 (PUT 의 "값 지우기" 용).
pub fn blank_to_null(v: Option<Option<String>>) -> Option<Option<String>> {
    v.map(|inner| inner.filter(|s| !s.trim().is_empty()))
}

/// 공백뿐인 문자열은 None 으로.
pub fn non_blank(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.trim().is_empty())
}

/// ILIKE 패턴에 넣기 전에 `\`, `%`, `_` 를 이스케이프.
pub fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// 공백으로 나눈 토큰을 `tok:* | tok:*` 형태의 tsquery 로 만든다.
/// 토큰 앞의 `#` 은 떼고, 영숫자(한글 포함)만 남긴다. 유효 토큰이 없으면 빈 문자열.
pub fn build_tsquery(q: &str) -> String {
    q.split_whitespace()
        .map(|tok| {
            tok.trim_start_matches('#')
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|tok| !tok.is_empty())
        .map(|tok| format!("{tok}:*"))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// DB 에러 → API 에러. UNIQUE 위반(23505)은 409 와 한국어 메시지로 바꾸고,
/// 그 외는 `fallback` 상태코드에 원문 메시지를 담는다.
pub fn db_err(e: sqlx::Error, fallback: StatusCode) -> ApiError {
    if let sqlx::Error::Database(db) = &e {
        if db.code().as_deref() == Some("23505") {
            let msg = match db.constraint() {
                Some("users_email_key") => "이미 가입된 이메일입니다",
                Some("users_username_key") => "이미 사용 중인 아이디입니다",
                _ => "이미 존재하는 데이터입니다",
            };
            return (StatusCode::CONFLICT, Json(json!({"error": msg})));
        }
    }
    (fallback, Json(json!({"error": e.to_string()})))
}
