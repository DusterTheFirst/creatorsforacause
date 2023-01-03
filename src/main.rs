#![forbid(clippy::unwrap_used)]

use std::{borrow::Cow, env, net::SocketAddr, sync::Arc};

use color_eyre::eyre::Context;
use prometheus_client::{
    metrics::info::Info,
    registry::{Registry, Unit},
};
use sentry::SessionMode;
use serde::Deserialize;
use tokio::sync::watch;
use tracing::trace;
use watcher::WatcherEnvironment;

use crate::{
    config::CONFIG,
    metrics::metrics_server,
    watcher::{live_watcher, WatcherDataReceive},
    web::web_server,
};

mod config;
mod metrics;
mod model;
mod tracing_setup;
mod watcher;
mod web;

#[derive(Deserialize, Debug)]
struct OpenTelemetryEnvironment {
    /// API key for honeycomb
    honeycomb_key: String,
    /// Endpoint for collecting opentelemetry metrics
    otlp_endpoint: String,
}

#[derive(Deserialize, Debug)]
struct Environment {
    /// Socket to listen on for the web server
    listen: SocketAddr,

    #[serde(flatten)]
    open_telemetry: Option<OpenTelemetryEnvironment>,

    #[serde(flatten)]
    watcher: WatcherEnvironment,
}

// Since fly.io is a one core machine, we only need the current thread
#[tokio::main(flavor = "current_thread")]
async fn main() -> color_eyre::Result<()> {
    async_main().await
}

// FIXME: color_eyre or better error context providing outside of panics, tracing_error?
async fn async_main() -> color_eyre::Result<()> {
    // Try to load .env file, quietly fail
    let dotenv = dotenv::dotenv();

    // Install pretty error formatting
    color_eyre::install()?;

    let _guard = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        debug: cfg!(debug_assertions),
        dsn: env::var("SENTRY_DSN")
            .ok()
            .map(|dsn| dsn.parse().expect("SENTRY_DSN should be a valid DSN")),
        auto_session_tracking: true,
        session_mode: SessionMode::Application,
        default_integrations: true,
        attach_stacktrace: true,
        server_name: env::var("FLY_REGION").ok().map(Cow::from),
        ..Default::default()
    });

    // Load environment variables
    let environment: Environment = match envy::from_env() {
        Err(envy::Error::MissingValue(missing_env)) => {
            color_eyre::eyre::bail!("missing required environment variable: {missing_env}");
        }
        e => e.wrap_err("failed to get required environment variables")?,
    };

    tracing_setup::setup_tracing(environment.open_telemetry)?;

    if let Ok(path) = dotenv {
        trace!(?path, "Loaded environment variables");
    }

    trace!(?CONFIG, "static config set");

    // TODO: more configuration
    // TODO: respect rate limits
    let reqwest_client = reqwest::Client::builder()
        .build()
        .expect("failed to setup http client");

    let mut registry = <Registry>::default();
    registry.register(
        "build",
        "Information about the current build of the server",
        Info::new([
            ("hash", git_version::git_version!()),
            ("cargo_version", env!("CARGO_PKG_VERSION")),
            ("cargo_name", env!("CARGO_PKG_NAME")),
        ]),
    );
    // registry.register_with_unit(
    //     "watcher_refresh_period",
    //     "The time between refreshes of the watched data",
    //     Unit::Seconds,
    //     metric,
    // );

    let (watcher_sender, watcher_receiver) = watch::channel::<WatcherDataReceive>(None);

    tokio::join!(
        live_watcher(reqwest_client, environment.watcher, &CONFIG, watcher_sender),
        web_server(environment.listen, watcher_receiver),
        metrics_server(Arc::new(registry))
    );

    Ok(())
}
