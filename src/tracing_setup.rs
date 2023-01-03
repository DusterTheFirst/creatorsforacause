use std::str::FromStr;

use color_eyre::eyre::Context;
use opentelemetry::{
    sdk::{
        trace::{RandomIdGenerator, Sampler},
        Resource,
    },
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use tonic::{metadata::MetadataMap, transport::ClientTlsConfig};
use tracing::Level;
use tracing_error::ErrorLayer;
use tracing_subscriber::{filter::Targets, prelude::*, EnvFilter, Registry};

use super::OpenTelemetryEnvironment;

pub(crate) fn setup_tracing(
    environment: Option<OpenTelemetryEnvironment>,
) -> Result<(), color_eyre::Report> {
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
