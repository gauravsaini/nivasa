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
        .set_class_metadata(
            "controller",
            serde_json::Value::String("UsersController".into())
        )
        .is_none());
    assert_eq!(
        context.set_class_metadata(
            "controller",
            serde_json::Value::String("AccountsController".into())
        ),
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
fn http_status_handles_status_code_error_path_and_from_conversions() {
    let accepted_code: u16 = HttpStatus::Accepted.into();
    let gateway_timeout: http::StatusCode = HttpStatus::GatewayTimeout.into();

    assert_eq!(accepted_code, 202);
    assert_eq!(gateway_timeout, http::StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(
        HttpStatus::try_from(http::StatusCode::from_u16(777).unwrap()),
        Err(InvalidHttpStatus(777))
    );
}

#[test]
fn http_status_classification_boundaries_exclude_other_ranges() {
    let redirection = HttpStatus::MultipleChoices;
    assert!(!redirection.is_informational());
    assert!(!redirection.is_success());
    assert!(redirection.is_redirection());
    assert!(!redirection.is_client_error());
    assert!(!redirection.is_server_error());

    let client_error = HttpStatus::TooManyRequests;
    assert!(!client_error.is_informational());
    assert!(!client_error.is_success());
    assert!(!client_error.is_redirection());
    assert!(client_error.is_client_error());
    assert!(!client_error.is_server_error());

    let server_error = HttpStatus::ServiceUnavailable;
    assert!(!server_error.is_informational());
    assert!(!server_error.is_success());
    assert!(!server_error.is_redirection());
    assert!(!server_error.is_client_error());
    assert!(server_error.is_server_error());
}

#[test]
fn http_status_informational_helper_accepts_1xx() {
    assert!(HttpStatus::Continue.is_informational());
}

#[test]
fn http_status_success_helper_accepts_2xx() {
    assert!(HttpStatus::Ok.is_success());
}

#[test]
fn http_exception_uses_unknown_fallback_for_unrecognized_status() {
    let err = HttpException::new(599, "proxy exploded");

    assert_eq!(err.status_code, 599);
    assert_eq!(err.error, "Unknown Error");
    assert_eq!(err.to_string(), "599 Unknown Error: proxy exploded");
}

#[test]
fn http_status_into_exception_uses_typed_reason_phrase() {
    let err = HttpStatus::ImATeapot.into_exception("short and stout");

    assert_eq!(err.status_code, 418);
    assert_eq!(err.message, "short and stout");
    assert_eq!(err.error, "I'm a teapot");
    assert_eq!(err.to_string(), "418 I'm a teapot: short and stout");
}

#[test]
fn http_status_variants_roundtrip_through_numeric_and_http_types() {
    let variants = [
        HttpStatus::Continue,
        HttpStatus::SwitchingProtocols,
        HttpStatus::Processing,
        HttpStatus::EarlyHints,
        HttpStatus::Ok,
        HttpStatus::Created,
        HttpStatus::Accepted,
        HttpStatus::NonAuthoritativeInformation,
        HttpStatus::NoContent,
        HttpStatus::ResetContent,
        HttpStatus::PartialContent,
        HttpStatus::MultiStatus,
        HttpStatus::AlreadyReported,
        HttpStatus::ImUsed,
        HttpStatus::MultipleChoices,
        HttpStatus::MovedPermanently,
        HttpStatus::Found,
        HttpStatus::SeeOther,
        HttpStatus::NotModified,
        HttpStatus::UseProxy,
        HttpStatus::TemporaryRedirect,
        HttpStatus::PermanentRedirect,
        HttpStatus::BadRequest,
        HttpStatus::Unauthorized,
        HttpStatus::PaymentRequired,
        HttpStatus::Forbidden,
        HttpStatus::NotFound,
        HttpStatus::MethodNotAllowed,
        HttpStatus::NotAcceptable,
        HttpStatus::ProxyAuthenticationRequired,
        HttpStatus::RequestTimeout,
        HttpStatus::Conflict,
        HttpStatus::Gone,
        HttpStatus::LengthRequired,
        HttpStatus::PreconditionFailed,
        HttpStatus::PayloadTooLarge,
        HttpStatus::UriTooLong,
        HttpStatus::UnsupportedMediaType,
        HttpStatus::RangeNotSatisfiable,
        HttpStatus::ExpectationFailed,
        HttpStatus::ImATeapot,
        HttpStatus::MisdirectedRequest,
        HttpStatus::UnprocessableEntity,
        HttpStatus::Locked,
        HttpStatus::FailedDependency,
        HttpStatus::TooEarly,
        HttpStatus::UpgradeRequired,
        HttpStatus::PreconditionRequired,
        HttpStatus::TooManyRequests,
        HttpStatus::RequestHeaderFieldsTooLarge,
        HttpStatus::UnavailableForLegalReasons,
        HttpStatus::InternalServerError,
        HttpStatus::NotImplemented,
        HttpStatus::BadGateway,
        HttpStatus::ServiceUnavailable,
        HttpStatus::GatewayTimeout,
        HttpStatus::HttpVersionNotSupported,
        HttpStatus::VariantAlsoNegotiates,
        HttpStatus::InsufficientStorage,
        HttpStatus::LoopDetected,
        HttpStatus::NotExtended,
        HttpStatus::NetworkAuthenticationRequired,
    ];

    for variant in variants {
        let code = variant.as_u16();
        let status_code = variant.to_http_status_code();
        let display = variant.to_string();

        assert_eq!(u16::from(variant), code);
        assert_eq!(status_code.as_u16(), code);
        assert_eq!(HttpStatus::try_from(code), Ok(variant));
        assert_eq!(HttpStatus::try_from(status_code), Ok(variant));
        assert!(display.starts_with(&format!("{code} ")));
        assert!(display.ends_with(variant.reason_phrase()));
        assert!(!variant.reason_phrase().is_empty());
    }
}

#[test]
fn http_status_class_helpers_track_numeric_ranges_for_every_variant() {
    let variants = [
        HttpStatus::Continue,
        HttpStatus::SwitchingProtocols,
        HttpStatus::Processing,
        HttpStatus::EarlyHints,
        HttpStatus::Ok,
        HttpStatus::Created,
        HttpStatus::Accepted,
        HttpStatus::NonAuthoritativeInformation,
        HttpStatus::NoContent,
        HttpStatus::ResetContent,
        HttpStatus::PartialContent,
        HttpStatus::MultiStatus,
        HttpStatus::AlreadyReported,
        HttpStatus::ImUsed,
        HttpStatus::MultipleChoices,
        HttpStatus::MovedPermanently,
        HttpStatus::Found,
        HttpStatus::SeeOther,
        HttpStatus::NotModified,
        HttpStatus::UseProxy,
        HttpStatus::TemporaryRedirect,
        HttpStatus::PermanentRedirect,
        HttpStatus::BadRequest,
        HttpStatus::Unauthorized,
        HttpStatus::PaymentRequired,
        HttpStatus::Forbidden,
        HttpStatus::NotFound,
        HttpStatus::MethodNotAllowed,
        HttpStatus::NotAcceptable,
        HttpStatus::ProxyAuthenticationRequired,
        HttpStatus::RequestTimeout,
        HttpStatus::Conflict,
        HttpStatus::Gone,
        HttpStatus::LengthRequired,
        HttpStatus::PreconditionFailed,
        HttpStatus::PayloadTooLarge,
        HttpStatus::UriTooLong,
        HttpStatus::UnsupportedMediaType,
        HttpStatus::RangeNotSatisfiable,
        HttpStatus::ExpectationFailed,
        HttpStatus::ImATeapot,
        HttpStatus::MisdirectedRequest,
        HttpStatus::UnprocessableEntity,
        HttpStatus::Locked,
        HttpStatus::FailedDependency,
        HttpStatus::TooEarly,
        HttpStatus::UpgradeRequired,
        HttpStatus::PreconditionRequired,
        HttpStatus::TooManyRequests,
        HttpStatus::RequestHeaderFieldsTooLarge,
        HttpStatus::UnavailableForLegalReasons,
        HttpStatus::InternalServerError,
        HttpStatus::NotImplemented,
        HttpStatus::BadGateway,
        HttpStatus::ServiceUnavailable,
        HttpStatus::GatewayTimeout,
        HttpStatus::HttpVersionNotSupported,
        HttpStatus::VariantAlsoNegotiates,
        HttpStatus::InsufficientStorage,
        HttpStatus::LoopDetected,
        HttpStatus::NotExtended,
        HttpStatus::NetworkAuthenticationRequired,
    ];

    for variant in variants {
        let code = variant.as_u16();

        assert_eq!(variant.is_informational(), (100..=199).contains(&code));
        assert_eq!(variant.is_success(), (200..=299).contains(&code));
        assert_eq!(variant.is_redirection(), (300..=399).contains(&code));
        assert_eq!(variant.is_client_error(), (400..=499).contains(&code));
        assert_eq!(variant.is_server_error(), (500..=599).contains(&code));
    }
}

#[test]
fn invalid_http_status_displays_human_readable_message() {
    let err = InvalidHttpStatus(777);

    assert_eq!(err.to_string(), "unsupported standard HTTP status code: 777");
}
