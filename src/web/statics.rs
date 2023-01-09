use axum::{http::HeaderValue, routing::get, Router};
use hyper::header;

pub fn router() -> Router {
    Router::new()
        .route(
            "/date_renderer.js",
            get(|| async {
                (
                    [(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/javascript"),
                    )],
                    include_str!("../../static/date_renderer.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, HeaderValue::from_static("text/css"))],
                    include_str!("../../static/style.css"),
                )
            }),
        )
}
