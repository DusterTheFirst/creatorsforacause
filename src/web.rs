use std::{collections::HashMap, hash::Hash, net::SocketAddr, time::Duration};

use axum::{body::Bytes, extract::State, routing::get, Json, Router, Server};
use hyper::StatusCode;
use sentry_tower::{SentryHttpLayer, SentryLayer};
use serde::Serialize;
use serde_json::{json, Value};
use time::OffsetDateTime;
use tokio::sync::watch;
use tower_http::{
    catch_panic::CatchPanicLayer, cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::{error, info};
use twitch_api::types::Nickname;

use crate::{youtube::YoutubeHandle, Campaign};

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
    http_client: reqwest::Client,
    tiltify_api_key: String,
    campaign: Campaign,
    youtube_livestreams: watch::Receiver<LiveStreamList<YoutubeHandle>>,
    twitch_livestreams: watch::Receiver<LiveStreamList<Nickname>>,
) {
    let app = Router::new()
        .route("/", get(|| async { "OK" }))
        .route_service(
            "/streams",
            get(streams).with_state((youtube_livestreams, twitch_livestreams)),
        )
        .route_service(
            "/fundraiser",
            get(fundraiser).with_state((tiltify_api_key, campaign, http_client)),
        )
        .layer(
            tower::ServiceBuilder::new()
                .layer(SentryLayer::new_from_top())
                .layer(SentryHttpLayer::with_transaction())
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(CorsLayer::permissive())
                .layer(CatchPanicLayer::new()),
        );

    info!("Starting web server on http://{listen}");

    Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .expect("axum server ran into a problem")
}

#[axum::debug_handler]
#[tracing::instrument(skip_all)]
async fn fundraiser(
    State((tiltify_api_key, campaign, http_client)): State<(String, Campaign, reqwest::Client)>,
) -> Result<Bytes, StatusCode> {
    let request = http_client
        .get(format!(
            "https://tiltify.com/api/v3/campaigns/{}",
            campaign.id
        ))
        .bearer_auth(tiltify_api_key)
        .build()
        .expect("tiltify request should be well formed");

    let response = http_client
        .execute(request)
        .await
        .expect("tiltify api request failed");

    response.bytes().await.map_err(|err| {
        error!(%err, "failed to read tiltify body");

        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[axum::debug_handler]
#[allow(clippy::type_complexity)]
async fn streams(
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
