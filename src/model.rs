use std::{cmp, fmt::Debug};

use reqwest::Url;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

mod javascript_unix_timestamp;

#[derive(Debug, Serialize)]
pub struct Creator {
    pub id: String,
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
    id: u32,
    name: String,
    slug: String,
    #[serde(deserialize_with = "javascript_unix_timestamp::deserialize")]
    #[serde(serialize_with = "time::serde::rfc3339::serialize")]
    starts_at: OffsetDateTime,
    #[serde(deserialize_with = "javascript_unix_timestamp::option::deserialize")]
    #[serde(serialize_with = "time::serde::rfc3339::option::serialize")]
    ends_at: Option<OffsetDateTime>,
    description: String,
    avatar: TiltifyAvatar,
    cause_id: u32,

    // Using floats since no math or large numbers are used, so precision is not a problem
    fundraiser_goal_amount: f64,
    original_fundraiser_goal: f64,
    amount_raised: f64,
    supporting_amount_raised: f64,
    total_amount_raised: f64,

    supportable: bool,

    user: TiltifyUser,
    team: TiltifyTeam,
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
    src: Url,
    alt: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "camel_case")]
pub struct TiltifyUser {
    id: u32,
    username: String,
    slug: String,
    url: String,
    avatar: TiltifyAvatar,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "camel_case")]
pub struct TiltifyTeam {
    id: u32,
    name: String,
    slug: String,
    url: String,
    avatar: TiltifyAvatar,
}
