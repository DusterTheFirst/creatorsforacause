use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use axum::{
    body::{Body, HttpBody},
    extract::State,
    http::Request,
    response::{IntoResponse, Response},
};
use color_eyre::eyre::Context;
use hyper::{body, StatusCode};
use reqwest::Url;
use serde::Deserialize;
use tokio::task::{JoinSet, LocalSet};
use tracing::{error, info, trace, trace_span, warn, Instrument};
use twitch_api::{
    eventsub::{
        self, stream::StreamOnlineV1, Event, Message, Payload, Status, Transport,
        VerificationRequest,
    },
    helix::{
        channels::ChannelInformation,
        eventsub::{
            CreateEventSubSubscriptionBody, CreateEventSubSubscriptionRequest,
            DeleteEventSubSubscriptionRequest, GetEventSubSubscriptionsRequest,
        },
    },
    twitch_oauth2::{AppAccessToken, ClientId, ClientSecret, TwitchToken},
    types::{Nickname, UserName},
};

#[derive(Deserialize, Debug)]
pub struct TwitchEnvironment {
    #[serde(rename = "twitch_client_id")]
    client_id: ClientId,
    #[serde(rename = "twitch_client_secret")]
    client_secret: ClientSecret,
}

pub async fn twitch_live_watcher(
    http_client: reqwest::Client,
    environment: TwitchEnvironment,
    domain: Url,
    eventsub_secret: Arc<str>,
    creators_names: HashSet<UserName>,
) -> color_eyre::Result<()> {
    info!(
        ?creators_names,
        "Starting live status watch of twitch creators"
    );

    let client = twitch_api::HelixClient::with_client(http_client);
    let token = Rc::new(
        AppAccessToken::get_app_access_token(
            &client,
            environment.client_id,
            environment.client_secret,
            vec![],
        )
        .await
        .wrap_err("attempted to fetch app access token")?,
    );

    info!(expires_in = ?token.expires_in(), "Acquired access token");

    // See what we are already subscribed to
    let subscriptions = client
        .req_get(GetEventSubSubscriptionsRequest::default(), token.as_ref())
        .await
        .expect("piss");
    let subscriptions = subscriptions.data.subscriptions;

    let bad_subscriptions = subscriptions
        .iter()
        .filter(|subscription| subscription.status == Status::WebhookCallbackVerificationFailed);

    let good_subscriptions = subscriptions
        .iter()
        .filter(|subscription| subscription.status != Status::WebhookCallbackVerificationFailed);

    let set = LocalSet::new();
    for subscription in bad_subscriptions {
        let client = client.clone();
        let token = token.clone();
        let id = subscription.id.clone();

        let span = trace_span!("delete_subscription", %id);

        set.spawn_local(
            async move {
                client
                    .req_delete(DeleteEventSubSubscriptionRequest::id(id), token.as_ref())
                    .await
                    .expect("TODO: piss");

                trace!("removed failed subscription");
            }
            .instrument(span),
        );
    }
    set.await;

    dbg!(subscriptions);

    let channel_info = get_channel_information(creators_names, client.clone(), token.clone()).await;

    for channel in channel_info.into_values() {
        let online_subscription = StreamOnlineV1::broadcaster_user_id(channel.broadcaster_id);
        // let offline_subscription = StreamOfflineV1::broadcaster_user_id(broadcaster_user_id);
        // let update_subscription = ChannelUpdateV1::broadcaster_user_id(broadcaster_user_id);

        let transport = Transport::webhook(
            domain
                .join("twitch/eventsub")
                .expect("webhook url should be valid"),
            String::from(eventsub_secret.as_ref()),
        );

        client
            .req_post(
                CreateEventSubSubscriptionRequest::new(),
                CreateEventSubSubscriptionBody::new(online_subscription, transport),
                token.as_ref(),
            )
            .await
            .expect("TODO:");
    }

    // let live_streams = client
    //     .req_get(
    //         twitch_api::helix::streams::GetStreamsRequest::user_logins(
    //             creators_names
    //                 .iter()
    //                 .map(|name| name.as_ref())
    //                 .collect::<Vec<_>>(),
    //         ),
    //         token.as_ref(),
    //     )
    //     .await
    //     .wrap_err("failed to fetch live streams")?;

    // dbg!(&live_streams);

    Ok(())
}

async fn get_channel_information(
    creators_names: HashSet<Nickname>,
    client: twitch_api::HelixClient<'static, reqwest::Client>,
    token: Rc<AppAccessToken>,
) -> HashMap<Nickname, ChannelInformation> {
    let mut creators_futures = JoinSet::new();
    for nickname in creators_names {
        creators_futures.spawn_local({
            let client = client.clone();
            let token = token.clone();

            async move {
                (
                    client
                        .get_channel_from_login(&nickname, token.as_ref())
                        .await,
                    nickname,
                )
            }
        });
    }

    let mut creators = HashMap::with_capacity(creators_futures.len());
    while let Some(join_result) = creators_futures.join_next().await {
        match join_result {
            Ok((Err(error), nickname)) => {
                error!(?nickname, ?error, "failed to fetch channel from login")
            }
            Ok((Ok(None), nickname)) => warn!(?nickname, "creator not found"),
            Ok((Ok(Some(channel)), nickname)) => {
                creators.insert(nickname, channel);
            }
            Err(error) => error!(%error, "failed to drive creator future to completion"),
        }
    }

    creators
}

pub async fn handle_eventsub(
    State(eventsub_secret): State<Arc<str>>,
    request: Request<Body>,
) -> Response {
    const MAX_ALLOWED_REQUEST_SIZE: u64 = 2u64.pow(20); // 1 Megabyte

    // Protect against super large http bodies
    match request.body().size_hint().upper() {
        Some(size) if size < MAX_ALLOWED_REQUEST_SIZE => {}
        Some(size) => {
            warn!(size, "rejected eventsub request with body length too large");
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "content-length exceeds maximum of 1 Megabyte",
            )
                .into_response();
        }
        None => {
            warn!("rejected eventsub request with no body length given");
            return (StatusCode::BAD_REQUEST, "content-length not provided").into_response();
        }
    }

    // Convert Body to Vec<u8> the hard way
    let request = {
        let mut builder = Request::builder()
            .uri(request.uri())
            .method(request.method());

        builder
            .headers_mut()
            .expect("request should be a valid request")
            .extend(request.headers().clone());

        builder
            .body(body::to_bytes(request.into_body()).await.expect("TODO:"))
            .expect("request should be a valid request")
    };

    // Verify that twitch did indeed send this request
    if !Event::verify_payload(&request, eventsub_secret.as_bytes()) {
        warn!("rejected eventsub request with bad hmac");

        return (
            StatusCode::FORBIDDEN,
            "hmac check failed on provided information",
        )
            .into_response();
    }

    match Event::parse_http(&request) {
        Ok(Event::StreamOnlineV1(Payload {
            message: Message::VerificationRequest(VerificationRequest { challenge, .. }),
            subscription,
            ..
        })) => {
            trace!(?subscription, "verified subscription to new event");

            (StatusCode::OK, challenge).into_response()
        }
        Ok(event) => {
            warn!(?event, "received unexpected event");

            StatusCode::NOT_IMPLEMENTED.into_response()
        }
        Err(error) => {
            error!(%error, "encountered error parsing event from http request");

            (
                StatusCode::BAD_REQUEST,
                "unable to parse event data from request body",
            )
                .into_response()
        }
    }
}
