use std::{net::SocketAddr, time::Duration};

use axum::{extract::State, routing::get, Json, Router, Server};
use hyper::header::{self, HeaderValue};
use sentry_tower::{SentryHttpLayer, SentryLayer};
use tokio::sync::watch;
use tower_http::{
    catch_panic::CatchPanicLayer, cors::CorsLayer, set_header::SetResponseHeaderLayer,
    timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::info;

use crate::watcher::WatcherDataReceive;

mod live_view;
mod markup;

pub async fn web_server(listen: SocketAddr, watcher_data: watch::Receiver<WatcherDataReceive>) {
    let app = Router::new()
        .nest("/", live_view::router(listen, watcher_data.clone()))
        .route("/healthy", get(|| async { "OK" }))
        .route_service("/json", get(json).with_state(watcher_data))
        .route(
            "/.well-known/security.txt",
            get(|| async { include_str!("../.well-known/security.txt") }),
        )
        .layer(
            tower::ServiceBuilder::new()
                .layer(SentryLayer::new_from_top())
                .layer(SentryHttpLayer::with_transaction())
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                // # Risk description
                // The Content-Security-Policy (CSP) header activates a protection mechanism implemented
                // in web browsers which prevents exploitation of Cross-Site Scripting vulnerabilities
                // (XSS). If the target application is vulnerable to XSS, lack of this header makes
                // it easily exploitable by attackers.
                // # Recommendation
                // Configure the Content-Security-Header to be sent with each HTTP response in order
                // to apply the specific policies needed by the application.
                .layer(SetResponseHeaderLayer::overriding(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static("self"),
                ))
                // # Risk description
                // The Referrer-Policy HTTP header controls how much referrer information the browser
                // will send with each request originated from the current web application. For instance,
                // if a user visits the web page "http://example.com/pricing/" and it clicks on a link
                // from that page going to e.g. "https://www.google.com", the browser will send to
                // Google the full originating URL in the `Referer` header, assuming the Referrer-Policy
                // header is not set. The originating URL could be considered sensitive information
                // and it could be used for user tracking.
                // # Recommendation
                // The Referrer-Policy header should be configured on the server side to avoid user
                // tracking and inadvertent information leakage. The value `no-referrer` of this header
                // instructs the browser to omit the Referer header entirely.
                .layer(SetResponseHeaderLayer::overriding(
                    header::REFERRER_POLICY,
                    HeaderValue::from_static("no-referrer"),
                ))
                // # Risk description
                // The HTTP Strict-Transport-Security header instructs the browser to initiate only
                // secure (HTTPS) connections to the web server and deny any unencrypted HTTP connection
                // attempts. Lack of this header permits an attacker to force a victim user to initiate
                // a clear-text HTTP connection to the server, thus opening the possibility to eavesdrop
                // on the network traffic and extract sensitive information (e.g. session cookies).
                // # Recommendation
                // The Strict-Transport-Security HTTP header should be sent with each HTTPS response.
                // The syntax is as follows: `Strict-Transport-Security: max-age=<seconds>[; includeSubDomains]`
                // The parameter `max-age` gives the time frame for requirement of HTTPS in seconds
                // and should be chosen quite high, e.g. several months. A value below 7776000 is
                // considered as too low by this scanner check. The flag `includeSubDomains` defines
                // that the policy applies also for sub domains of the sender of the response.
                .layer(SetResponseHeaderLayer::overriding(
                    header::STRICT_TRANSPORT_SECURITY,
                    HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
                ))
                // # Risk description
                // The HTTP header `X-Content-Type-Options` is addressed to the Internet Explorer browser
                // and prevents it from reinterpreting the content of a web page (MIME-sniffing) and
                // thus overriding the value of the Content-Type header). Lack of this header could
                // lead to attacks such as Cross-Site Scripting or phishing.
                // # Recommendation
                // We recommend setting the X-Content-Type-Options header such as `X-Content-Type-Options: nosniff`.
                .layer(SetResponseHeaderLayer::overriding(
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ))
                // # Risk description
                // Because the `X-Frame-Options` header is not sent by the server, an attacker could
                // embed this website into an iframe of a third party website. By manipulating the
                // display attributes of the iframe, the attacker could trick the user into performing
                // mouse clicks in the application, thus performing activities without user consent
                // (ex: delete user, subscribe to newsletter, etc). This is called a Clickjacking
                // attack and it is described in detail here: https://owasp.org/www-community/attacks/Clickjacking
                // # Recommendation
                // We recommend you to add the `X-Frame-Options` HTTP header with the values `DENY`
                // or `SAMEORIGIN` to every page that you want to be protected against Clickjacking
                // attacks.
                .layer(SetResponseHeaderLayer::overriding(
                    header::X_FRAME_OPTIONS,
                    HeaderValue::from_static("DENY"),
                ))
                // # Risk description
                // The `X-XSS-Protection` HTTP header instructs the browser to stop loading web pages
                // when they detect reflected Cross-Site Scripting (XSS) attacks. Lack of this header
                // exposes application users to XSS attacks in case the web application contains such
                // vulnerability.
                // # Recommendation
                // We recommend setting the X-XSS-Protection header to `X-XSS-Protection: 1; mode=block`.
                .layer(SetResponseHeaderLayer::overriding(
                    header::X_XSS_PROTECTION,
                    HeaderValue::from_static("1; mode=block"),
                ))
                .layer(CorsLayer::permissive())
                .layer(CatchPanicLayer::new()),
        );

    info!("Starting web server on http://{listen}");

    Server::bind(&listen)
        .serve(app.into_make_service())
        .await
        .expect("axum server ran into a problem")
}

#[axum::debug_handler]
async fn json(
    State(watcher_data): State<watch::Receiver<WatcherDataReceive>>,
) -> Json<WatcherDataReceive> {
    Json(watcher_data.borrow().as_ref().cloned())
}
