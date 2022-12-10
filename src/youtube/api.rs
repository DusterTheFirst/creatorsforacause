use std::fmt::Debug;

use google_youtube3::api::{
    ChannelListResponse, ChannelSnippet, VideoListResponse, VideoLiveStreamingDetails, VideoSnippet,
};
use hyper::StatusCode;
use once_cell::sync::Lazy;
use reqwest::Url;
use tracing::warn;

#[aliri_braid::braid(serde)]
pub struct YoutubeHandle;

#[aliri_braid::braid(serde)]
pub struct VideoId;

#[aliri_braid::braid(serde)]
pub struct ChannelId;

#[aliri_braid::braid(serde, display = "omit", debug = "omit")]
pub struct ApiKey;

impl Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "*****")
    }
}

#[derive(Debug)]
pub enum WebError {
    Request(reqwest::Error),
    Status(StatusCode),
    Body(reqwest::Error),
}

pub struct CreatorInfo {
    pub snippet: ChannelSnippet,
}

#[tracing::instrument(skip(http_client, api_key))]
pub async fn get_creator_info(
    http_client: &reqwest::Client,
    api_key: &ApiKey,
    channel_id: ChannelId,
) -> Result<CreatorInfo, WebError> {
    // Channel API endpoint
    static CHANNEL_API_URL: Lazy<Url> = Lazy::new(|| {
        Url::parse("https://www.googleapis.com/youtube/v3/channels").expect("url should be valid")
    });

    // Create the video api url for the specific video id
    let channel_api_url = {
        let mut url = CHANNEL_API_URL.clone();

        url.query_pairs_mut()
            .append_pair("part", "snippet")
            .append_pair("id", channel_id.as_str())
            .append_pair("key", api_key.as_str());

        url
    };

    // Get more information about the given channel
    let request = http_client
        .get(channel_api_url)
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
    let response: ChannelListResponse = response
        .json()
        .await
        .map_err(|err| WebError::Body(err.without_url()))?;

    // Extract the items
    let mut items = response.items.expect("items part should exist in response");

    // Get the first channel response (the only one)
    let channel = items
        .pop()
        .expect("channel provided by canonical link must exist");

    // Warn if there were more than 1 channel returned from the API
    if !items.is_empty() {
        warn!("multiple channels were provided from the API response");
    }

    // Extract important information from the response
    Ok(CreatorInfo {
        snippet: channel
            .snippet
            .expect("snippet part should exist in response"),
    })
}

pub struct VideoInfo {
    pub live_streaming_details: VideoLiveStreamingDetails,
    pub snippet: VideoSnippet,
}

#[tracing::instrument(skip(http_client, api_key))]
pub async fn get_video_info(
    http_client: &reqwest::Client,
    api_key: &ApiKey,
    video_id: &VideoIdRef,
) -> Result<VideoInfo, WebError> {
    // Video API endpoint
    static VIDEO_API_URL: Lazy<Url> = Lazy::new(|| {
        Url::parse("https://www.googleapis.com/youtube/v3/videos").expect("url should be valid")
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
    Ok(VideoInfo {
        live_streaming_details: video
            .live_streaming_details
            .expect("liveStreamingDetails part should exist in response"),
        snippet: video
            .snippet
            .expect("snippet part should exist in response"),
    })
}
