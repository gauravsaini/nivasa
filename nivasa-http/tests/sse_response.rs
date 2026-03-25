use http::StatusCode;
use nivasa_http::{IntoResponse, NivasaResponse, Sse, SseEvent};

#[test]
fn sse_event_into_response_sets_headers_and_frames_event_body() {
    let response = SseEvent::data("ready")
        .event("message")
        .id("42")
        .retry(2_500)
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/event-stream; charset=utf-8"
    );
    assert_eq!(
        response.headers().get(http::header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    assert_eq!(
        response.body().as_bytes(),
        b"event: message\nid: 42\nretry: 2500\ndata: ready\n\n"
    );
}

#[test]
fn buffered_sse_response_frames_multiline_payloads_and_comments() {
    let response = NivasaResponse::sse([
        SseEvent::data("line one\nline two").event("update"),
        SseEvent::comment("keepalive"),
    ]);

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/event-stream; charset=utf-8"
    );
    assert_eq!(
        response.headers().get(http::header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    assert_eq!(
        response.body().as_bytes(),
        b"event: update\ndata: line one\ndata: line two\n\n: keepalive\n\n"
    );

    let response = Sse::new([SseEvent::data("alpha").data_line("beta")]).into_response();
    assert_eq!(response.body().as_bytes(), b"data: alpha\ndata: beta\n\n");
}
