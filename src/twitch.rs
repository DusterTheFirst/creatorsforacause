use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use serde::Deserialize;
use time::OffsetDateTime;
use tokio::{
    sync::watch,
    time::{Duration, Instant},
};
use tracing::{error, info, trace, warn};
use twitch_api::{
    helix::streams::GetStreamsRequest,
    twitch_oauth2::{AppAccessToken, ClientId, ClientSecret, TwitchToken},
    types::{Nickname, UserName},
};

use crate::model::{CreatorsList, LiveStreamDetails, TwitchSource};

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
    status_sender: watch::Sender<CreatorsList<TwitchSource>>,
) {
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
        .expect("access token should be fetched successfully"),
    );

    info!(expires_in = ?token.expires_in(), "Acquired access token");

    let mut next_refresh = Instant::now();
    let refresh_interval = Duration::from_secs(60 * 10);

    loop {
        if let Some(creators) = get_live_statuses(&client, &creators_names, &token).await {
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

        tokio::time::sleep_until(next_refresh).await;
    }
}

async fn get_creators() {}

#[tracing::instrument(skip(client, creators_names, token))]
async fn get_live_statuses(
    client: &twitch_api::HelixClient<'static, reqwest::Client>,
    creators_names: &HashSet<Nickname>,
    token: &Rc<AppAccessToken>,
) -> Option<HashMap<Nickname, Option<LiveStreamDetails>>> {
    let live_streams = client
        .req_get(
            GetStreamsRequest::user_logins(
                creators_names
                    .iter()
                    .map(|name| name.as_ref())
                    .collect::<Vec<_>>(),
            ),
            token.as_ref(),
        )
        .await;

    let live_streams = match live_streams {
        Ok(live_streams) => live_streams,
        Err(error) => {
            error!(%error, "failed to get livestreams");

            return None;
        }
    };

    let mut all_streams: HashMap<Nickname, Option<LiveStreamDetails>> = creators_names
        .iter()
        .cloned()
        .map(|handle| (handle, None))
        .collect();

    // Read through pagination
    let mut live_streams = Some(live_streams);
    while let Some(previous) = live_streams {
        all_streams.extend(previous.data.iter().cloned().map(|stream| {
            let livestream_details = LiveStreamDetails {
                href: format!("https://twitch.tv/{}", stream.user_login),
                title: stream.title,
                start_time: stream.started_at.take(),
                viewers: stream
                    .viewer_count
                    .try_into()
                    .expect("viewer_count should be no larger than a 32 bit integer"),
            };

            (stream.user_login, Some(livestream_details))
        }));

        live_streams = match previous.get_next(client, token.as_ref()).await {
            Ok(live_streams) => live_streams,
            Err(error) => {
                error!(%error, "failed to get next pagination result");

                return Some(all_streams);
            }
        }
    }

    Some(all_streams)
}
