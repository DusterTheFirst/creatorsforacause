use color_eyre::{eyre::Context, Help};
use serde::Deserialize;

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

#[tracing::instrument(skip(http_client, tiltify_api_key))]
pub async fn get_campaign(
    http_client: &reqwest::Client,
    campaign: CampaignConfig,
    tiltify_api_key: &str,
) -> color_eyre::Result<Campaign> {
    let request = http_client
        .get(format!(
            "https://tiltify.com/api/v3/campaigns/{}",
            campaign.id
        ))
        .bearer_auth(tiltify_api_key)
        .build()
        .expect("tiltify request should be well formed");

    let response = http_client
        .execute(request)
        .await
        .wrap_err("tiltify api request failed")?
        .error_for_status()
        .wrap_err("tiltify api returned non success status code")?;

    let response = response
        .text()
        .await
        .wrap_err("unable to receive text response from tiltify api")?;

    let json: TiltifyJson<Campaign> = serde_json::from_str(&response)
        .wrap_err("incompatible json received from tiltify api")
        .with_note(|| response)?;

    Ok(json.data)
}
