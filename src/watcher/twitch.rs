use std::collections::HashMap;

use serde::Deserialize;
use time::{format_description::well_known, OffsetDateTime};
use tracing::{error, info, trace};
use twitch_api::{
    helix::{
        streams::GetStreamsRequest,
        users::{GetUsersRequest, User},
    },
    twitch_oauth2::{AppAccessToken, ClientId, ClientSecret, TwitchToken},
    types::{Nickname, NicknameRef},
};

use crate::model::{Creator, LiveStreamDetails, StreamingService};

#[derive(Deserialize, Debug)]
pub struct TwitchEnvironment {
    #[serde(rename = "twitch_client_id")]
    client_id: ClientId,
    #[serde(rename = "twitch_client_secret")]
    client_secret: ClientSecret,
}

pub struct TwitchLiveWatcher {
    helix_client: twitch_api::HelixClient<'static, reqwest::Client>,
    token: AppAccessToken,
    creators_names: &'static [&'static NicknameRef],
}

impl TwitchLiveWatcher {
    #[tracing::instrument(skip_all)]
    pub async fn setup(
        http_client: reqwest::Client,
        environment: TwitchEnvironment,
        creators_names: &'static [&'static NicknameRef],
    ) -> Self {
        let helix_client = twitch_api::HelixClient::with_client(http_client);

        let token = AppAccessToken::get_app_access_token(
            &helix_client,
            environment.client_id,
            environment.client_secret,
            vec![],
        )
        .await
        .expect("access token should be fetched successfully");

        let expires_at = OffsetDateTime::now_utc() + token.expires_in();

        info!(?expires_at, "acquired access token");

        TwitchLiveWatcher {
            helix_client,
            token,
            creators_names,
        }
    }

    #[tracing::instrument(skip(self), fields(creators_names = ?self.creators_names))]
    pub async fn get_creators(&mut self) -> Option<Vec<Creator>> {
        let client = &self.helix_client;
        let creators_names = self.creators_names;
        let token: &mut AppAccessToken = &mut self.token;

        if token.is_elapsed() {
            match token.refresh_token(client).await {
                Ok(()) => trace!(expires_in = ?token.expires_in(), "refreshed access token"),
                Err(error) => {
                    error!(%error, "failed to refresh twitch access token");
                }
            };
        }

        let (users, streams) = tokio::join!(
            get_user_info(client, creators_names, token),
            get_live_statuses(client, creators_names, token)
        );

        let (users, streams) = users.zip(streams)?;

        Some(
            users
                .into_iter()
                .map(|user| {
                    Creator {
                        service: StreamingService::Twitch,
                        id: user.id.take(),
                        display_name: user.display_name.take(),
                        href: format!("https://twitch.tv/{}", user.login),
                        stream: streams.get(&user.login).cloned(),
                        handle: user.login.take(),
                        icon_url: user
                            .profile_image_url
                            // TODO: replace with placeholder?
                            .expect("twitch streamer should have a profile image url"),
                    }
                })
                .collect(),
        )
    }
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
                viewers: Some(
                    stream
                        .viewer_count
                        .try_into()
                        .expect("viewer_count should be no larger than a 32 bit integer"),
                ),
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
