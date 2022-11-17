use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use color_eyre::eyre::Context;
use serde::Deserialize;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use twitch_api::{
    twitch_oauth2::{AppAccessToken, ClientId, ClientSecret, TwitchToken},
    types::UserName,
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

    let mut creators_futures = JoinSet::new();
    let mut creators = HashMap::with_capacity(creators_names.len());

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

    while let Some(join_result) = creators_futures.join_next().await {
        let (channel_result, nickname) = join_result.expect("failed to join future");

        match channel_result {
            Err(error) => error!(?nickname, ?error, "failed to fetch channel from login"),
            Ok(None) => warn!(?nickname, "creator not found"),
            Ok(Some(channel)) => {
                creators.insert(nickname, channel);
            }
        }
    }

    dbg!(creators);

    // client.req_get(twitch_api::helix::streams::GetStreamsRequest::user_ids(&[]), token)

    Ok(())
}
