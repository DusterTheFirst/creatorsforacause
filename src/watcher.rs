use std::sync::Arc;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::watch;

use crate::{
    config::Config,
    model::{Campaign, Creator},
};

use self::{twitch::TwitchEnvironment, youtube::YoutubeEnvironment};

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
    reqwest_client: reqwest::Client,
    environment: WatcherEnvironment,
    config: &Config,
    sender: watch::Sender<WatcherDataReceive>,
) {
}
