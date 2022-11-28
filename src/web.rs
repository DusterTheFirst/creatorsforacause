use std::{collections::HashMap, hash::Hash, net::SocketAddr};

use axum::{extract::State, routing::get, Json, Router, Server};
use sentry_tower::{SentryHttpLayer, SentryLayer};
use serde::Serialize;
use serde_json::{json, Value};
use time::OffsetDateTime;
use tokio::sync::watch;
use tracing::info;
use twitch_api::types::Nickname;

use crate::youtube::YoutubeHandle;

#[derive(Debug, Serialize)]

pub struct LiveStreamList<Key: Hash + Eq> {
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub streams: HashMap<Key, Option<LiveStreamDetails>>,
}

#[derive(Debug, Serialize)]
pub struct LiveStreamDetails {
    pub href: String,
    pub title: String,
    pub start_time: String,
    pub viewers: u32,
}

#[tracing::instrument(skip(youtube_livestreams, twitch_livestreams))]
pub async fn web_server(
    listen: SocketAddr,
    youtube_livestreams: watch::Receiver<LiveStreamList<YoutubeHandle>>,
    twitch_livestreams: watch::Receiver<LiveStreamList<Nickname>>,
) {
    let app = Router::new()
        .route("/", get(|| async { "OK" }))
        .route_service(
            "/status",
            get(status).with_state((youtube_livestreams, twitch_livestreams)),
        )
        .layer(SentryLayer::new_from_top())
        .layer(SentryHttpLayer::with_transaction());

    info!("Starting web server on http://{listen}");

    Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .expect("axum server ran into a problem")
}

#[axum::debug_handler]
#[allow(clippy::type_complexity)]
async fn status(
    State((youtube_status, twitch_live_streams)): State<(
        watch::Receiver<LiveStreamList<YoutubeHandle>>,
        watch::Receiver<LiveStreamList<Nickname>>,
    )>,
) -> Json<Value> {
    Json(json!({
        "youtube": &*youtube_status.borrow(),
        "twitch": &*twitch_live_streams.borrow(),
    }))
}
