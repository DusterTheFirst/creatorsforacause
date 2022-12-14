use std::{
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
};

use axum::{extract::State, response::Redirect, routing::get, Router, Server};
use hyper::StatusCode;
use prometheus_client::registry::Registry;
use sentry_tower::{SentryHttpLayer, SentryLayer};
use tower_http::catch_panic::CatchPanicLayer;
use tracing::{error, info};

pub mod gauge_info;

pub mod types {
    use std::sync::atomic::{AtomicI64, AtomicU64};

    use prometheus_client::{
        encoding::EncodeLabelSet,
        metrics::{counter::Counter, family::Family, gauge::Gauge},
    };

    use crate::model::StreamingService;

    pub type WatcherRefreshPeriodMetric = Gauge<i64, AtomicI64>;
    pub type LiveCreatorsMetric = Family<StreamingServiceMetricKey, Gauge<i64, AtomicI64>>;
    pub type YoutubeQuotaUsageMetric = Counter<u64, AtomicU64>;

    #[derive(Debug, Clone, PartialEq, Eq, Hash, EncodeLabelSet)]
    pub struct StreamingServiceMetricKey {
        pub service: StreamingService,
        pub username: String,
        pub id: String,
    }
}

pub async fn metrics_server(registry: Arc<Registry>) {
    let router = Router::new()
        .route("/metrics", get(metrics).with_state(registry))
        .fallback(|| async { Redirect::to("/metrics") })
        .layer(
            tower::ServiceBuilder::new()
                .layer(SentryLayer::new_from_top())
                .layer(SentryHttpLayer::with_transaction())
                .layer(CatchPanicLayer::new()),
        );

    info!("Starting metrics server on http://localhost:9091");

    let listen = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 9091);
    Server::bind(&listen.into())
        .serve(router.into_make_service())
        .await
        .expect("axum server ran into a problem")
}

#[tracing::instrument(skip_all)]
#[axum::debug_handler]
async fn metrics(State(registry): State<Arc<Registry>>) -> Result<String, StatusCode> {
    let mut buffer = String::new();

    // TODO: "application/openmetrics-text; version=1.0.0; charset=utf-8"
    match prometheus_client::encoding::text::encode(&mut buffer, &registry) {
        Ok(()) => Ok(buffer),
        Err(error) => {
            error!(?error, "failed to encode prometheus data");

            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
