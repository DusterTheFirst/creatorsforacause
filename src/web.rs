use std::{net::SocketAddr, time::Duration};

use axum::{extract::State, routing::get, Json, Router, Server};
use hyper::header::{self, HeaderValue};
use sentry_tower::{SentryHttpLayer, SentryLayer};
use tokio::sync::watch;
use tower_http::{
    catch_panic::CatchPanicLayer, cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::info;

use crate::{watcher::WatcherDataReceive, web::layers::helmet};

mod layers;
mod live_view;
mod markup;
mod statics;
mod well_known;

pub async fn web_server(listen: SocketAddr, watcher_data: watch::Receiver<WatcherDataReceive>) {
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, HeaderValue::from_static("text/html"))],
                    include_str!("../static/dashboard.html"),
                )
            }),
        )
        .nest("/live-view", live_view::router(listen, watcher_data.clone()))
        .route("/healthy", get(|| async { "OK" }))
        .route_service("/json", get(json).with_state(watcher_data))
        .nest("/.well-known", well_known::router())
        .nest("/static", statics::router())
        .layer(
            tower::ServiceBuilder::new()
                .layer(SentryLayer::new_from_top())
                .layer(SentryHttpLayer::with_transaction())
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(helmet::layer())
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
async fn json(
    State(watcher_data): State<watch::Receiver<WatcherDataReceive>>,
) -> Json<WatcherDataReceive> {
    Json(watcher_data.borrow().as_ref().cloned())
}
