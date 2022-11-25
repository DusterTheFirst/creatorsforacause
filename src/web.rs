use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::State,
    handler::Handler,
    routing::{get, post},
    Json, Router, Server,
};
use color_eyre::eyre::Context;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::watch;
use tracing::info;

use crate::{twitch::handle_eventsub, youtube::YoutubeLiveStreams};

#[derive(Debug, Serialize)]
pub struct LiveStreamDetails {
    pub href: String,
    pub title: String,
    pub start_time: String,
    pub concurrent_viewers: String,
}

#[tracing::instrument(skip(youtube_status, eventsub_secret))]
pub async fn web_server(
    listen: SocketAddr,
    youtube_status: watch::Receiver<YoutubeLiveStreams>,
    eventsub_secret: Arc<str>,
) -> color_eyre::Result<()> {
    let app = Router::new()
        .route("/", get(|| async { "OK" }))
        .route_service("/status", status.with_state(youtube_status))
        .route_service(
            "/twitch/eventsub",
            post(handle_eventsub).with_state(eventsub_secret),
        );

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
