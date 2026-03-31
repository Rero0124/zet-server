mod auth;
mod posts;
mod reactions;
mod feed;
mod search;
mod trending;
mod profile;
mod interactions;
pub mod ai;
mod upload;

use std::sync::Arc;
use axum::Router;
use crate::db::Db;
use crate::storage::Storage;

pub fn api_router() -> Router<Db> {
    Router::new()
        .merge(auth::router())
        .merge(posts::router())
        .merge(reactions::router())
        .merge(feed::router())
        .merge(search::router())
        .merge(trending::router())
        .merge(profile::router())
        .merge(interactions::router())
}

pub fn upload_router() -> Router<(Db, Arc<dyn Storage>)> {
    upload::router()
}

pub fn ai_router() -> Router<Db> {
    ai::router()
}
