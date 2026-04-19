use nivasa_common::{HttpException, RequestContext};
use nivasa_filters::{
    http_exception_summary, ArgumentsHost, ExceptionFilterMetadata, WsArgumentsHost,
};

#[derive(Debug, PartialEq)]
struct RequestSnapshot {
    path: &'static str,
}

#[derive(Default)]
struct NamedFilter;

impl ExceptionFilterMetadata for NamedFilter {
    fn exception_type(&self) -> Option<&'static str> {
        Some("ValidationError")
    }

    fn is_catch_all(&self) -> bool {
        true
    }
}

#[test]
fn arguments_host_exposes_typed_request_data() {
    let mut request_context = RequestContext::new();
    request_context.insert_request_data(RequestSnapshot { path: "/filters" });

    let host = ArgumentsHost::new().with_request_context(request_context);

    assert_eq!(
        host.request::<RequestSnapshot>(),
        Some(&RequestSnapshot { path: "/filters" })
    );
}

#[test]
fn arguments_host_debug_reflects_presence_of_request_context() {
    let empty = format!("{:?}", ArgumentsHost::new());
    assert_eq!(empty, "ArgumentsHost { has_request_context: false }");

    let mut request_context = RequestContext::new();
    request_context.insert_request_data(RequestSnapshot { path: "/debug" });
    let host = ArgumentsHost::new().with_request_context(request_context);

    let populated = format!("{:?}", host);
    assert_eq!(populated, "ArgumentsHost { has_request_context: true }");
}

#[test]
fn arguments_host_returns_none_without_request_context() {
    let host = ArgumentsHost::new();

    assert!(host.request_context().is_none());
    assert!(host.request::<RequestSnapshot>().is_none());
}

#[test]
fn ws_arguments_host_alias_exposes_typed_request_data() {
    let mut request_context = RequestContext::new();
    request_context.insert_request_data(RequestSnapshot {
        path: "/filters/ws",
    });

    let host: WsArgumentsHost = ArgumentsHost::new().with_request_context(request_context);

    assert_eq!(
        host.request::<RequestSnapshot>(),
        Some(&RequestSnapshot {
            path: "/filters/ws"
        })
    );
}

#[test]
fn exception_filter_metadata_defaults_to_non_catch_all() {
    struct DefaultFilter;

    impl ExceptionFilterMetadata for DefaultFilter {}

    let filter = DefaultFilter;

    assert_eq!(filter.exception_type(), None);
    assert!(!filter.is_catch_all());
}

#[test]
fn exception_filter_metadata_customizes_exception_type_and_catch_all() {
    let filter = NamedFilter;

    assert_eq!(filter.exception_type(), Some("ValidationError"));
    assert!(filter.is_catch_all());
}

#[test]
fn http_exception_summary_preserves_the_default_http_error_shape() {
    let summary = http_exception_summary(&HttpException::unprocessable_entity("Validation failed"));

    assert_eq!(summary.status_code, 422);
    assert_eq!(summary.error, "Unprocessable Entity");
    assert_eq!(summary.message, "Validation failed");
    assert_eq!(
        summary.to_string(),
        "422 Unprocessable Entity: Validation failed"
    );
}

#[test]
fn http_exception_summary_formats_bad_request_shape() {
    let summary = http_exception_summary(&HttpException::bad_request("missing payload"));

    assert_eq!(summary.status_code, 400);
    assert_eq!(summary.error, "Bad Request");
    assert_eq!(summary.message, "missing payload");
    assert_eq!(summary.to_string(), "400 Bad Request: missing payload");
}
