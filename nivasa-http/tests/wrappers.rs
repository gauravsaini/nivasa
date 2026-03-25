use http::header::HeaderMap;
use http::{Method, Request, StatusCode};
use nivasa_http::{
    Body, FromRequest, Html, IntoResponse, Json, NivasaRequest, NivasaResponse, Query,
    Redirect, RequestExtractError, Text,
};
use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod, RoutePathCaptures};
use serde::Deserialize;

#[test]
fn request_wrapper_exposes_basic_parts() {
    let request = NivasaRequest::new(Method::POST, "/users/42", Body::text("hello"));

    assert_eq!(request.method(), Method::POST);
    assert_eq!(request.path(), "/users/42");
    assert_eq!(request.body().as_bytes(), b"hello");
}

#[test]
fn request_extraction_supports_query_headers_and_json() {
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct SearchQuery {
        page: u32,
        active: bool,
    }

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct CreateUser {
        name: String,
    }

    let request = Request::builder()
        .method(Method::POST)
        .uri("/users?page=2&active=true")
        .header("x-request-id", "abc123")
        .body(Body::json(serde_json::json!({"name": "Ada"})))
        .expect("request must build");

    let request = NivasaRequest::from_http(request);

    assert_eq!(request.query("page"), Some("2"));
    assert_eq!(request.query("missing"), None);
    assert_eq!(request.header("x-request-id").unwrap().to_str().unwrap(), "abc123");

    let headers = HeaderMap::from_request(&request).unwrap();
    assert_eq!(headers.get("x-request-id").unwrap().to_str().unwrap(), "abc123");

    let query = Query::<SearchQuery>::from_request(&request).unwrap();
    assert_eq!(
        query.into_inner(),
        SearchQuery {
            page: 2,
            active: true,
        }
    );

    let json = Json::<CreateUser>::from_request(&request).unwrap();
    assert_eq!(
        json.into_inner(),
        CreateUser {
            name: "Ada".to_string(),
        }
    );
}

#[test]
fn request_extraction_supports_single_query_and_header_values() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/users?page=2&active=true")
        .header("x-retry-count", "3")
        .header("x-request-id", "abc123")
        .body(Body::empty())
        .expect("request must build");

    let request = NivasaRequest::from_http(request);

    assert_eq!(request.query_typed::<u32>("page").unwrap(), 2);
    assert_eq!(request.query_typed::<bool>("active").unwrap(), true);
    assert_eq!(request.header_typed::<u32>("x-retry-count").unwrap(), 3);
    assert_eq!(
        request.header_typed::<String>("x-request-id").unwrap(),
        "abc123"
    );

    assert!(matches!(
        request.query_typed::<u32>("missing"),
        Err(RequestExtractError::MissingQueryParameter { .. })
    ));
    assert!(matches!(
        request.header_typed::<u32>("missing"),
        Err(RequestExtractError::MissingHeader { .. })
    ));
}

#[test]
fn request_extraction_supports_path_parameters() {
    let request = NivasaRequest::new(Method::GET, "/users/42", Body::empty());
    let mut pipeline = nivasa_http::RequestPipeline::new(request);
    let mut routes = RouteDispatchRegistry::new();
    routes
        .register_pattern(RouteMethod::Get, "/users/:id", "show")
        .unwrap();

    pipeline.parse_request().unwrap();
    pipeline.complete_middleware().unwrap();

    let outcome = pipeline.match_route(&routes).unwrap();
    assert!(matches!(outcome, RouteDispatchOutcome::Matched(_)));

    let request = pipeline.request();
    assert_eq!(request.path_params().unwrap().get("id"), Some("42"));
    assert_eq!(request.path_param("id"), Some("42"));
    assert_eq!(request.path_param_typed::<u32>("id").unwrap(), 42);

    let captures = RoutePathCaptures::from_request(request).unwrap();
    assert_eq!(captures.get("id"), Some("42"));
    assert_eq!(captures.len(), 1);
}

#[test]
fn request_extraction_reports_missing_path_parameters() {
    let request = NivasaRequest::new(Method::GET, "/users/42", Body::empty());

    let err = request.path_param_typed::<u32>("id").unwrap_err();
    assert!(matches!(
        err,
        nivasa_http::RequestExtractError::MissingPathParameter { .. }
    ));

    let captures = RoutePathCaptures::from_request(&request).unwrap_err();
    assert!(matches!(
        captures,
        nivasa_http::RequestExtractError::MissingPathParameters
    ));
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
fn response_ergonomics_support_builder_and_result() {
    let response = NivasaResponse::builder()
        .status(StatusCode::CREATED)
        .header("x-nivasa", "ready")
        .into_response();
    let result: Result<&str, StatusCode> = Ok("done");
    let ok = result.into_response();
    let err = Err::<&str, StatusCode>(StatusCode::BAD_REQUEST).into_response();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers().get("x-nivasa").unwrap(), "ready");
    assert!(response.body().is_empty());

    assert_eq!(ok.status(), StatusCode::OK);
    assert_eq!(ok.body().as_bytes(), b"done");
    assert_eq!(err.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn into_response_supports_text_json_and_status_tuples() {
    let text = "hello".into_response();
    let json = serde_json::json!({"ok": true}).into_response();
    let tuple = (StatusCode::ACCEPTED, "queued").into_response();
    let html = Html("<strong>hello</strong>").into_response();
    let redirect = Redirect::temporary("/users").into_response();

    assert_eq!(text.status(), StatusCode::OK);
    assert_eq!(text.body().as_bytes(), b"hello");
    assert_eq!(
        text.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );

    assert_eq!(html.status(), StatusCode::OK);
    assert_eq!(html.body().as_bytes(), b"<strong>hello</strong>");
    assert_eq!(
        html.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );

    assert_eq!(
        json.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert!(String::from_utf8(json.body().as_bytes()).unwrap().contains("\"ok\":true"));

    assert_eq!(tuple.status(), StatusCode::ACCEPTED);
    assert_eq!(tuple.body().as_bytes(), b"queued");
    assert_eq!(redirect.status(), StatusCode::FOUND);
    assert_eq!(
        redirect.headers().get(http::header::LOCATION).unwrap(),
        "/users"
    );
    assert!(redirect.body().is_empty());
}

#[test]
fn explicit_text_wrapper_and_redirect_variants_work() {
    let text = Text("plain text").into_response();
    let permanent = Redirect::permanent("/docs").into_response();
    let preserve = Redirect::permanent_preserve_method("/submit").into_response();

    assert_eq!(text.status(), StatusCode::OK);
    assert_eq!(text.body().as_bytes(), b"plain text");
    assert_eq!(
        text.headers().get(http::header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );

    assert_eq!(permanent.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(
        permanent.headers().get(http::header::LOCATION).unwrap(),
        "/docs"
    );
    assert!(permanent.body().is_empty());

    assert_eq!(preserve.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        preserve.headers().get(http::header::LOCATION).unwrap(),
        "/submit"
    );
    assert!(preserve.body().is_empty());
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
