use std::{net::SocketAddr, time::Duration};

use axum::{body::Bytes, extract::State, routing::get, Json, Router, Server};
use hyper::StatusCode;
use sentry_tower::{SentryHttpLayer, SentryLayer};
use tokio::sync::watch;
use tower_http::{
    catch_panic::CatchPanicLayer, cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::{error, info};

use crate::{config::CampaignConfig, watcher::WatcherDataReceive};

mod live_view;
mod markup;

#[tracing::instrument(skip(watcher_data, http_client))]
pub async fn web_server(
    listen: SocketAddr,
    http_client: reqwest::Client,
    watcher_data: watch::Receiver<WatcherDataReceive>,
) {
    let app = Router::new()
        .nest("/", live_view::router(listen, watcher_data.clone()))
        .route("/healthy", get(|| async { "OK" }))
        .route_service("/json", get(json).with_state(watcher_data))
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
    State((tiltify_api_key, campaign, http_client)): State<(
        String,
        CampaignConfig,
        reqwest::Client,
    )>,
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
async fn json(
    State(watcher_data): State<watch::Receiver<WatcherDataReceive>>,
) -> Json<WatcherDataReceive> {
    Json(watcher_data.borrow().as_ref().cloned())
}
