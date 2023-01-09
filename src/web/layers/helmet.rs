use std::pin::Pin;

use axum::{
    body::BoxBody,
    http::HeaderValue,
    middleware::{FromFnLayer, Next},
    response::Response,
};
use futures::Future;
use hyper::{header, Body, Request};

pub fn layer<T>() -> FromFnLayer<MiddlewareFn, (), T> {
    axum::middleware::from_fn::<_, T>(middleware)
}

type MiddlewareFn = fn(
    Request<Body>,
    Next<Body>,
) -> Pin<Box<dyn Future<Output = Response<BoxBody>> + Send + 'static>>;

fn middleware(
    req: Request<Body>,
    next: Next<Body>,
) -> Pin<Box<dyn Future<Output = Response<BoxBody>> + Send + 'static>> {
    Box::pin(async move {
        let mut response = next.run(req).await;

        let headers = response.headers_mut();

        // # Risk description
        // The Content-Security-Policy (CSP) header activates a protection mechanism implemented
        // in web browsers which prevents exploitation of Cross-Site Scripting vulnerabilities
        // (XSS). If the target application is vulnerable to XSS, lack of this header makes
        // it easily exploitable by attackers.
        // # Recommendation
        // Configure the Content-Security-Header to be sent with each HTTP response in order
        // to apply the specific policies needed by the application.
        headers.append(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; img-src 'self' yt3.ggpht.com static-cdn.jtvnw.net",
            ),
        ); // TODO: report-uri

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
        headers.append(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );

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
        headers.append(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
        );

        // # Risk description
        // The HTTP header `X-Content-Type-Options` is addressed to the Internet Explorer browser
        // and prevents it from reinterpreting the content of a web page (MIME-sniffing) and
        // thus overriding the value of the Content-Type header). Lack of this header could
        // lead to attacks such as Cross-Site Scripting or phishing.
        // # Recommendation
        // We recommend setting the X-Content-Type-Options header such as `X-Content-Type-Options: nosniff`.
        headers.append(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );

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
        headers.append(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));

        // # Risk description
        // The `X-XSS-Protection` HTTP header instructs the browser to stop loading web pages
        // when they detect reflected Cross-Site Scripting (XSS) attacks. Lack of this header
        // exposes application users to XSS attacks in case the web application contains such
        // vulnerability.
        // # Recommendation
        // We recommend setting the X-XSS-Protection header to `X-XSS-Protection: 1; mode=block`.
        headers.append(
            header::X_XSS_PROTECTION,
            HeaderValue::from_static("1; mode=block"),
        );

        response
    })
}
