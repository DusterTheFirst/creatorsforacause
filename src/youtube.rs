use axum::http::response;
use google_youtube3::{
    hyper, hyper_rustls,
    oauth2::{ApplicationSecret, InstalledFlowReturnMethod},
    YouTube,
};
use serde::Deserialize;
use tokio::stream;

#[derive(Deserialize, Debug)]
pub struct YoutubeEnvironment {
    #[serde(rename = "youtube_api_key")]
    api_key: String,

    #[serde(rename = "youtube_client_id")]
    client_id: String,
    #[serde(rename = "youtube_client_secret")]
    client_secret: String,
}

pub async fn youtube_live_watcher(http_client: reqwest::Client, environment: YoutubeEnvironment) {
    // let request = http_client
    //     .get("https://www.youtube.com/@LofiGirl/live")
    //     // .get("https://www.googleapis.com/youtube/v3/liveBroadcasts?part=status&broadcastStatus=active&broadcastType=persistent")
    //     // .bearer_auth(environment.api_key)
    //     .build()
    //     .expect("building youtube request");

    // let response = http_client.execute(request).await.expect("TODO: piss");
    // let status = response.status();
    // let response = response.text().await.expect("TODO: piss");

    // if !status.is_success() {
    //     panic!("aw shit, got a {} error: {:?}", status, response);
    // }

    // dbg!(response);

    let auth = google_youtube3::oauth2::InstalledFlowAuthenticator::builder(
        ApplicationSecret {
            client_id: environment.client_id,
            client_secret: environment.client_secret,
            auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_uri: "https://oauth2.googleapis.com/token".into(),
            auth_provider_x509_cert_url: Some("https://www.googleapis.com/oauth2/v1/certs".into()),
            redirect_uris: vec![
                "https://creatorsforacause.dusterthefirst.com".into(),
                "http://127.0.0.1:40643".into(),
            ],
            ..Default::default()
        },
        InstalledFlowReturnMethod::HTTPRedirect,
    )
    .build()
    .await
    .unwrap();

    let mut hub = YouTube::new(
        hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .https_only()
                .enable_http1()
                .enable_http2()
                .build(),
        ),
        auth,
    );

    let streams = hub
        .live_broadcasts()
        .list(&vec![])
        .broadcast_status("active")
        .broadcast_type("persistent")
        .doit()
        .await
        .expect("TODO: piss");

    dbg!(streams);
}
