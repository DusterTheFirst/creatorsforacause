use std::{cmp, fmt::Debug};

use reqwest::Url;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Serialize)]
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
    pub viewers: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "camel_case")]
pub struct Campaign {
    id: u32,
    name: String,
    slug: String,
    url: String,
    #[serde(with = "time::serde::timestamp")]
    starts_at: OffsetDateTime,
    #[serde(with = "time::serde::timestamp")]
    ends_at: OffsetDateTime,
    description: String,
    avatar: TiltifyAvatar,
    cause_id: u32,

    fundraising_event_id: u32,
    fundraiser_goal_amount: u32,
    original_goal_amount: u32,
    // Using floats since no math or large numbers are used, so precision is not a problem
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
    username: String,
    slug: String,
    url: String,
    avatar: TiltifyAvatar,
}
