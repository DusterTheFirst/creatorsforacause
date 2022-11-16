use std::{collections::HashMap, net::SocketAddr};

use axum::{routing::get, Router};
use color_eyre::eyre::Context;
use serde::Deserialize;
use tokio::runtime::Builder;
use tracing::info;

#[derive(Deserialize, Debug)]
struct Creators {
    twitch: HashMap<String, String>,
    youtube: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
struct Environment {
    /// Socket to listen on for the web server
    listen: Option<SocketAddr>,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // TODO: honeycomb
    tracing_subscriber::fmt().init();

    let environment: Environment = match envy::from_env() {
        Err(envy::Error::MissingValue(missing_env)) => {
            color_eyre::eyre::bail!("missing required environment variable: {missing_env}");
        }
        e => e.wrap_err("failed to get required environment variables")?,
    };

    // TODO: better file loading
    let creators = std::fs::read("./creators.toml").wrap_err("failed to read creators.toml")?;
    let creators: Creators =
        toml::from_slice(creators.as_slice()).wrap_err("failed to deserialize creators.toml")?;

    dbg!(&creators);

    // TODO: more configuration
    let http_client = reqwest::Client::builder()
        .build()
        .expect("failed to setup http client");

    // Since fly.io is a one core machine, using current thread
    // can remove the need for locking and atomics.
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .wrap_err("unable to create tokio runtime")?;

    runtime.spawn(twitch_live_watcher(http_client, creators.twitch));
    runtime.block_on(web_server(
        environment
            .listen
            .unwrap_or_else(|| "127.0.0.1:8080".parse().unwrap()),
    ))
}

async fn twitch_live_watcher(http_client: reqwest::Client, creators: HashMap<String, String>) {
    info!(?creators, "Starting live status watch of twitch creators");

    let client = twitch_api::HelixClient::with_client(http_client);

    client.ev
}

async fn web_server(listen: SocketAddr) -> color_eyre::Result<()> {
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    info!("Starting web server on http://{listen}");

    axum::Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .wrap_err("axum server ran into a problem")
}
