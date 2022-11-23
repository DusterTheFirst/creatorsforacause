#![forbid(clippy::unwrap_used)]

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::SocketAddr,
};

use color_eyre::eyre::Context;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::{
    runtime::Builder,
    sync::watch,
    task::{JoinHandle, LocalSet},
};
use tracing::{debug, Instrument, Level};
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter};
use twitch::TwitchEnvironment;
use youtube::{YoutubeEnvironment, YoutubeHandle};

use crate::{
    twitch::twitch_live_watcher,
    web::web_server,
    youtube::{youtube_live_watcher, YoutubeLiveStreams},
};

mod twitch;
mod web;
mod youtube;

#[derive(Deserialize, Debug)]
struct Creators {
    twitch: HashSet<twitch_api::types::UserName>,
    youtube: HashSet<YoutubeHandle>,
}

#[derive(Deserialize, Debug)]
struct Environment {
    /// Socket to listen on for the web server
    listen: Option<SocketAddr>,

    #[serde(flatten)]
    twitch: TwitchEnvironment,

    #[serde(flatten)]
    youtube: YoutubeEnvironment,
}

fn main() -> color_eyre::Result<()> {
    // Try to load .env file, quietly fail
    let dotenv = dotenv::dotenv();
    // Install pretty error formatting
    color_eyre::install()?;

    // TODO: honeycomb
    // Initialize logging

    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(
                EnvFilter::builder()
                    .with_default_directive(Level::INFO.into())
                    .from_env()
                    .wrap_err("failed to parse RUST_LOG")?,
            )
            .with(ErrorLayer::default()),
    )
    .expect("tracing subscriber should set properly");

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
    let reqwest_client = reqwest::Client::builder()
        .build()
        .expect("failed to setup http client");

    // Since fly.io is a one core machine, using current thread
    // can remove the need for locking and atomics.
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .wrap_err("unable to create tokio runtime")?;

    let local_set = LocalSet::new();

    let (youtube_live_status_sender, mut youtube_live_status_receiver) =
        watch::channel(YoutubeLiveStreams {
            updated: OffsetDateTime::UNIX_EPOCH,
            streams: HashMap::new(),
        });

    // let _: JoinHandle<()> = local_set.spawn_local(async move {
    //     twitch_live_watcher(reqwest_client, environment.twitch, creators.twitch)
    //         .await
    //         .expect("web server encountered an un-recoverable error")
    // });
    let _: JoinHandle<()> = local_set.spawn_local(youtube_live_watcher(
        reqwest_client,
        environment.youtube,
        creators.youtube,
        youtube_live_status_sender,
    ));
    // local_set.spawn_local(async move {
    //     loop {
    //         {
    //             let status = &*youtube_live_status_receiver.borrow_and_update();

    //             println!(
    //                 "{}",
    //                 serde_json::to_string_pretty(status)
    //                     .expect("status should be serializable to json")
    //             );
    //         }

    //         youtube_live_status_receiver
    //             .changed()
    //             .await
    //             .expect("receiver should not produce an error");
    //     }
    // });
    let _: JoinHandle<()> = local_set.spawn_local(async move {
        web_server(
            environment.listen.unwrap_or_else(|| {
                "127.0.0.1:8080"
                    .parse()
                    .expect("default socket address should be a valid socket address")
            }),
            youtube_live_status_receiver,
        )
        .await
        .expect("web server encountered an un-recoverable error")
    });

    runtime.block_on(local_set);

    Ok(())
}
