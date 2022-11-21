use std::collections::{HashMap, HashSet};

use axum::http::response;
use google_youtube3::api::{Video, VideoListResponse};
use reqwest::Url;
use scraper::Selector;
use serde::Deserialize;
use tokio::stream;
use tracing::trace;

#[derive(Deserialize, Debug)]
pub struct YoutubeEnvironment {
    #[serde(rename = "youtube_api_key")]
    api_key: String,

    #[serde(rename = "youtube_client_id")]
    client_id: String,
    #[serde(rename = "youtube_client_secret")]
    client_secret: String,
}

pub async fn youtube_live_watcher(
    http_client: reqwest::Client,
    environment: YoutubeEnvironment,
    creators: HashSet<String>,
) {
    for creator in creators {
        let request = http_client
            .get(format!("https://youtube.com/{creator}/live"))
            .header(
                "user-agent",
                "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
            ) // Impersonate googlebot cause fuck google
            .build()
            .expect("building youtube request");

        let response = http_client.execute(request).await.expect("TODO: piss");
        let status = response.status();
        let response = response.text().await.expect("TODO: piss");

        if !status.is_success() {
            panic!("aw shit, got a {} error: {:?}", status, response);
        }

        let html = scraper::Html::parse_document(&response);
        let selector = Selector::parse("link[rel=canonical]").unwrap();

        let canonical = html
            .select(&selector)
            .next()
            .and_then(|element| element.value().attr("href"));

        dbg!(canonical);

        if let Some(canonical_url) = canonical {
            let url = canonical_url
                .parse::<Url>()
                .expect("canonical href should be a valid url");

            assert!(
                url.host_str() == Some("www.youtube.com"),
                "canonical url should point to www.youtube.com"
            );
            assert!(url.scheme() == "https", "canonical url should be https");

            if url.path() == "/watch" {
                let video_id = url
                    .query_pairs()
                    .find_map(|(key, value)| (key == "v").then_some(value))
                    .expect("canonical url should have a `v` query parameter with the video id");

                dbg!(creator, &video_id);

                let request = http_client.get(format!("https://youtube.googleapis.com/youtube/v3/videos?part=snippet%2Cstatistics%2CliveStreamingDetails&id={video_id}&key={}", environment.api_key)).header("accept", "application/json").build().expect("request should be valid");
                let response = http_client.execute(request).await.expect("TODO: piss");

                let status = response.status();

                if !status.is_success() {
                    panic!("aw shit, got a {} error", status);
                }

                let response: VideoListResponse = response.json().await.expect("TODO: piss");

                let items = response.items.expect("items part should exist in response");
                let [video] = &items[..] else {
                    panic!("video provided by canonical link must be only item")
                };

                let live_streaming_details = video
                    .live_streaming_details
                    .as_ref()
                    .expect("liveStreamingDetails part should exist in response");
                let snippet = video
                    .snippet
                    .as_ref()
                    .expect("snippet part should exist in response");
                let statistics = video
                    .statistics
                    .as_ref()
                    .expect("statistics part should exist in response");

                dbg!(live_streaming_details, snippet, statistics);
            }
        }
    }

    // http_client.get(url)        .bearer_auth(environment.api_key)

    // let auth = google_youtube3::oauth2::InstalledFlowAuthenticator::builder(
    //     ApplicationSecret {
    //         client_id: environment.client_id,
    //         client_secret: environment.client_secret,
    //         auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".into(),
    //         token_uri: "https://oauth2.googleapis.com/token".into(),
    //         auth_provider_x509_cert_url: Some("https://www.googleapis.com/oauth2/v1/certs".into()),
    //         redirect_uris: vec![
    //             "https://creatorsforacause.dusterthefirst.com".into(),
    //             "http://127.0.0.1:40643".into(),
    //         ],
    //         ..Default::default()
    //     },
    //     InstalledFlowReturnMethod::HTTPRedirect,
    // )
    // .build()
    // .await
    // .unwrap();

    // let mut hub = YouTube::new(
    //     hyper::Client::builder().build(
    //         hyper_rustls::HttpsConnectorBuilder::new()
    //             .with_native_roots()
    //             .https_only()
    //             .enable_http1()
    //             .enable_http2()
    //             .build(),
    //     ),
    //     auth,
    // );

    // let streams = hub.videos().list(vec!["liveStreamingDetails"])
    //     .live_broadcasts()
    //     .list(&vec![])
    //     .broadcast_status("active")
    //     .broadcast_type("persistent")
    //     .doit()
    //     .await
    //     .expect("TODO: piss");
}
