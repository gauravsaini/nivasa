use http::{header::CONTENT_TYPE, StatusCode};
use nivasa_http::{Body, IntoResponse, Json, Sse, SseEvent, StreamBody};
use serde::Serialize;

struct AlwaysFailSerialize;

impl Serialize for AlwaysFailSerialize {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("boom"))
    }
}

#[test]
fn json_wrapper_falls_back_when_serialization_fails() {
    let response = Json(AlwaysFailSerialize).into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get(CONTENT_TYPE).unwrap(), "application/json");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response.body().as_bytes()).unwrap(),
        serde_json::json!({
            "error": "response serialization failed"
        })
    );
}

#[test]
fn stream_body_push_skips_empty_chunks_when_inferring_content_type() {
    let response = StreamBody::new(Vec::<Body>::new())
        .push(Body::empty())
        .push(Body::text("tail"))
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.body().as_bytes(), b"tail");
}

#[test]
fn stream_body_with_only_empty_chunks_stays_empty_without_content_type() {
    let response = StreamBody::new(Vec::<Body>::new())
        .push(Body::empty())
        .push(Body::empty())
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.body().is_empty());
    assert!(response.headers().get(CONTENT_TYPE).is_none());
}

#[test]
fn sse_push_sanitizes_event_and_id_lines_and_strips_carriage_returns() {
    let response = Sse::new(Vec::<SseEvent>::new())
        .push(
            SseEvent::data("line one\r\nline two\n")
                .event("up\r\ndate")
                .id("id\r\n42")
                .retry(2_500),
        )
        .push(SseEvent::comment("ping\r\npong"))
        .into_response();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "text/event-stream; charset=utf-8"
    );
    assert_eq!(
        response.headers().get(http::header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    assert_eq!(
        response.body().as_bytes(),
        b"event: up  date\nid: id  42\nretry: 2500\ndata: line one\ndata: line two\ndata: \n\n: ping\n: pong\n\n"
    );
}
