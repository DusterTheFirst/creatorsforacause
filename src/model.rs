use std::{cmp, fmt::Debug};

use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::watch;

#[derive(Clone)]
pub struct Creators {
    twitch: watch::Receiver<CreatorsList>,
    youtube: watch::Receiver<CreatorsList>,
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
        watch::Sender<CreatorsList>,
        watch::Sender<CreatorsList>,
    ) {
        // We have to use "Sync" channels over Rc<RefCell<_>> since axum requires all state be sync
        // even though we are guaranteed to be on the same thread (single threaded async runtime)
        let (youtube_writer, youtube_reader) = watch::channel(CreatorsList {
            updated: OffsetDateTime::UNIX_EPOCH,
            creators: Box::new([]),
        });

        let (twitch_writer, twitch_reader) = watch::channel(CreatorsList {
            updated: OffsetDateTime::UNIX_EPOCH,
            creators: Box::new([]),
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

    pub fn twitch(&self) -> watch::Ref<CreatorsList> {
        self.twitch.borrow()
    }

    pub fn youtube(&self) -> watch::Ref<CreatorsList> {
        self.youtube.borrow()
    }
}

#[derive(Debug, Serialize)]
pub struct CreatorsList {
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub creators: Box<[Creator]>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Creator {
    pub display_name: String,
    pub href: String,
    pub icon_url: String,
    pub stream: Option<LiveStreamDetails>,
}

impl Eq for Creator {}
impl PartialEq for Creator {
    fn eq(&self, other: &Self) -> bool {
        self.display_name == other.display_name && self.stream.is_some() == other.stream.is_some()
    }
}

impl Ord for Creator {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        // dbg!(
        //     (&self.display_name, &other.display_name),
        //     (self.stream.is_some(), other.stream.is_some())
        // );

        match (self.stream.is_some(), other.stream.is_some()) {
            (true, false) => cmp::Ordering::Less,
            (false, true) => cmp::Ordering::Greater,
            (true, true) | (false, false) => self.display_name.cmp(&other.display_name),
        }
    }
}
impl PartialOrd for Creator {
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
