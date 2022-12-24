use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use time::{format_description::well_known, OffsetDateTime};
use tokio::{
    sync::watch,
    time::{Duration, Instant},
};
use tracing::{error, info, trace, warn};
use twitch_api::{
    helix::{
        streams::GetStreamsRequest,
        users::{GetUsersRequest, User},
    },
    twitch_oauth2::{AppAccessToken, ClientId, ClientSecret, TwitchToken},
    types::{Nickname, NicknameRef, UserName},
};

use crate::model::{Creator, CreatorsList, LiveStreamDetails};

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
    creators_names: HashSet<UserName>,
    status_sender: watch::Sender<CreatorsList>,
) {
    let creators_names = creators_names
        .iter()
        .map(|nickname| nickname.as_ref())
        .collect::<Box<_>>();

    info!(
        ?creators_names,
        "Starting live status watch of twitch creators"
    );

    let client = twitch_api::HelixClient::with_client(http_client);
    let mut token = AppAccessToken::get_app_access_token(
        &client,
        environment.client_id,
        environment.client_secret,
        vec![],
    )
    .await
    .expect("access token should be fetched successfully");

    info!(expires_in = ?token.expires_in(), "acquired access token");

    let mut next_refresh = Instant::now();
    let refresh_interval = Duration::from_secs(10 * 60); // 10 minutes

    loop {
        tokio::time::sleep_until(next_refresh).await;

        if token.is_elapsed() {
            match token.refresh_token(&client).await {
                Ok(()) => trace!(expires_in = ?token.expires_in(), "refreshed access token"),
                Err(error) => {
                    error!(%error, "failed to refresh twitch access token");
                }
            };
        }

        if let Some(creators) = get_creators(&client, &creators_names, &token).await {
            status_sender.send_replace(CreatorsList {
                updated: OffsetDateTime::now_utc(),
                creators,
            });
        } else {
            warn!("no update to the live streams");
        }

        // Refresh every 10 minutes
        next_refresh += refresh_interval;
        trace!(?refresh_interval, "Waiting for next refresh");
    }
}

async fn get_creators(
    client: &twitch_api::HelixClient<'static, reqwest::Client>,
    creators_names: &[&NicknameRef],
    token: &AppAccessToken,
) -> Option<Box<[Creator]>> {
    let (users, streams) = tokio::join!(
        get_user_info(client, creators_names, token),
        get_live_statuses(client, creators_names, token)
    );

    let (users, streams) = users.zip(streams)?;

    let mut creators = users
        .into_iter()
        .map(|user| {
            Creator {
                display_name: user.display_name.take(),
                href: format!("https://twitch.tv/{}", user.login),
                icon_url: user
                    .profile_image_url
                    // TODO: replace with placeholder?
                    .expect("twitch streamer should have a profile image url"),
                stream: streams.get(&user.login).cloned(),
            }
        })
        .collect::<Box<_>>();

    // Return the array sorted
    creators.sort_unstable();

    Some(creators)
}

async fn get_user_info(
    client: &twitch_api::HelixClient<'static, reqwest::Client>,
    creators_names: &[&NicknameRef],
    token: &AppAccessToken,
) -> Option<Vec<User>> {
    // TODO: split if more than 100 users, lol

    let creators = client
        .req_get(GetUsersRequest::logins(creators_names), token)
        .await;

    let creators = match creators {
        Ok(creators) => creators,
        Err(error) => {
            error!(%error, "failed to get livestreams");

            return None;
        }
    };

    Some(creators.data)
}

#[tracing::instrument(skip(client, creators_names, token))]
async fn get_live_statuses(
    client: &twitch_api::HelixClient<'static, reqwest::Client>,
    creators_names: &[&NicknameRef],
    token: &AppAccessToken,
) -> Option<HashMap<Nickname, LiveStreamDetails>> {
    let live_streams = client
        .req_get(
            GetStreamsRequest::user_logins(creators_names).first(100),
            token,
        )
        .await;

    let live_streams = match live_streams {
        Ok(live_streams) => live_streams,
        Err(error) => {
            error!(%error, "failed to get livestreams");

            return None;
        }
    };

    let mut all_streams = HashMap::with_capacity(creators_names.len());

    // Read through pagination
    let mut live_streams = Some(live_streams);
    while let Some(previous) = live_streams {
        all_streams.extend(previous.data.iter().cloned().map(|stream| {
            let livestream_details = LiveStreamDetails {
                href: format!("https://twitch.tv/{}", stream.user_login),
                title: stream.title,
                start_time: OffsetDateTime::parse(stream.started_at.as_str(), &well_known::Rfc3339)
                    .expect("stream start time should be a well formed rfc3339 date-time"),
                viewers: stream
                    .viewer_count
                    .try_into()
                    .expect("viewer_count should be no larger than a 32 bit integer"),
            };

            (stream.user_login, livestream_details)
        }));

        live_streams = match previous.get_next(client, token).await {
            Ok(live_streams) => live_streams,
            Err(error) => {
                error!(%error, "failed to get next pagination result");

                return Some(all_streams);
            }
        }
    }

    Some(all_streams)
}
