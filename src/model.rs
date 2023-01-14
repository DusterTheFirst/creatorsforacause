use std::cmp;

use prometheus_client::encoding::EncodeLabelValue;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub mod javascript_unix_timestamp;

#[derive(Debug, Serialize)]
pub struct Creator {
    /// The internal, unchanging ID used by the respective service
    pub id: String,
    pub display_name: String,
    pub handle: String,
    pub href: String,
    pub icon_url: String,
    pub stream: Option<LiveStreamDetails>,
    pub service: StreamingService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EncodeLabelValue, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamingService {
    Twitch,
    Youtube,
}

impl Eq for Creator {}
impl PartialEq for Creator {
    fn eq(&self, other: &Self) -> bool {
        self.display_name == other.display_name && self.stream.is_some() == other.stream.is_some()
    }
}

impl Ord for Creator {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
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
    #[serde(with = "time::serde::rfc3339")]
    pub start_time: OffsetDateTime,
    pub viewers: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Campaign {
    pub id: u32,
    pub name: String,
    pub slug: String,
    #[serde(deserialize_with = "javascript_unix_timestamp::deserialize")]
    #[serde(serialize_with = "time::serde::rfc3339::serialize")]
    pub starts_at: OffsetDateTime,
    #[serde(deserialize_with = "javascript_unix_timestamp::option::deserialize")]
    #[serde(serialize_with = "time::serde::rfc3339::option::serialize")]
    pub ends_at: Option<OffsetDateTime>,
    pub description: String,
    pub avatar: TiltifyAvatar,
    pub cause_id: u32,

    // Using floats since no math or large numbers are used, so precision is not a problem
    pub fundraiser_goal_amount: f64,
    pub original_fundraiser_goal: f64,
    pub amount_raised: f64,
    pub supporting_amount_raised: f64,
    pub total_amount_raised: f64,

    pub supportable: bool,

    pub user: TiltifyUser,
    pub team: TiltifyTeam,
}

impl Eq for Campaign {}
impl PartialEq for Campaign {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "camel_case")]
pub struct TiltifyAvatar {
    pub src: Url,
    pub alt: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "camel_case")]
pub struct TiltifyUser {
    pub id: u32,
    pub username: String,
    pub slug: String,
    pub url: String,
    pub avatar: TiltifyAvatar,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "camel_case")]
pub struct TiltifyTeam {
    pub id: u32,
    pub name: String,
    pub slug: String,
    pub url: String,
    pub avatar: TiltifyAvatar,
}
