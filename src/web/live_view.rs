use std::net::SocketAddr;

use askama::Template;
use axum::{extract::WebSocketUpgrade, routing::get, Router};

use crate::model::CreatorsWatcher;

use super::markup::{DashboardProps, dashboard};

#[derive(Debug, Template)]
#[template(path = "dashboard.html")]
struct Dashboard {
    glue: String,
}

pub(super) fn router(listen: SocketAddr, creators: CreatorsWatcher) -> Router {
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
                ws.on_upgrade(move |socket| {
                    let twitch = creators.twitch().borrow().clone();
                    let youtube = creators.youtube().borrow().clone();

                    async move {
                        _ = view
                            .launch_with_props(
                                dioxus_liveview::axum_socket(socket),
                                dashboard,
                                DashboardProps { twitch, youtube },
                            )
                            .await;
                    }
                })
            }),
        )
}
