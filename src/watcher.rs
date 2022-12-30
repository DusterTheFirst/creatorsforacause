use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::{sync::watch, time::Instant};
use tracing::trace;

use crate::{
    config::Config,
    model::{Campaign, Creator},
};

use self::{
    twitch::{TwitchEnvironment, TwitchLiveWatcher},
    youtube::YoutubeEnvironment,
};

pub mod tiltify;
pub mod twitch;
pub mod youtube;

#[derive(Debug, Deserialize)]
pub struct WatcherEnvironment {
    #[serde(flatten)]
    twitch: TwitchEnvironment,

    #[serde(flatten)]
    youtube: YoutubeEnvironment,

    /// API key for tiltify
    tiltify_api_key: String,
}

pub type WatcherDataReceive = Option<Arc<WatcherData>>;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WatcherData {
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub twitch: Box<[Creator]>,
    pub youtube: Box<[Creator]>,
    pub tiltify: Campaign,
}

pub async fn live_watcher(
    http_client: reqwest::Client,
    environment: WatcherEnvironment,
    config: &Config,
    sender: watch::Sender<WatcherDataReceive>,
) {
    let mut twitch_live_watcher = TwitchLiveWatcher::setup(
        http_client.clone(),
        environment.twitch,
        config.creators.twitch,
    )
    .await;

    let mut next_refresh = Instant::now();
    let refresh_interval = Duration::from_secs(10 * 60); // 10 minutes

    loop {
        let (youtube, twitch, tiltify) = tokio::join!(
            youtube::get_creators(&http_client, config.creators.youtube, &environment.youtube),
            twitch_live_watcher.get_creators(),
            tiltify::get_campaign(&http_client, config.campaign, &environment.tiltify_api_key),
        );

        let twitch = twitch.expect("TODO: REPLACE WITH ERROR HANDLING");
        let tiltify = tiltify.expect("TODO: REPLACE WITH ERROR HANDLING");

        sender.send_replace(Some(Arc::new(WatcherData {
            updated: OffsetDateTime::now_utc(),
            twitch,
            youtube,
            tiltify,
        })));

        // Refresh every 10 minutes
        next_refresh += refresh_interval;
        trace!(?refresh_interval, "Waiting for next refresh");

        tokio::time::sleep_until(next_refresh).await;
    }
}
