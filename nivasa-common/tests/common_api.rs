use nivasa_common::{HttpException, HttpStatus, InvalidHttpStatus, RequestContext};

#[derive(Debug, PartialEq, Eq)]
struct TestRequest {
    method: &'static str,
    path: &'static str,
}

#[test]
fn request_context_overwrites_previous_values() {
    let mut context = RequestContext::new();

    assert!(context.request_data::<TestRequest>().is_none());
    assert!(context.handler_metadata("roles").is_none());
    assert!(context.class_metadata("controller").is_none());
    assert!(context.custom_data("request_id").is_none());

    assert!(context
        .insert_request_data(TestRequest {
            method: "GET",
            path: "/users/1",
        })
        .is_none());

    assert_eq!(
        context.insert_request_data(TestRequest {
            method: "POST",
            path: "/users/2",
        }),
        Some(TestRequest {
            method: "GET",
            path: "/users/1",
        })
    );

    assert_eq!(
        context.request_data::<TestRequest>(),
        Some(&TestRequest {
            method: "POST",
            path: "/users/2",
        })
    );

    assert!(context
        .set_handler_metadata("roles", serde_json::Value::String("admin".into()))
        .is_none());
    assert_eq!(
        context.set_handler_metadata("roles", serde_json::Value::String("owner".into())),
        Some(serde_json::Value::String("admin".into()))
    );
    assert_eq!(
        context.handler_metadata("roles"),
        Some(&serde_json::Value::String("owner".into()))
    );

    assert!(context
        .set_class_metadata("controller", serde_json::Value::String("UsersController".into()))
        .is_none());
    assert_eq!(
        context.set_class_metadata("controller", serde_json::Value::String("AccountsController".into())),
        Some(serde_json::Value::String("UsersController".into()))
    );
    assert_eq!(
        context.class_metadata("controller"),
        Some(&serde_json::Value::String("AccountsController".into()))
    );

    assert!(context
        .set_custom_data("request_id", serde_json::Value::String("req-1".into()))
        .is_none());
    assert_eq!(
        context.set_custom_data("request_id", serde_json::Value::String("req-2".into())),
        Some(serde_json::Value::String("req-1".into()))
    );
    assert_eq!(
        context.custom_data("request_id"),
        Some(&serde_json::Value::String("req-2".into()))
    );
}

#[test]
fn http_status_reports_invalid_codes_and_known_roundtrips() {
    assert_eq!(
        HttpStatus::try_from(503).unwrap(),
        HttpStatus::ServiceUnavailable
    );
    assert_eq!(
        HttpStatus::try_from(http::StatusCode::NOT_FOUND).unwrap(),
        HttpStatus::NotFound
    );
    assert_eq!(HttpStatus::try_from(777), Err(InvalidHttpStatus(777)));
}

#[test]
fn http_exception_uses_unknown_fallback_for_unrecognized_status() {
    let err = HttpException::new(599, "proxy exploded");

    assert_eq!(err.status_code, 599);
    assert_eq!(err.error, "Unknown Error");
    assert_eq!(err.to_string(), "599 Unknown Error: proxy exploded");
}

