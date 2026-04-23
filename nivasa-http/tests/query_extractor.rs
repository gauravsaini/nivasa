use http::{Method, Request};
use nivasa_http::{Body, FromRequest, NivasaRequest, Query, RequestExtractError};
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct SearchFilters {
    page: u32,
    active: bool,
    term: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct OptionalFilters {
    page: Option<u32>,
    active: Option<bool>,
}

#[test]
fn full_query_extraction_deserializes_into_a_typed_dto() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/users?page=2&active=true&term=nimasa")
        .body(Body::empty())
        .expect("request must build");
    let request = NivasaRequest::from_http(request);

    let filters = Query::<SearchFilters>::from_request(&request).unwrap();

    assert_eq!(
        filters.into_inner(),
        SearchFilters {
            page: 2,
            active: true,
            term: "nimasa".to_string(),
        }
    );
}

#[test]
fn missing_query_string_still_supports_optional_struct_fields() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/users")
        .body(Body::empty())
        .expect("request must build");
    let request = NivasaRequest::from_http(request);

    let filters = Query::<OptionalFilters>::from_request(&request).unwrap();

    assert_eq!(
        filters.into_inner(),
        OptionalFilters {
            page: None,
            active: None,
        }
    );
}

#[test]
fn malformed_query_fields_still_report_deserialization_errors() {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/users?page=not-a-number&active=true")
        .body(Body::empty())
        .expect("request must build");
    let request = NivasaRequest::from_http(request);

    let err = Query::<SearchFilters>::from_request(&request).unwrap_err();

    assert!(matches!(err, RequestExtractError::InvalidQuery(_)));
    assert!(err.to_string().starts_with("invalid query string:"));
    assert!(err.to_string().contains("page"));
}

#[test]
fn full_query_extraction_decodes_values_and_keeps_last_duplicate() {
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct SearchTerm {
        name: String,
        tag: String,
    }

    let request = Request::builder()
        .method(Method::GET)
        .uri("/users?name=Alice%20Smith&tag=one&tag=two")
        .body(Body::empty())
        .expect("request must build");
    let request = NivasaRequest::from_http(request);

    let query = Query::<SearchTerm>::from_request(&request).unwrap();

    assert_eq!(
        query.into_inner(),
        SearchTerm {
            name: "Alice Smith".to_string(),
            tag: "two".to_string(),
        }
    );
}

#[test]
fn query_extraction_preserves_json_scalar_values() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct ScalarFilters {
        count: u32,
        enabled: bool,
        ratio: f64,
        name: String,
    }

    let request = Request::builder()
        .method(Method::GET)
        .uri("/users?count=3&enabled=true&ratio=1.5&name=%22Ada%22")
        .body(Body::empty())
        .expect("request must build");
    let request = NivasaRequest::from_http(request);

    let query = Query::<ScalarFilters>::from_request(&request).unwrap();

    assert_eq!(
        query.into_inner(),
        ScalarFilters {
            count: 3,
            enabled: true,
            ratio: 1.5,
            name: "Ada".to_string(),
        }
    );
}
