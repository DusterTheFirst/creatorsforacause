use std::net::SocketAddr;

use axum::{extract::State, handler::Handler, routing::get, Json, Router, Server};
use color_eyre::eyre::{bail, Context};
use serde_json::{json, Value};
use tokio::sync::watch;
use tracing::info;

use crate::youtube::YoutubeLiveStreams;

pub async fn web_server(
    listen: SocketAddr,
    youtube_status: watch::Receiver<YoutubeLiveStreams>,
) -> color_eyre::Result<()> {
    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route_service("/status", status.with_state(youtube_status));

    info!("Starting web server on http://{listen}");

    Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .wrap_err("axum server ran into a problem")
}

async fn status(State(youtube_status): State<watch::Receiver<YoutubeLiveStreams>>) -> Json<Value> {
    Json(json!({
        "youtube": &*youtube_status.borrow(),
    }))
}
