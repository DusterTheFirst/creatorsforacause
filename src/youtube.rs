use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
};

use google_youtube3::api::{
    VideoListResponse, VideoLiveStreamingDetails, VideoSnippet, VideoStatistics,
};
use once_cell::sync::Lazy;
use reqwest::{StatusCode, Url};
use scraper::{Html, Selector};
use serde::Deserialize;
use tokio::{
    sync::watch,
    time::{Duration, Instant},
};
use tracing::{debug, error, trace, warn};

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
) {
    for creator in creators {
        let video_id = get_livestream_video_id(&http_client, &creator)
            .await
            .expect("TODO: piss");

        if let Some(video_id) = video_id {
            debug!(%creator, %video_id, "creator has live stream");

            let video_info = get_video_info(&http_client, &video_id, &environment.api_key)
                .await
                .expect("TODO: piss");

            // TODO:
            // dbg!(live_streaming_details, snippet, statistics);
        }
    }

    let mut last_refresh = Instant::now();
    let mut live_statuses: HashMap<YoutubeHandle, Option<VideoId>> = HashMap::new();
    let (live_status_sender, live_status_receiver) = watch::channel(None::<()>);
    loop {
        // Refresh every 10 minutes
        tokio::time::sleep_until(last_refresh + Duration::from_secs(60 * 10)).await;
    }
}

struct VideoInfo {
    live_streaming_details: VideoLiveStreamingDetails,
    snippet: VideoSnippet,
    statistics: VideoStatistics,
}

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
            .append_pair("part", "snippet,statistics,liveStreamingDetails")
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
    let statistics = video
        .statistics
        .expect("statistics part should exist in response");

    Ok(VideoInfo {
        live_streaming_details,
        snippet,
        statistics,
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
        .select(&*SELECTOR)
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
        trace!(%canonical_url, %creator, "canonical url is not a watch url");

        return Ok(None);
    }

    // Get the video ID from the query parameters
    let video_id = canonical_url
        .query_pairs()
        .find(|(key, value)| key == "v")
        .map(|(_, value)| value)
        .expect("canonical url should have a `v` query parameter with the video id");

    Ok(Some(video_id.into_owned().into()))
}
