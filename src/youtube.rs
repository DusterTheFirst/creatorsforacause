use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    rc::Rc,
};

use google_youtube3::api::{VideoListResponse, VideoLiveStreamingDetails, VideoSnippet};
use once_cell::sync::Lazy;
use reqwest::{StatusCode, Url};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::{
    sync::watch,
    task::JoinSet,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, trace, warn, Instrument};

use crate::web::LiveStreamDetails;

#[derive(Deserialize, Debug)]
pub struct YoutubeEnvironment {
    #[serde(rename = "youtube_api_key")]
    api_key: ApiKey,
}

#[aliri_braid::braid(serde)]
pub struct YoutubeHandle;

#[aliri_braid::braid(serde)]
pub struct VideoId;

#[aliri_braid::braid(serde, display = "omit", debug = "omit")]
pub struct ApiKey;

impl Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "*****")
    }
}

pub async fn youtube_live_watcher(
    http_client: reqwest::Client,
    environment: YoutubeEnvironment,
    creators: HashSet<YoutubeHandle>,
    status_sender: watch::Sender<YoutubeLiveStreams>,
) {
    let api_key = Rc::new(environment.api_key);

    let mut next_refresh = Instant::now();
    let refresh_interval = Duration::from_secs(60 * 10);

    loop {
        update_live_statuses(&creators, &http_client, &api_key, &status_sender).await;

        // Refresh every 10 minutes
        next_refresh += refresh_interval;
        trace!(?refresh_interval, "Waiting for next refresh");

        tokio::time::sleep_until(next_refresh).await;
    }
}

#[tracing::instrument(skip_all)]
async fn update_live_statuses(
    creators: &HashSet<YoutubeHandle>,
    http_client: &reqwest::Client,
    api_key: &Rc<ApiKey>,
    status_sender: &watch::Sender<YoutubeLiveStreams>,
) {
    let mut set = JoinSet::new();
    for creator in creators.iter().cloned() {
        let http_client = http_client.clone();
        let api_key = api_key.clone();

        let span = tracing::trace_span!("creator_update", ?creator);

        set.spawn_local(
            async move {
                let video_id = get_livestream_video_id(&http_client, &creator)
                    .await
                    .expect("TODO: piss");

                if let Some(video_id) = video_id {
                    debug!(%video_id, "creator has live stream");

                    let video_info = get_video_info(&http_client, &video_id, &api_key)
                        .await
                        .expect("TODO: piss");

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
                            .expect("concurrent_viewers field should be present in liveStreamingDetails");
                        let title = video_info
                            .snippet
                            .title
                            .expect("title should be present in snippet");

                        let livestream_details = LiveStreamDetails {
                            href: format!("https://youtube.com/watch?v={}", video_id),
                            title,
                            start_time,
                            concurrent_viewers,
                        };

                        info!(?livestream_details, "creator is live");

                        return Some((creator, livestream_details));
                    }
                }

                None
            }
            .instrument(span),
        );
    }

    // Drive all futures to completion, collecting their results
    let mut live_broadcasts: HashMap<YoutubeHandle, Option<LiveStreamDetails>> = creators
        .iter()
        .cloned()
        .map(|handle| (handle, None))
        .collect();

    while let Some(result) = set.join_next().await {
        match result {
            Ok(Some((creator, broadcast_info))) => {
                live_broadcasts.insert(creator, Some(broadcast_info));
            }
            Ok(None) => {}
            Err(error) => error!(%error, "failed to drive creator future to completion"),
        }
    }

    // Send status to web server
    status_sender.send_replace(YoutubeLiveStreams {
        updated: OffsetDateTime::now_utc(),
        streams: live_broadcasts,
    });
}

#[derive(Debug, Serialize)]

pub struct YoutubeLiveStreams {
    #[serde(with = "time::serde::rfc3339")]
    pub updated: OffsetDateTime,
    pub streams: HashMap<YoutubeHandle, Option<LiveStreamDetails>>,
}

struct VideoInfo {
    live_streaming_details: VideoLiveStreamingDetails,
    snippet: VideoSnippet,
}

