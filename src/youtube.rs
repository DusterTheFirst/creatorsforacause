use std::{
    collections::{BTreeSet, HashSet},
    fmt::Debug,
    rc::Rc,
};

use serde::Deserialize;
use time::{format_description::well_known, OffsetDateTime};
use tokio::{
    sync::watch,
    task::JoinSet,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, trace, Instrument};

use crate::model::{Creator, CreatorsList, LiveStreamDetails, YoutubeSource};

use self::{
    api::{get_creator_info, get_video_info, ApiKey, YoutubeHandle},
    scraping::{get_channel_id, get_livestream_video_id},
};

pub mod api;
mod scraping;

#[derive(Deserialize, Debug)]
pub struct YoutubeEnvironment {
    #[serde(rename = "youtube_api_key")]
    api_key: ApiKey,
}

pub async fn youtube_live_watcher(
    http_client: reqwest::Client,
    environment: YoutubeEnvironment,
    creators: HashSet<YoutubeHandle>,
    status_sender: watch::Sender<CreatorsList<YoutubeSource>>,
) {
    let api_key = Rc::new(environment.api_key);

    let mut next_refresh = Instant::now();
    let refresh_interval = Duration::from_secs(60 * 10);

    loop {
        let creators = get_creators(&creators, &http_client, &api_key).await;

        // Send status to web server
        status_sender.send_replace(CreatorsList {
            updated: OffsetDateTime::now_utc(),
            creators,
        });

        // Refresh every 10 minutes
        next_refresh += refresh_interval;
        trace!(?refresh_interval, "Waiting for next refresh");

        tokio::time::sleep_until(next_refresh).await;
    }
}

#[tracing::instrument(skip_all)]
async fn get_creators(
    creator_names: &HashSet<YoutubeHandle>,
    http_client: &reqwest::Client,
    api_key: &Rc<ApiKey>,
) -> BTreeSet<Creator<YoutubeSource>> {
    let mut set = JoinSet::new();
    for creator_name in creator_names.iter().cloned() {
        let http_client = http_client.clone();
        let api_key = api_key.clone();

        let span = tracing::trace_span!("creator_update", ?creator_name);

        set.spawn_local(
            async move {
                let livestream_details = tokio::task::spawn_local({
                    let http_client = http_client.clone();
                    let api_key = api_key.clone();
                    let creator_name = creator_name.clone();

                    async move {
                    let video_id = match get_livestream_video_id(&http_client, &creator_name).await {
                        Ok(video_id) => {video_id},
                        Err(error) => {
                            error!(?error, "failed to get video id");
                            return None;
                        },
                    };

                    if let Some(video_id) = video_id {
                        debug!(%video_id, "creator has live stream");


                        let video_info = match get_video_info(&http_client, &api_key, &video_id ).await {
                            Ok(video_info) => {video_info},
                            Err(error) => {
                                error!(?error, "failed to get video info");
                                return None;
                            },
                        };

                        if matches!(
                            video_info.snippet.live_broadcast_content.as_deref(),
                            Some("live")
                        ) {
                            let start_time = video_info
                                .live_streaming_details
                                .actual_start_time
                                .expect("actual_start_time field should be present in liveStreamingDetails");
                            let concurrent_viewers = video_info
                                .live_streaming_details
                                .concurrent_viewers
                                .expect("concurrent_viewers field should be present in liveStreamingDetails")
                                .parse()
                                .expect("concurrent_viewers should be a valid integer");
                            let title = video_info
                                .snippet
                                .title
                                .expect("title should be present in snippet");

                            let livestream_details = LiveStreamDetails {
                                href: format!("https://youtube.com/watch?v={}", video_id),
                                title,
                                start_time: OffsetDateTime::parse(&start_time, &well_known::Rfc3339).expect("start_time should be a valid RFC3339 date-time"),
                                viewers: concurrent_viewers,
                            };

                            info!(?livestream_details, "creator is live");

                            return Some(livestream_details);
                        }
                    }

                    None
                }});

                let creator_info = tokio::task::spawn_local({
                    let http_client = http_client.clone();
                    let api_key = api_key.clone();

                    async move {
                        let channel_id = match get_channel_id(&http_client, &creator_name).await {
                            Ok(channel_id) => {channel_id},
                            Err(error) => {
                                error!(?error, "failed to get channel id");
                                return None;
                            },
                        };

                        if let Some(channel_id) = channel_id {
                            let creator_info = match get_creator_info(&http_client, &api_key, channel_id).await {
                                Ok(creator_info) => {creator_info},
                                Err(error) => {
                                    error!(?error, "failed to get creator info");
                                    return None;
                                },
                            };

                            let display_name = creator_info.snippet.title.expect("title field should be present in snippet");
                            let icon_url = creator_info.snippet.thumbnails.expect("thumbnails field should be present in snippet").default.expect("default thumbnail should exist").url.expect("default thumbnail url should exist");

                            let livestream_details = livestream_details.await.expect("failed to drive livestream_details to completion");

                            Some(Creator {
                                display_name,
                                href: format!("https://youtube.com/{creator_name}"),
                                icon_url,
                                stream: livestream_details,
                                internal_identifier: creator_name,
                            })
                        } else {
                            None
                        }
                    }
                });

                creator_info.await.expect("failed to drive creator_info to completion")
            }
            .instrument(span),
        );
    }

    // Drive all futures to completion, collecting their results
    let mut live_broadcasts: BTreeSet<Creator<YoutubeSource>> = BTreeSet::new();

    while let Some(result) = set.join_next().await {
        match result {
            Ok(Some(creator)) => {
                live_broadcasts.insert(creator);
            }
            Ok(None) => {}
            Err(error) => error!(%error, "failed to drive creator future to completion"),
        }
    }

    live_broadcasts
}
