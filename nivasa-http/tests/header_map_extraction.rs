use http::{Method, Request};
use nivasa_http::{Body, HeaderMap, NivasaRequest};

#[test]
fn request_extracts_the_full_header_map_through_the_public_api() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/articles/42")
        .header("x-request-id", "abc123")
        .header("x-trace-id", "trace-1")
        .header("set-cookie", "session=one")
        .header("set-cookie", "theme=dark")
        .body(Body::empty())
        .expect("request must build");

    let request = NivasaRequest::from_http(request);

    let headers = request
        .extract::<HeaderMap>()
        .expect("header map must extract");

    assert_eq!(
        headers.get("x-request-id").unwrap().to_str().unwrap(),
        "abc123"
    );
    assert_eq!(
        headers.get("x-trace-id").unwrap().to_str().unwrap(),
        "trace-1"
    );

    let cookie_values: Vec<_> = headers
        .get_all("set-cookie")
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect();
    assert_eq!(cookie_values, vec!["session=one", "theme=dark"]);

    assert_eq!(
        request
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap(),
        "abc123"
    );
}
