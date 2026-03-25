use http::{Method, StatusCode};
use nivasa_http::{Body, IntoResponse, NivasaRequest, NivasaResponse};

#[test]
fn request_wrapper_exposes_basic_parts() {
    let request = NivasaRequest::new(Method::POST, "/users/42", Body::text("hello"));

    assert_eq!(request.method(), Method::POST);
    assert_eq!(request.path(), "/users/42");
    assert_eq!(request.body().as_bytes(), b"hello");
}

#[test]
fn response_builder_sets_defaults_and_headers() {
    let response = NivasaResponse::builder()
        .status(StatusCode::CREATED)
        .header("x-nivasa", "ready")
        .body("created");

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers().get("x-nivasa").unwrap(), "ready");
    assert_eq!(
        response.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.body().as_bytes(), b"created");
}

#[test]
fn into_response_supports_text_json_and_status_tuples() {
    let text = "hello".into_response();
    let json = serde_json::json!({"ok": true}).into_response();
    let tuple = (StatusCode::ACCEPTED, "queued").into_response();

    assert_eq!(text.status(), StatusCode::OK);
    assert_eq!(text.body().as_bytes(), b"hello");
    assert_eq!(
        text.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );

    assert_eq!(
        json.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert!(String::from_utf8(json.body().as_bytes()).unwrap().contains("\"ok\":true"));

    assert_eq!(tuple.status(), StatusCode::ACCEPTED);
    assert_eq!(tuple.body().as_bytes(), b"queued");
}

#[test]
fn body_converts_between_empty_text_json_and_bytes() {
    assert!(Body::empty().is_empty());
    assert_eq!(Body::from("abc").into_bytes(), b"abc");
    assert_eq!(Body::from(vec![1, 2, 3]).into_bytes(), vec![1, 2, 3]);
    assert_eq!(
        Body::from(serde_json::json!({"answer": 42})).into_bytes(),
        br#"{"answer":42}"#.to_vec()
    );
}
