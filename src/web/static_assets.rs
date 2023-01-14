use axum::{
    extract::{OriginalUri, Path},
    http::HeaderValue,
    response::{IntoResponse, Response},
};
use hyper::{header, HeaderMap, StatusCode};
use rust_embed_for_web::{EmbedableFile, RustEmbed};
use time::{format_description::well_known, OffsetDateTime};

#[derive(RustEmbed)]
#[folder = "${CARGO_MANIFEST_DIR}/static/"]
#[exclude = "LICENSE"]
struct StaticAssets;

pub async fn handler(OriginalUri(uri): OriginalUri, headers: HeaderMap) -> Response {
    let file_path = uri.path();

    let file_path = if file_path == "/" {
        "dashboard.html"
    } else {
        file_path
    };

    dbg!(&file_path);

    let file = match StaticAssets::get(&file_path) {
        Some(file) => file,
        None => {
            // TODO: better 404 page
            return (StatusCode::NOT_FOUND, "Not Found").into_response();
        }
    };

    // Response headers
    let mut response_headers = HeaderMap::<HeaderValue>::with_capacity(3);

    response_headers.append(
        header::ETAG,
        file.etag()
            .parse()
            .expect("etag should be a valid header value"),
    );

    if let Some(last_modified) = file.last_modified() {
        response_headers.append(
            header::LAST_MODIFIED,
            last_modified
                .parse()
                .expect("last_modified should be a valid header value"),
        );
    }

    if let Some(mime) = file.mime_type() {
        response_headers.append(
            header::CONTENT_TYPE,
            mime.parse()
                .expect("mime type should be a valid header value"),
        );
    };

    // Encoding
    let selected_encoding = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split(',')
                .flat_map(|etag| Compression::from_str(etag.trim()))
                .max()
        });

    dbg!(selected_encoding);

    let encoded_data = selected_encoding.and_then(|encoding| {
        match encoding {
            Compression::GZip => file.data_gzip(),
            Compression::Brotli => file.data_br(),
        }
        .map(|data| (data, encoding))
    });

    dbg!(&encoded_data);

    let data = match encoded_data {
        Some((data, encoding)) => {
            response_headers.insert(header::CONTENT_ENCODING, encoding.into_header_value());
            data
        }
        None => file.data(),
    };

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

    (response_headers, data).into_response()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
