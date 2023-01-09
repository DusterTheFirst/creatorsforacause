use axum::{Router, routing::get};

pub fn router() -> Router {
    Router::new().route(
        "/.well-known/security.txt",
        get(|| async { include_str!("../../.well-known/security.txt") }),
    )
}