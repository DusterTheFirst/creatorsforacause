use std::{cmp, collections::BTreeSet, fmt::Debug, hash::Hash};

use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::watch;
use twitch_api::types::Nickname;

use crate::youtube::api::YoutubeHandle;

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
        watch::Sender<CreatorsList<TwitchSource>>,
        watch::Sender<CreatorsList<YoutubeSource>>,
    ) {
        // We have to use "Sync" channels over Rc<RefCell<_>> since axum requires all state be sync
        // even though we are guaranteed to be on the same thread (single threaded async runtime)
        let (youtube_writer, youtube_reader) = watch::channel(CreatorsList {
            updated: OffsetDateTime::UNIX_EPOCH,
            creators: BTreeSet::new(),
        });

        let (twitch_writer, twitch_reader) = watch::channel(CreatorsList {
            updated: OffsetDateTime::UNIX_EPOCH,
            creators: BTreeSet::new(),
        });

        (
            Self {
                twitch: twitch_reader,
                youtube: youtube_reader,
            },
            twitch_writer,
            youtube_writer,
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
pub trait CreatorSource: Serialize + Debug {
    type Identifier: Serialize + Hash + Eq + Debug;
}

#[derive(Debug, Serialize)]
pub struct YoutubeSource;

impl CreatorSource for YoutubeSource {
    type Identifier = YoutubeHandle;
}

#[derive(Debug, Serialize)]
pub struct TwitchSource;

impl CreatorSource for TwitchSource {
    type Identifier = Nickname;
}

#[derive(Debug, Serialize)]
pub struct CreatorsList<Source: CreatorSource> {
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub creators: BTreeSet<Creator<Source>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Creator<Source: CreatorSource> {
    pub internal_identifier: Source::Identifier,
    pub display_name: String,
    pub href: String,
    pub icon_url: String,
    pub stream: Option<LiveStreamDetails>,
}

impl<Source: CreatorSource> Eq for Creator<Source> {}
impl<Source: CreatorSource> PartialEq for Creator<Source> {
    fn eq(&self, other: &Self) -> bool {
        self.display_name == other.display_name && self.stream.is_some() == other.stream.is_some()
    }
}

impl<Source: CreatorSource + Debug> Ord for Creator<Source> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        dbg!(
            (&self.display_name, &other.display_name),
            (self.stream.is_some(), other.stream.is_some())
        );

        match (self.stream.is_some(), other.stream.is_some()) {
            (true, false) => dbg!(cmp::Ordering::Less),
            (false, true) => dbg!(cmp::Ordering::Greater),
            (true, true) | (false, false) => self.display_name.cmp(&other.display_name),
        }
    }
}
impl<Source: CreatorSource + Debug> PartialOrd for Creator<Source> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct LiveStreamDetails {
    pub href: String,
    pub title: String,
    pub start_time: OffsetDateTime,
    pub viewers: u32,
}
