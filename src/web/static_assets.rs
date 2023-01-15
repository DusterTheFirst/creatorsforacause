use std::collections::HashSet;

use axum::{
    extract::OriginalUri,
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use hyper::{header, HeaderMap, StatusCode};
use rust_embed_for_web::{EmbedableFile, RustEmbed};
use time::{format_description::well_known, OffsetDateTime};
use tracing::debug;

#[derive(RustEmbed)]
#[folder = "${CARGO_MANIFEST_DIR}/static/"]
#[exclude = "LICENSE"]
struct StaticAssets;

#[tracing::instrument(skip(headers))]
pub async fn handler(OriginalUri(uri): OriginalUri, headers: HeaderMap) -> Response {
    let file_path = uri.path().trim_start_matches('/');

    let file_path = if file_path.is_empty() {
        "dashboard.html"
    } else {
        file_path
    };

    let file = match StaticAssets::get(file_path) {
        Some(file) => file,
        None => {
            // TODO: better 404 page
            return (StatusCode::NOT_FOUND, "Not Found").into_response();
        }
    };

    // Encoding
    let accepted_encodings = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map_or(HashSet::new(), |value| {
            value
                .split(',')
                .flat_map(|etag| Compression::from_str(etag.trim()))
                .collect()
        });

    let supported_encodings: HashSet<_> = [
        file.data_br().is_some().then_some(Compression::Brotli),
        file.data_gzip().is_some().then_some(Compression::Brotli),
    ]
    .into_iter()
    .flatten()
    .collect();

    let selected_encoding = supported_encodings
        .intersection(&accepted_encodings)
        .max()
        .copied();

    // Response headers
    let response_headers: HeaderMap<_> = [
        Some((header::CACHE_CONTROL, HeaderValue::from_static("no-cache"))),
        Some((
            header::ETAG,
            file.etag()
                .parse()
                .expect("etag should be a valid header value"),
        )),
        file.last_modified().map(|last_modified| {
            (
                header::LAST_MODIFIED,
                last_modified
                    .parse()
                    .expect("last_modified should be a valid header value"),
            )
        }),
        file.mime_type().map(|mime| {
            (
                header::CONTENT_TYPE,
                mime.parse()
                    .expect("mime type should be a valid header value"),
            )
        }),
        selected_encoding.map(|encoding| (header::CONTENT_ENCODING, encoding.into_header_value())),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Conditional request (Caching)
    // Prefer ETag over If-Modified-Since

    // Ignore input if it contains errors
    for etag in headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .iter()
        .flat_map(|value| value.split(',').map(|etag| etag.trim()))
    {
        if etag == file.etag() {
            return (response_headers, StatusCode::NOT_MODIFIED).into_response();
        }
    }
    if let Some(last_modified) = file.last_modified_timestamp() {
        let last_modified = OffsetDateTime::from_unix_timestamp(last_modified)
            .expect("last_modified should be a valid unix timestamp");

        // Ignore input if it contains errors
        if let Some(since) = headers
            .get(header::IF_MODIFIED_SINCE)
            .and_then(|since| since.to_str().ok())
            .and_then(|since| OffsetDateTime::parse(since, &well_known::Rfc2822).ok())
        {
            if last_modified <= since {
                return (response_headers, StatusCode::NOT_MODIFIED).into_response();
            }
        }
    }

    let data = match selected_encoding {
        Some(Compression::Brotli) => file.data_br().expect("brotli data should exist"),
        Some(Compression::GZip) => file.data_gzip().expect("gzip data should exist"),
        None => file.data(),
    };

    debug!(
        ?accepted_encodings,
        ?supported_encodings,
        ?selected_encoding,
        "serving static asset"
    );

    (response_headers, data).into_response()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Compression {
    GZip = 0,
    // Preferred
    Brotli = 1,
}

impl Compression {
    pub fn into_header_value(self) -> HeaderValue {
        match self {
            Compression::GZip => HeaderValue::from_static("gzip"),
            Compression::Brotli => HeaderValue::from_static("br"),
        }
    }

    pub fn from_str(str: &str) -> Option<Self> {
        match str {
            "br" => Some(Self::Brotli),
            "gzip" => Some(Self::GZip),
            _ => None,
        }
    }
}
