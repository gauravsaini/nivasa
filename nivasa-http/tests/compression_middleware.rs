#![cfg(feature = "compression-gzip")]

use flate2::read::GzDecoder;
use http::{header, HeaderValue, Method, StatusCode};
use nivasa_http::{
    Body, CompressionMiddleware, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
};
use std::io::Read;

fn decompress_gzip(bytes: &[u8]) -> String {
    let mut decoder = GzDecoder::new(bytes);
    let mut output = String::new();
    decoder
        .read_to_string(&mut output)
        .expect("gzip payload must decode");
    output
}

#[tokio::test]
async fn compression_middleware_gzips_accepted_responses() {
    let middleware = CompressionMiddleware::new();
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.path(), "/compress");
        NivasaResponse::new(StatusCode::OK, Body::text("compress me"))
    });

    let mut request = NivasaRequest::new(Method::GET, "/compress", Body::empty());
    request.set_header(header::ACCEPT_ENCODING.as_str(), "gzip");

    let response = middleware.use_(request, next).await;
    let body = response.body().as_bytes();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(decompress_gzip(&body), "compress me");
    assert_eq!(
        response.headers().get(header::CONTENT_ENCODING),
        Some(&HeaderValue::from_static("gzip"))
    );
    assert_eq!(
        response.headers().get(header::VARY),
        Some(&HeaderValue::from_static("Accept-Encoding"))
    );
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&HeaderValue::from_static("text/plain; charset=utf-8"))
    );
    assert_eq!(
        response.headers().get(header::CONTENT_LENGTH),
        Some(&HeaderValue::from_str(&body.len().to_string()).expect("length header"))
    );
}

#[tokio::test]
async fn compression_middleware_leaves_non_gzip_requests_unchanged() {
    let middleware = CompressionMiddleware::new();
    let next = NextMiddleware::new(|request: NivasaRequest| async move {
        assert_eq!(request.path(), "/plain");
        NivasaResponse::new(StatusCode::CREATED, Body::text("plain"))
    });

    let request = NivasaRequest::new(Method::GET, "/plain", Body::empty());

    let response = middleware.use_(request, next).await;

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.body(), &Body::text("plain"));
    assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
}
