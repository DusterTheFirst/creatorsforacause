use std::{net::SocketAddr, time::Duration};

use axum::{body::Bytes, extract::State, routing::get, Json, Router, Server};
use hyper::StatusCode;
use sentry_tower::{SentryHttpLayer, SentryLayer};
use serde_json::{json, Value};
use tower_http::{
    catch_panic::CatchPanicLayer, cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::{error, info};

use crate::{config::Campaign, CreatorsWatcher};

#[tracing::instrument(skip(creators, http_client, tiltify_api_key))]
pub async fn web_server(
    listen: SocketAddr,
    http_client: reqwest::Client,
    tiltify_api_key: String,
    campaign: Campaign,
    creators: CreatorsWatcher,
) {
    let app = Router::new()
        .nest("/", live_view::router(listen, creators.clone()))
        .route("/healthy", get(|| async { "OK" }))
        .route_service("/streams", get(streams).with_state(creators))
        .route_service(
            "/fundraiser",
            get(fundraiser).with_state((tiltify_api_key, campaign, http_client)),
        )
        .layer(
            tower::ServiceBuilder::new()
                .layer(SentryLayer::new_from_top())
                .layer(SentryHttpLayer::with_transaction())
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(CorsLayer::permissive())
                .layer(CatchPanicLayer::new()),
        );

    info!("Starting web server on http://{listen}");

    Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .expect("axum server ran into a problem")
}

mod live_view {
    use std::net::SocketAddr;

    use askama::Template;
    use axum::{extract::WebSocketUpgrade, routing::get, Router};
    use dioxus::prelude::*;

    use crate::model::{Creator, CreatorsList, CreatorsWatcher};

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
                    Dashboard {
                        glue: dioxus_liveview::interpreter_glue(&format!("ws://{listen}/ws")),
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

    #[derive(Debug, Props)]
    struct CreatorCardProps<'c> {
        creator: &'c Creator,
    }

    fn creator_card<'s>(cx: Scope<'s, CreatorCardProps<'s>>) -> Element<'s> {
        let creator = cx.props.creator;

        let class = if creator.stream.is_some() {
            "creator live"
        } else {
            "creator"
        };

        cx.render(rsx! {
            div {
                class: class,
                img {
                    src: "{creator.icon_url}",
                    alt: "Profile Picture",
                    // loading: "lazy",
                }
                h4 {
                    class: "display_name",
                    a {
                        href: "{creator.href}",
                        "{creator.display_name}"
                    }
                }
                {
                    creator.stream.as_ref().map(|stream| {
                        rsx! {
                            div {
                                h5 { "Stream" }
                                p { "Title: " a { href: "{stream.href}", "{stream.title}" } }
                                p { "Start Time: {stream.start_time}" }
                                p { "Viewers: "
                                    if let Some(viewers) = stream.viewers {
                                        rsx! { "{viewers}" }
                                    } else {
                                        rsx! { "Hidden By Creator" }
                                    }
                                }
                            }
                        }
                    })
                }
            }
        })
    }

    #[derive(Debug, Props, PartialEq, Eq)]
    pub struct DashboardProps {
        pub twitch: CreatorsList,
        pub youtube: CreatorsList,
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn dashboard<'s>(cx: Scope<'s, DashboardProps>) -> Element<'s> {
        let funds = 0;

        cx.render(rsx! {
            main {
                h1 { "Creators for a Cause" }
                section {
                    h2 { "Fundraiser" }
                    p { "Together we have raised ${funds}"}
                }
                section {
                    h2 { "Participating Streamers" }
                    section {
                        h3 { "Twitch" }
                        pre { "{cx.props.twitch.updated}" }
                        div {
                            {
                                cx.props.twitch.creators.iter().map(|creator| {
                                    cx.render(rsx! {
                                        creator_card { creator: creator, }
                                    })
                                })
                            }
                        }
                    }
                    section {
                        h3 { "Youtube" }
                        pre { "{cx.props.youtube.updated}" }
                        div {
                            {
                                cx.props.youtube.creators.iter().map(|creator| {
                                    cx.render(rsx! {
                                        creator_card { creator: creator, }
                                    })
                                })
                            }
                        }
                    }
                }
            }
        })
    }
}

#[axum::debug_handler]
#[tracing::instrument(skip_all)]
async fn fundraiser(
    State((tiltify_api_key, campaign, http_client)): State<(String, Campaign, reqwest::Client)>,
) -> Result<Bytes, StatusCode> {
    let request = http_client
        .get(format!(
            "https://tiltify.com/api/v3/campaigns/{}",
            campaign.id
        ))
        .bearer_auth(tiltify_api_key)
        .build()
        .expect("tiltify request should be well formed");

    let response = http_client
        .execute(request)
        .await
        .expect("tiltify api request failed");

    response.bytes().await.map_err(|err| {
        error!(%err, "failed to read tiltify body");

        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[axum::debug_handler]
#[allow(clippy::type_complexity)]
async fn streams(State(creators): State<CreatorsWatcher>) -> Json<Value> {
    Json(json!({
        "youtube": &*creators.youtube().borrow(),
        "twitch": &*creators.twitch().borrow(),
    }))
}
