use std::net::SocketAddr;

use axum::{routing::get, Router, Server};
use color_eyre::eyre::{bail, Context};
use tracing::info;

pub async fn web_server(listen: SocketAddr) -> color_eyre::Result<()> {
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    info!("Starting web server on http://{listen}");

    Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .wrap_err("axum server ran into a problem")
}
