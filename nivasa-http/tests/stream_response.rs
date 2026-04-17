use http::{header::CONTENT_TYPE, StatusCode};
use nivasa_http::{Body, IntoResponse, NivasaResponse, StreamBody};

#[test]
fn nivasa_response_stream_buffers_text_chunks_and_sets_text_content_type() {
    let response = NivasaResponse::stream([Body::text("chunk-a"), Body::text("chunk-b")]);

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.body().as_bytes(), b"chunk-achunk-b");
}

#[test]
fn stream_body_allows_explicit_content_type_overrides_for_non_sse_payloads() {
    let response = StreamBody::new([Body::text("{\"id\":1}\n"), Body::text("{\"id\":2}\n")])
        .with_content_type("application/x-ndjson")
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/x-ndjson"
    );
    assert_eq!(response.body().as_bytes(), b"{\"id\":1}\n{\"id\":2}\n");
}

#[test]
fn stream_body_falls_back_to_octet_stream_for_mixed_chunk_types() {
    let response =
        StreamBody::new([Body::text("plain"), Body::html("<p>html</p>")]).into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert_eq!(response.body().as_bytes(), b"plain<p>html</p>");
}
