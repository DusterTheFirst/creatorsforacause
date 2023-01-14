use std::sync::Arc;

use axum::http::HeaderValue;
use color_eyre::{
    eyre::{Context, ContextCompat},
    Help,
};
use hyper::{header, StatusCode};
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
    cache: Option<(HeaderValue, Arc<Campaign>)>,
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
        let mut request = self
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
        if let Some((etag, _)) = self.cache.as_ref() {
            request
                .headers_mut()
                .append(header::IF_NONE_MATCH, etag.clone());
        }

        let mut response = self
            .http_client
            .execute(request)
            .await
            .wrap_err("tiltify api request failed")?
            .error_for_status()
            .wrap_err("tiltify api returned non success status code")?;

        if response.status() != StatusCode::NOT_MODIFIED {
            let etag = response
                .headers_mut()
                .remove(header::ETAG)
                .wrap_err("etag should be present in response headers")?;

            let response = response
                .text()
                .await
                .wrap_err("unable to receive text response from tiltify api")?;

            let json: TiltifyJson<Campaign> = serde_json::from_str(&response)
                .wrap_err("incompatible json received from tiltify api")
                .with_note(|| response)?;

            trace!(?etag, "caching new campaign");

            self.cache = Some((etag, Arc::new(json.data)));
        }

        let (_, cached) = self
            .cache
            .as_ref()
            .cloned()
            .expect("cache should be populated at this point");

        Ok(cached)
    }
}
