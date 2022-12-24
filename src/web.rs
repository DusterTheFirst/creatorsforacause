use std::{net::SocketAddr, time::Duration};

use askama::Template;
use axum::{body::Bytes, extract::State, routing::get, Json, Router, Server};
use hyper::StatusCode;
use sentry_tower::{SentryHttpLayer, SentryLayer};
use serde_json::{json, Value};
use tower_http::{
    catch_panic::CatchPanicLayer, cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::{error, info};

use crate::{config::Campaign, Creators};

#[tracing::instrument(skip(creators, http_client, tiltify_api_key))]
pub async fn web_server(
    listen: SocketAddr,
    http_client: reqwest::Client,
    tiltify_api_key: String,
    campaign: Campaign,
    creators: Creators,
) {
    let app = Router::new()
        .route("/", get(dashboard).with_state(creators.clone()))
        .route("/healthy", get(|| async { "OK" }))
        .route_service("/streams", get(streams).with_state(creators))
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

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct Dashboard {
    funds: u64,
    creators: Creators,
}

#[axum::debug_handler]
#[tracing::instrument(skip_all)]
async fn dashboard(State(creators): State<Creators>) -> Dashboard {
    Dashboard {
        funds: 100,
        creators,
    }
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
async fn streams(State(creators): State<Creators>) -> Json<Value> {
    Json(json!({
        "youtube": &*creators.youtube(),
        "twitch": &*creators.twitch(),
    }))
}
