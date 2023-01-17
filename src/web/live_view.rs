use std::net::SocketAddr;

use axum::{extract::WebSocketUpgrade, http::HeaderValue, routing::get, Router};
use hyper::header;
use tokio::sync::watch;

use crate::watcher::WatcherDataReceive;

use super::markup::{dashboard, DashboardProps};

pub fn router(listen: SocketAddr, watcher_data: watch::Receiver<WatcherDataReceive>) -> Router {
    let view = dioxus_liveview::LiveViewPool::new();

    Router::new()
        .route(
            "/glue.js",
            get(move || async move {
                // TODO: CACHING

                #[cfg(debug_assertions)]
                let domain = &format!("ws://{listen}/live-view/ws");

                #[cfg(not(debug_assertions))]
                let _ = listen;
                #[cfg(not(debug_assertions))]
                let domain = "wss://creatorsforacause.fly.dev/live-view/ws";

                (
                    [(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/javascript"),
                    )],
                    dioxus_liveview::interpreter_glue(domain)
                        .trim_start_matches("\n<script>")
                        .trim_end_matches("</script>\n    ")
                        .to_string(),
                )
            }),
        )
        .route(
            "/ws",
            get(move |ws: WebSocketUpgrade| async move {
                ws.on_upgrade(move |socket| async move {
                    _ = view
                        .launch_with_props(
                            dioxus_liveview::axum_socket(socket),
                            dashboard,
                            DashboardProps {
                                watched_data: watcher_data,
                            },
                        )
                        .await;
                })
            }),
        )
}
