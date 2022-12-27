use std::net::SocketAddr;

use askama::Template;
use axum::{extract::WebSocketUpgrade, routing::get, Router};
use tokio::sync::watch;

use crate::watcher::WatcherDataReceive;

use super::markup::{dashboard, DashboardProps};

#[derive(Debug, Template)]
#[template(path = "dashboard.html")]
struct Dashboard {
    glue: String,
}

pub(super) fn router(
    listen: SocketAddr,
    watcher_data: watch::Receiver<WatcherDataReceive>,
) -> Router {
    let view = dioxus_liveview::LiveViewPool::new();

    Router::new()
        .route(
            "/",
            get(move || async move {
                #[cfg(debug_assertions)]
                let domain = &format!("ws://{listen}/ws");

                #[cfg(not(debug_assertions))]
                let _ = listen;
                #[cfg(not(debug_assertions))]
                let domain = "wss://creatorsforacause.fly.dev/ws";

                Dashboard {
                    // FIXME: wss on https, and use correct domain.
                    glue: dioxus_liveview::interpreter_glue(domain),
                }
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
