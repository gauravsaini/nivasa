use http::header::CONTENT_TYPE;
use http::StatusCode;
use nivasa_http::{ControllerResponse, IntoResponse};

#[test]
fn controller_response_ignores_invalid_headers_and_keeps_json_contract() {
    let mut response = ControllerResponse::new();
    response
        .status(StatusCode::ACCEPTED)
        .header("x-valid", "ok")
        .header("bad header", "ignored")
        .header("x-bad-value", "bad\nvalue")
        .json(serde_json::json!({ "ok": true }));

    let response = response.into_response();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers().get("x-valid").unwrap(), "ok");
    assert!(response.headers().get("bad header").is_none());
    assert!(response.headers().get("x-bad-value").is_none());
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response.body().as_bytes()).unwrap(),
        serde_json::json!({ "ok": true })
    );
}

#[test]
fn into_response_helpers_cover_unit_and_byte_vector_edges() {
    let empty = ().into_response();
    let bytes = vec![1_u8, 2, 3].into_response();

    assert_eq!(empty.status(), StatusCode::OK);
    assert!(empty.body().is_empty());
    assert!(empty.headers().get(CONTENT_TYPE).is_none());

    assert_eq!(bytes.status(), StatusCode::OK);
    assert_eq!(bytes.body().as_bytes(), [1_u8, 2, 3]);
    assert_eq!(
        bytes.headers().get(CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
}
