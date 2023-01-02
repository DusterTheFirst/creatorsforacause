use std::sync::Arc;

use axum::http::HeaderValue;
use color_eyre::{eyre::Context, Help};
use hyper::{header, Method};
use serde::Deserialize;
use tracing::trace;

use crate::{config::CampaignConfig, model::Campaign};

#[derive(Debug, Deserialize)]
struct Meta {
    status: u16,
}

#[derive(Debug, Deserialize)]
struct TiltifyJson<D> {
    meta: Meta,
    data: D,
}

pub struct TiltifyWatcher {
    http_client: reqwest::Client,
    campaign: CampaignConfig,
    api_key: String,

    // ETag based cache
    cache: Option<(Option<HeaderValue>, Arc<Campaign>)>,
}

impl TiltifyWatcher {
    pub fn new(http_client: reqwest::Client, campaign: CampaignConfig, api_key: String) -> Self {
        Self {
            http_client,
            campaign,
            api_key,
            cache: None,
        }
    }

    // No known rate limit
    #[tracing::instrument(skip(self))]
    pub async fn get_campaign<'s>(&'s mut self) -> color_eyre::Result<Arc<Campaign>> {
        let request = self
            .http_client
            .get(format!(
                "https://tiltify.com/api/v3/campaigns/{}",
                self.campaign.id
            ))
            .bearer_auth(&self.api_key)
            .build()
            .expect("tiltify request should be well formed");

        // Assuming `cache-control: must-revalidate, private, max-age=0`
        // as that is what the endpoint headers stated at time of development
        if let Some((cache_etag, cached)) = self.cache.as_ref() {
            trace!(?cache_etag, "cache is populated, validating");

            let head_request = {
                let mut request = request
                    .try_clone()
                    .expect("request should be able to be cloned");

                *request.method_mut() = Method::HEAD;

                request
            };

            let response = self
                .http_client
                .execute(head_request)
                .await
                .wrap_err("tiltify api HEAD request failed")?
                .error_for_status()
                .wrap_err("tiltify api returned non success status code")?;

            let etag = response.headers().get(header::ETAG);

            match (etag, cache_etag) {
                (Some(etag), Some(cache_etag)) if etag == cache_etag => {
                    trace!(?cache_etag, "cache is valid, using cached campaign");

                    return Ok(Arc::clone(cached));
                }
                _ => {
                    trace!(?cache_etag, ?etag, "cache is invalid");
                }
            }
        } else {
            trace!("cache miss");
        }

        let response = self
            .http_client
            .execute(request)
            .await
            .wrap_err("tiltify api request failed")?
            .error_for_status()
            .wrap_err("tiltify api returned non success status code")?;

        let etag = response.headers().get(header::ETAG).cloned();

        let response = response
            .text()
            .await
            .wrap_err("unable to receive text response from tiltify api")?;

        let json: TiltifyJson<Campaign> = serde_json::from_str(&response)
            .wrap_err("incompatible json received from tiltify api")
            .with_note(|| response)?;

        let data = Arc::new(json.data);

        trace!(?etag, "caching new campaign");

        self.cache = Some((etag, Arc::clone(&data)));

        Ok(data)
    }
}
