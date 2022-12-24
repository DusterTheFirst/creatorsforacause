#![forbid(clippy::unwrap_used)]

use std::{borrow::Cow, env, net::SocketAddr, str::FromStr};

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
use tonic::{metadata::MetadataMap, transport::ClientTlsConfig};
use tracing::{trace, Level};
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    filter::Targets, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};
use twitch::TwitchEnvironment;
use youtube::YoutubeEnvironment;

use crate::{
    config::CONFIG, model::CreatorsWatcher, twitch::twitch_live_watcher, web::web_server,
    youtube::youtube_live_watcher,
};

mod config;
mod model;
mod twitch;
mod web;
mod youtube;

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
    twitch: TwitchEnvironment,

    #[serde(flatten)]
    youtube: YoutubeEnvironment,

    /// API key for tiltify
    tiltify_api_key: String,
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

    setup_tracing(environment.open_telemetry)?;

    if let Ok(path) = dotenv {
        trace!(?path, "Loaded environment variables");
    }

    trace!(?CONFIG, "static config set");

    // TODO: more configuration
    // TODO: respect rate limits
    let reqwest_client = reqwest::Client::builder()
        .build()
        .expect("failed to setup http client");

    let (creators, twitch_writer, youtube_writer) = CreatorsWatcher::new();

    tokio::join!(
        twitch_live_watcher(
            reqwest_client.clone(),
            environment.twitch,
            CONFIG.creators.twitch,
            twitch_writer,
        ),
        youtube_live_watcher(
            reqwest_client.clone(),
            environment.youtube,
            CONFIG.creators.youtube,
            youtube_writer,
        ),
        web_server(
            environment.listen,
            reqwest_client,
            environment.tiltify_api_key,
            CONFIG.campaign,
            creators,
        )
    );

    Ok(())
}

fn setup_tracing(environment: Option<OpenTelemetryEnvironment>) -> Result<(), color_eyre::Report> {
    let tracer = environment
        .map(|environment| {
            opentelemetry_otlp::new_pipeline()
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
        })
        .transpose()
        .wrap_err("failed to setup opentelemetry exporter")?;

    Registry::default()
        .with(tracing_subscriber::fmt::layer().pretty())
        .with(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env()
                .wrap_err("failed to parse RUST_LOG")?,
        )
        .with(ErrorLayer::default())
        .with(tracer.map(|tracer| {
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(
                    Targets::from_str("creatorsforacause=trace")
                        .expect("provided targets should be valid"),
                )
        }))
        .with(sentry::integrations::tracing::layer())
        .init();

    Ok(())
}
