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
    use std::{cell::RefCell, net::SocketAddr};

    use askama::Template;
    use axum::{extract::WebSocketUpgrade, routing::get, Router};
    use dioxus::prelude::*;
    use dioxus_ssr::Renderer;

    use crate::model::CreatorsWatcher;

    #[derive(Debug, Template)]
    #[template(path = "dashboard.html")]
    struct Dashboard {
        glue: String,
        ssr: String,
    }

    pub(super) fn router(listen: SocketAddr, creators: CreatorsWatcher) -> Router {
        let view = dioxus_liveview::LiveViewPool::new();

        Router::new()
            .route(
                "/",
                get({
                    let creators = creators.clone();
                    move || async move {
                        fn renderer() -> Renderer {
                            let mut renderer = Renderer::default();

                            renderer.pretty = false;
                            renderer.newline = false;
                            renderer.sanitize = true;
                            renderer.pre_render = true;
                            renderer.skip_components = false;

                            renderer
                        }

                        thread_local! {
                            static RENDERER: RefCell<Renderer> = RefCell::new(renderer());
                        }

                        let mut vdom = VirtualDom::new_with_props(dashboard, creators);

                        let _ = vdom.rebuild();

                        Dashboard {
                            glue: dioxus_liveview::interpreter_glue(&format!("ws://{listen}/ws")),
                            ssr: RENDERER.with(|renderer| {
                                renderer
                                    .borrow_mut()
                                    .render(&vdom)
                            }),
                        }
                    }
                }),
            )
            .route(
                "/ws",
                get(move |ws: WebSocketUpgrade| async move {
                    ws.on_upgrade(move |socket| async move {
                        _ = view
                            .launch_with_props(
                                dioxus_liveview::axum_socket(socket),
                                dashboard,
                                creators,
                            )
                            .await;
                    })
                }),
            )
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn dashboard(cx: Scope<CreatorsWatcher>) -> Element {
        cx.render(rsx! {
            div {
                "hello axum!"
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
        "youtube": &*creators.youtube(),
        "twitch": &*creators.twitch(),
    }))
}
