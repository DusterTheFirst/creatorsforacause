use std::fmt::Debug;

use futures::stream::{FuturesUnordered, StreamExt};
use serde::Deserialize;
use time::{format_description::well_known, OffsetDateTime};
use tokio::pin;
use tracing::{debug, error, info, warn, Instrument};

use crate::{
    metrics::types::YoutubeQuotaUsageMetric,
    model::{Creator, LiveStreamDetails},
};

use self::{
    api::{get_creator_info, get_video_info, ApiKey, ApiKeyRef, CreatorInfo, YoutubeHandleRef},
    scraping::{get_channel_id, get_livestream_video_id},
};

pub mod api;
mod scraping;

#[derive(Deserialize, Debug)]
pub struct YoutubeEnvironment {
    #[serde(rename = "youtube_api_key")]
    api_key: ApiKey,
}

#[tracing::instrument(skip_all)]
pub async fn get_creators(
    http_client: &reqwest::Client,
    creator_names: &[&YoutubeHandleRef],
    environment: &YoutubeEnvironment,
    youtube_quota_usage: &YoutubeQuotaUsageMetric,
) -> Box<[Creator]> {
    pin! {
        let futures = FuturesUnordered::new();
    };

    for creator_name in creator_names.iter() {
        let span = tracing::trace_span!("creator_update", ?creator_name);

        futures.push(
            async move {
                tokio::join!(
                    // Cache this and or the user ID
                    get_creator_info_from_handle(
                        http_client,
                        &environment.api_key,
                        creator_name,
                        youtube_quota_usage
                    ),
                    get_livestream_details(
                        http_client,
                        &environment.api_key,
                        creator_name,
                        youtube_quota_usage
                    )
                )
            }
            .instrument(span),
        );
    }

    // Drive all futures to completion, collecting their results
    let mut live_broadcasts: Vec<Creator> = Vec::with_capacity(creator_names.len());

    while let Some(result) = futures.next().await {
        match result {
            (Some(creator_info), livestream_details) => {
                let display_name = creator_info
                    .snippet
                    .title
                    .expect("title field should be present in snippet");
                let icon_url = creator_info
                    .snippet
                    .thumbnails
                    .expect("thumbnails field should be present in snippet")
                    .default
                    .expect("default thumbnail should exist")
                    .url
                    .expect("default thumbnail url should exist");
                let custom_url = creator_info
                    .snippet
                    .custom_url
                    .expect("custom_url field should be present in snippet");

                live_broadcasts.push(Creator {
                    id: creator_info.id.take(),
                    display_name,
                    href: format!("https://youtube.com/{custom_url}"),
                    handle: custom_url,
                    icon_url,
                    stream: livestream_details,
                });
            }
            (None, _) => warn!("failed to get creator info"),
        }
    }

    live_broadcasts.sort_unstable();

    live_broadcasts.into_boxed_slice()
}

async fn get_creator_info_from_handle(
    http_client: &reqwest::Client,
    api_key: &ApiKeyRef,
    handle: &YoutubeHandleRef,
    youtube_quota_usage: &YoutubeQuotaUsageMetric,
) -> Option<CreatorInfo> {
    let channel_id = match get_channel_id(http_client, handle).await {
        Ok(channel_id) => channel_id,
        Err(error) => {
            error!(?error, "failed to get channel id");
            return None;
        }
    }?;

    let creator_info =
        match get_creator_info(http_client, api_key, channel_id, youtube_quota_usage).await {
            Ok(creator_info) => creator_info,
            Err(error) => {
                error!(?error, "failed to get creator info");
                return None;
            }
        };

    Some(creator_info)
}

async fn get_livestream_details(
    http_client: &reqwest::Client,
    api_key: &ApiKeyRef,
    creator_name: &YoutubeHandleRef,
    youtube_quota_usage: &YoutubeQuotaUsageMetric,
) -> Option<LiveStreamDetails> {
    let video_id = match get_livestream_video_id(http_client, creator_name).await {
        Ok(video_id) => video_id,
        Err(error) => {
            error!(?error, "failed to get video id");
            return None;
        }
    };

    if let Some(video_id) = video_id {
        debug!(%video_id, "creator has live stream");

        let video_info =
            match get_video_info(http_client, api_key, &video_id, youtube_quota_usage).await {
                Ok(video_info) => video_info,
                Err(error) => {
                    error!(?error, "failed to get video info");
                    return None;
                }
            };

        if !matches!(
            video_info.snippet.live_broadcast_content.as_deref(),
            Some("live")
        ) {
            return None;
        }

        let start_time = video_info
            .live_streaming_details
            .actual_start_time
            .expect("actual_start_time field should be present in liveStreamingDetails");
        let concurrent_viewers =
            video_info
                .live_streaming_details
                .concurrent_viewers
                .map(|viewers| {
                    viewers
                        .parse()
                        .expect("concurrent_viewers should be a valid integer")
                });
        let title = video_info
            .snippet
            .title
            .expect("title should be present in snippet");

        let livestream_details = LiveStreamDetails {
            href: format!("https://youtube.com/watch?v={video_id}"),
            title,
            start_time: OffsetDateTime::parse(&start_time, &well_known::Rfc3339)
                .expect("start_time should be a valid RFC3339 date-time"),
            viewers: concurrent_viewers,
        };

        info!(?livestream_details, "creator is live");

        return Some(livestream_details);
    }

    None
}
