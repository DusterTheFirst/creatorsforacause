use std::fmt::Debug;

use color_eyre::eyre::Context;
use futures::{stream::FuturesUnordered, TryStreamExt};
use serde::Deserialize;
use time::{format_description::well_known, OffsetDateTime};
use tokio::pin;
use tracing::{debug, info, Instrument};

use crate::{
    metrics::types::YoutubeQuotaUsageMetric,
    model::{Creator, LiveStreamDetails, StreamingService},
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
) -> color_eyre::Result<Vec<Creator>> {
    let futures: FuturesUnordered<_> = creator_names
        .iter()
        .map(|creator_name| {
            let span = tracing::trace_span!("creator_update", ?creator_name);

            async move {
                tokio::try_join!(
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
            .instrument(span)
        })
        .collect();

    pin!(futures);

    // Drive all futures to completion, collecting their results
    futures
        .map_ok(|(creator_info, livestream_details)| {
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

            Creator {
                service: StreamingService::Youtube,
                id: creator_info.id.take(),
                display_name,
                href: format!("https://youtube.com/{custom_url}"),
                handle: custom_url,
                icon_url,
                stream: livestream_details,
            }
        })
        .try_collect()
        .await
}

#[tracing::instrument(skip(http_client, api_key, youtube_quota_usage))]
async fn get_creator_info_from_handle(
    http_client: &reqwest::Client,
    api_key: &ApiKeyRef,
    handle: &YoutubeHandleRef,
    youtube_quota_usage: &YoutubeQuotaUsageMetric,
) -> color_eyre::Result<CreatorInfo> {
    let channel_id = get_channel_id(http_client, handle)
        .await
        .wrap_err("failed to get channel id")?;

    get_creator_info(http_client, api_key, channel_id, youtube_quota_usage)
        .await
        .wrap_err("failed to get creator info")
}

#[tracing::instrument(skip(http_client, api_key, youtube_quota_usage))]
async fn get_livestream_details(
    http_client: &reqwest::Client,
    api_key: &ApiKeyRef,
    creator_name: &YoutubeHandleRef,
    youtube_quota_usage: &YoutubeQuotaUsageMetric,
) -> color_eyre::Result<Option<LiveStreamDetails>> {
    let video_id = get_livestream_video_id(http_client, creator_name)
        .await
        .wrap_err("failed to get video id")?;

    if let Some(video_id) = video_id {
        debug!(%video_id, "creator has live stream");

        let video_info = get_video_info(http_client, api_key, &video_id, youtube_quota_usage)
            .await
            .wrap_err("failed to get video info")?;

        // Return early if the channel is not live
        if !matches!(
            video_info.snippet.live_broadcast_content.as_deref(),
            Some("live")
        ) {
            return Ok(None);
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

        Ok(Some(livestream_details))
    } else {
        Ok(None)
    }
}