#[tracing::instrument(skip(http_client, api_key))]
async fn get_video_info(
    http_client: &reqwest::Client,
    video_id: &VideoIdRef,
    api_key: &ApiKey,
) -> Result<VideoInfo, WebError> {
    // Video API endpoint
    static VIDEO_API_URL: Lazy<Url> = Lazy::new(|| {
        Url::parse("https://youtube.googleapis.com/youtube/v3/videos").expect("url should be valid")
    });

    // Create the video api url for the specific video id
    let video_api_url = {
        let mut url = VIDEO_API_URL.clone();

        url.query_pairs_mut()
            .append_pair("part", "snippet,liveStreamingDetails")
            .append_pair("id", video_id.as_str())
            .append_pair("key", api_key.as_str());

        url
    };

    // Get more information about the given video
    let request = http_client
        .get(video_api_url)
        .header("accept", "application/json")
        .build()
        .expect("youtube api request should be a valid request");

    // Get the headers and return an error if non-success status code
    let response = http_client
        .execute(request)
        .await
        .map_err(|err| WebError::Request(err.without_url()))?
        .error_for_status()
        .map_err(|err| WebError::Status(err.status().expect("status should exist on error")))?;

    // Parse and read in the response
    let response: VideoListResponse = response
        .json()
        .await
        .map_err(|err| WebError::Body(err.without_url()))?;

    // Extract the items
    let mut items = response.items.expect("items part should exist in response");

    // Get the first video response (the only one)
    let video = items
        .pop()
        .expect("video provided by canonical link must exist");

    // Warn if there were more than 1 video returned from the API
    if !items.is_empty() {
        warn!("multiple videos were provided from the API response");
    }

    // Extract important information from the response
    let live_streaming_details = video
        .live_streaming_details
        .expect("liveStreamingDetails part should exist in response");
    let snippet = video
        .snippet
        .expect("snippet part should exist in response");

    Ok(VideoInfo {
        live_streaming_details,
        snippet,
    })
}

#[derive(Debug)]
enum WebError {
    Request(reqwest::Error),
    Status(StatusCode),
    Body(reqwest::Error),
}

#[tracing::instrument(skip(http_client))]
async fn get_livestream_video_id(
    http_client: &reqwest::Client,
    creator: &YoutubeHandleRef,
) -> Result<Option<VideoId>, WebError> {
    // Get the live stream html page
    let request = http_client
        .get(format!("https://youtube.com/{creator}/live"))
        // Impersonate googlebot cause fuck google
        .header(
            "user-agent",
            "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        )
        .build()
        .expect("youtube request should be a valid request");

    // Get the headers and return an error if non-success status code
    let response = http_client
        .execute(request)
        .await
        .map_err(WebError::Request)?
        .error_for_status()
        .map_err(|err| WebError::Status(err.status().expect("status should exist on error")))?;

    // Read the body as a utf-8 string
    let response = response.text().await.map_err(WebError::Body)?;

    // Parse the html content
    let html = Html::parse_document(&response);

    static SELECTOR: Lazy<Selector> =
        Lazy::new(|| Selector::parse("link[rel=canonical]").expect("selector should be valid"));

    // Get the canonical url from the first <link rel="canonical" href="..."/>
    let canonical_url = html
        .select(&SELECTOR)
        .next()
        .and_then(|element| element.value().attr("href"));

    // If no canonical url found, return none
    let canonical_url = match canonical_url {
        Some(url) => url
            .parse::<Url>()
            .expect("canonical href should be a valid url"),
        None => {
            trace!("no canonical url found in response");

            return Ok(None);
        }
    };

    // Assert that the host string is pointing to youtube
    if canonical_url.host_str() != Some("www.youtube.com") {
        error!(
            %canonical_url,
            %creator, "canonical url does not point to www.youtube.com"
        );

        return Ok(None);
    }

    // Ensure that the url is a watch (video) url
    if canonical_url.path() != "/watch" {
        trace!(%canonical_url, "canonical url is not a watch url");

        return Ok(None);
    }

    // Get the video ID from the query parameters
    let video_id = canonical_url
        .query_pairs()
        .find(|(key, _)| key == "v")
        .map(|(_, value)| value)
        .expect("canonical url should have a `v` query parameter with the video id");

    Ok(Some(video_id.into_owned().into()))
}
