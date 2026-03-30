mod auth;
mod posts;
mod reactions;
mod feed;
mod search;
mod trending;
mod profile;
mod interactions;

use axum::Router;
use crate::db::Db;

pub fn router() -> Router<Db> {
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
