use once_cell::sync::Lazy;
use reqwest::Url;
use scraper::{Html, Selector};
use tracing::{error, trace, warn};

use super::api::{ChannelId, VideoId, WebError, YoutubeHandleRef};

#[tracing::instrument(skip(http_client))]
pub async fn get_livestream_video_id(
    http_client: &reqwest::Client,
    creator_name: &YoutubeHandleRef,
) -> Result<Option<VideoId>, WebError> {
    let canonical_url = match get_canonical_youtube_url(
        http_client,
        format!("https://youtube.com/{creator_name}/live"),
    )
    .await
    {
        Ok(Some(canonical_url)) => canonical_url,
        Ok(None) => return Ok(None),
        Err(error) => return Err(error),
    };

    // Ensure that the url is a watch (video) url
    if canonical_url.path() != "/watch" {
        if canonical_url
            .path_segments()
            .expect("https urls should always be able to be a base")
            .next()
            != Some("channel")
        {
            warn!(%canonical_url, "canonical url is not a watch url or channel url");
        }

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

#[tracing::instrument(skip(http_client))]
pub async fn get_channel_id(
    http_client: &reqwest::Client,
    creator_name: &YoutubeHandleRef,
) -> Result<Option<ChannelId>, WebError> {
    let canonical_url =
        match get_canonical_youtube_url(http_client, format!("https://youtube.com/{creator_name}"))
            .await
        {
            Ok(Some(canonical_url)) => canonical_url,
            Ok(None) => return Ok(None),
            Err(error) => return Err(error),
        };

    // Ensure that the url is a watch (video) url
    if let Some(mut path_segments) = canonical_url.path_segments() {
        if path_segments.next() != Some("channel") {
            return Ok(None);
        }

        Ok(Some(
            path_segments
                .next()
                .expect("channel url should contain a channel id")
                .into(),
        ))
    } else {
        warn!(%canonical_url, "canonical url cannot be a base");

        Ok(None)
    }
}

#[tracing::instrument(skip(http_client))]
async fn get_canonical_youtube_url(
    http_client: &reqwest::Client,
    url: String,
) -> Result<Option<Url>, WebError> {
    let request = http_client
        .get(url)
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
            %canonical_url, "canonical url does not point to www.youtube.com"
        );

        return Ok(None);
    }

    Ok(Some(canonical_url))
}
