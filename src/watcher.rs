use std::sync::Arc;

use color_eyre::eyre::Context;
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::watch;
use tracing::{error, trace};

use crate::{
    config::Config,
    metrics::types::{LiveCreatorsMetric, StreamingServiceMetricKey, YoutubeQuotaUsageMetric},
    model::{Campaign, Creator},
};

use self::{
    tiltify::TiltifyWatcher,
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
    pub creators: Box<[Creator]>,
    pub tiltify: Arc<Campaign>,
}

pub async fn live_watcher(
    http_client: reqwest::Client,
    environment: WatcherEnvironment,
    config: &Config,
    sender: watch::Sender<WatcherDataReceive>,
    live_creators: LiveCreatorsMetric,
    youtube_quota_usage: YoutubeQuotaUsageMetric,
) {
    let mut tiltify_watcher = TiltifyWatcher::new(
        http_client.clone(),
        config.campaign,
        environment.tiltify_api_key,
    );

    let mut twitch_live_watcher = TwitchLiveWatcher::setup(
        http_client.clone(),
        environment.twitch,
        config.creators.twitch,
    )
    .await;

    let mut interval = tokio::time::interval(config.refresh_period);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        let result = tokio::try_join!(
            youtube::get_creators(
                &http_client,
                config.creators.youtube,
                &environment.youtube,
                &youtube_quota_usage
            )
            .map(|youtube| youtube.wrap_err("failed to update youtube creators")),
            twitch_live_watcher
                .get_creators()
                .map(|twitch| twitch.wrap_err("failed to update twitch creators")),
            tiltify_watcher
                .get_campaign()
                .map(|tiltify| tiltify.wrap_err("failed to update tiltify data")),
        );

        let (youtube, twitch, tiltify) = match result {
            Ok(success) => success,
            Err(error) => {
                error!(%error);
                continue;
            }
        };

        let mut creators = twitch
            .into_iter()
            .chain(youtube.into_iter())
            .collect::<Box<[Creator]>>();

        creators.sort();

        for creator in creators.iter() {
            live_creators
                .get_or_create(&StreamingServiceMetricKey {
                    service: creator.service,
                    username: creator.handle.clone(),
                    id: creator.id.clone(),
                })
                .set(creator.stream.is_some().into());
        }

        // TODO: unmerge creators and tiltify?
        sender.send_replace(Some(Arc::new(WatcherData {
            updated: OffsetDateTime::now_utc(),
            creators,
            tiltify,
        })));

        trace!(?config.refresh_period, "waiting for next refresh");
    }
}
