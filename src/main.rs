use std::{collections::{HashMap, HashSet}, future::Future, net::SocketAddr};

use color_eyre::eyre::Context;
use serde::Deserialize;
use tokio::{runtime::Builder, task::LocalSet};
use tracing::{debug, Level};
use tracing_subscriber::EnvFilter;
use twitch::TwitchEnvironment;

use crate::{twitch::twitch_live_watcher, web::web_server};

mod twitch;
mod web;

#[derive(Deserialize, Debug)]
struct Creators {
    twitch: HashSet<twitch_api::types::UserName>,
    youtube: HashMap<String, String>,
}

#[derive(Deserialize, Debug)]
struct Environment {
    /// Socket to listen on for the web server
    listen: Option<SocketAddr>,

    #[serde(flatten)]
    twitch: TwitchEnvironment,
}

fn main() -> color_eyre::Result<()> {
    // Try to load .env file, quietly fail
    let dotenv = dotenv::dotenv();
    // Install pretty error formatting
    color_eyre::install()?;

    // TODO: honeycomb
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env()
                .wrap_err("failed to parse RUST_LOG")?,
        )
        .init();

    if let Ok(path) = dotenv {
        debug!(?path, "Loaded environment variables");
    }

    // Load environment variables
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

    let local_set = LocalSet::new();

    local_set.spawn_local(async move {
        twitch_live_watcher(http_client, environment.twitch, creators.twitch)
            .await
            .expect("web server encountered an un-recoverable error")
    });
    local_set.spawn_local(async move {
        web_server(
            environment
                .listen
                .unwrap_or_else(|| "127.0.0.1:8080".parse().unwrap()),
        )
        .await
        .expect("web server encountered an un-recoverable error")
    });

    runtime.block_on(local_set);

    Ok(())
}
