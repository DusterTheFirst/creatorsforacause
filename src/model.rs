use std::{collections::HashMap, fmt::Debug, hash::Hash};

use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::watch;
use twitch_api::types::Nickname;

use crate::youtube::YoutubeHandle;

#[derive(Clone)]
pub struct Creators {
    twitch: watch::Receiver<CreatorsList<TwitchSource>>,
    youtube: watch::Receiver<CreatorsList<YoutubeSource>>,
}

impl Debug for Creators {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Creators")
            .field("twitch", &self.twitch.borrow())
            .field("youtube", &self.youtube.borrow())
            .finish()
    }
}

impl Creators {
    pub fn new() -> (
        Self,
        (
            watch::Sender<CreatorsList<TwitchSource>>,
            watch::Sender<CreatorsList<YoutubeSource>>,
        ),
    ) {
        // We have to use "Sync" channels over Rc<RefCell<_>> since axum requires all state be sync
        // even though we are guaranteed to be on the same thread (single threaded async runtime)
        let (youtube_writer, youtube_reader) = watch::channel(CreatorsList {
            updated: OffsetDateTime::UNIX_EPOCH,
            creators: HashMap::new(),
        });

        let (twitch_writer, twitch_reader) = watch::channel(CreatorsList {
            updated: OffsetDateTime::UNIX_EPOCH,
            creators: HashMap::new(),
        });

        (
            Self {
                twitch: twitch_reader,
                youtube: youtube_reader,
            },
            (twitch_writer, youtube_writer),
        )
    }

    pub fn twitch(&self) -> watch::Ref<CreatorsList<TwitchSource>> {
        self.twitch.borrow()
    }

    pub fn youtube(&self) -> watch::Ref<CreatorsList<YoutubeSource>> {
        self.youtube.borrow()
    }
}

#[axum::async_trait]
pub trait CreatorSource {
    type Identifier: Serialize + Hash + Eq;
}

#[derive(Debug)]
pub struct YoutubeSource;

impl CreatorSource for YoutubeSource {
    type Identifier = YoutubeHandle;
}

#[derive(Debug)]
pub struct TwitchSource;

impl CreatorSource for TwitchSource {
    type Identifier = Nickname;
}

#[derive(Debug, Serialize)]
pub struct CreatorsList<Source: CreatorSource> {
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub creators: HashMap<Source::Identifier, Creator>,
}

#[derive(Debug, Serialize)]
pub struct Creator {
    pub display_name: String,
    pub href: String,
    pub icon_url: String,
    pub stream: Option<LiveStreamDetails>,
}

#[derive(Debug, Serialize)]
pub struct LiveStreamDetails {
    pub href: String,
    pub title: String,
    pub start_time: OffsetDateTime,
    pub viewers: u32,
}
