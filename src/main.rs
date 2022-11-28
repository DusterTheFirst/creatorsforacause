#![forbid(clippy::unwrap_used)]

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    env,
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
use sentry::SessionMode;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::{
    sync::watch,
    task::{JoinHandle, LocalSet},
};
use tonic::{metadata::MetadataMap, transport::ClientTlsConfig};
use tracing::{trace, Level};
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    filter::Targets, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};
use twitch::TwitchEnvironment;
use youtube::{YoutubeEnvironment, YoutubeHandle};

use crate::{
    twitch::twitch_live_watcher,
    web::{web_server, LiveStreamList},
    youtube::youtube_live_watcher,
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
    listen: SocketAddr,

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
    async_main().await
}

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
        .with(sentry::integrations::tracing::layer())
        .init();

    if let Ok(path) = dotenv {
        trace!(?path, "Loaded environment variables");
    }

    // TODO: better file loading
    let creators = std::fs::read("./creators.toml").wrap_err("failed to read creators.toml")?;
    let creators: Creators =
        toml::from_slice(creators.as_slice()).wrap_err("failed to deserialize creators.toml")?;

    trace!(?creators, "loaded creators.toml");

    // TODO: more configuration
    let reqwest_client = reqwest::Client::builder()
        .build()
        .expect("failed to setup http client");

    let local_set = LocalSet::new();

    // We have to use "Sync" channels over Rc<RefCell<_>> since axum requires all state be sync
    // even though we are guaranteed to be on the same thread (single threaded async runtime)
    let (youtube_live_streams_writer, youtube_live_streams_reader) =
        watch::channel(LiveStreamList {
            updated: OffsetDateTime::UNIX_EPOCH,
            streams: HashMap::new(),
        });

    let (twitch_live_streams_writer, twitch_live_streams_reader) = watch::channel(LiveStreamList {
        updated: OffsetDateTime::UNIX_EPOCH,
        streams: HashMap::new(),
    });

    let _: JoinHandle<()> = local_set.spawn_local(twitch_live_watcher(
        reqwest_client.clone(),
        environment.twitch,
        creators.twitch,
        twitch_live_streams_writer,
    ));
    let _: JoinHandle<()> = local_set.spawn_local(youtube_live_watcher(
        reqwest_client,
        environment.youtube,
        creators.youtube,
        youtube_live_streams_writer,
    ));
    let _: JoinHandle<()> = local_set.spawn_local(web_server(
        environment.listen,
        youtube_live_streams_reader,
        twitch_live_streams_reader,
    ));

    local_set.await;

    Ok(())
}
