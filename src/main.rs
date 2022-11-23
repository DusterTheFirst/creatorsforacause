#![forbid(clippy::unwrap_used)]

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    str::FromStr,
};

use color_eyre::eyre::Context;
use opentelemetry::{
    sdk::{
        trace::{RandomIdGenerator, Sampler},
        Resource,
    },
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::{
    sync::watch,
    task::{JoinHandle, LocalSet},
};
use tonic::{metadata::MetadataMap, transport::ClientTlsConfig};
use tracing::{debug, Instrument, Level};
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    filter::Targets, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};
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

    /// API key for honeycomb
    honeycomb_key: String,
    /// Endpoint for collecting opentelemetry metrics
    otlp_endpoint: String,

    #[serde(flatten)]
    twitch: TwitchEnvironment,

    #[serde(flatten)]
    youtube: YoutubeEnvironment,
}

// Since fly.io is a one core machine, using current thread
// can remove the need for locking and atomics.
#[tokio::main(flavor = "current_thread")]
async fn main() -> color_eyre::Result<()> {
    // Try to load .env file, quietly fail
    let dotenv = dotenv::dotenv();
    // Install pretty error formatting
    color_eyre::install()?;

    // Load environment variables
    let environment: Environment = match envy::from_env() {
        Err(envy::Error::MissingValue(missing_env)) => {
            color_eyre::eyre::bail!("missing required environment variable: {missing_env}");
        }
        e => e.wrap_err("failed to get required environment variables")?,
    };

    // TODO: honeycomb
    // Open-telemetry
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_max_events_per_span(64)
                .with_max_attributes_per_span(16)
                .with_max_links_per_span(16)
                .with_resource(Resource::new([KeyValue::new(
                    opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                    "creatorsforacause",
                )])),
        )
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_tls_config(ClientTlsConfig::new().domain_name("api.honeycomb.io"))
                .with_endpoint(environment.otlp_endpoint)
                .with_metadata({
                    let mut meta = MetadataMap::new();

                    meta.append(
                        "x-honeycomb-team",
                        environment
                            .honeycomb_key
                            .parse()
                            .expect("honeycomb_key should be ascii"),
                    );

                    meta
                }),
        )
        .install_batch(opentelemetry::runtime::TokioCurrentThread)
        .wrap_err("failed to setup opentelemetry exporter")?;

    // Initialize logging
    Registry::default()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env()
                .wrap_err("failed to parse RUST_LOG")?,
        )
        .with(ErrorLayer::default())
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(
                    Targets::from_str("creatorsforacause=trace")
                        .expect("provided targets should be valid"),
                ),
        )
        .init();

    if let Ok(path) = dotenv {
        debug!(?path, "Loaded environment variables");
    }

    // TODO: better file loading
    let creators = std::fs::read("./creators.toml").wrap_err("failed to read creators.toml")?;
    let creators: Creators =
        toml::from_slice(creators.as_slice()).wrap_err("failed to deserialize creators.toml")?;

    dbg!(&creators);

    // TODO: more configuration
    let reqwest_client = reqwest::Client::builder()
        .build()
        .expect("failed to setup http client");

    let local_set = LocalSet::new();

    let (youtube_live_status_sender, youtube_live_status_receiver) =
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

    local_set.await;

    Ok(())
}
