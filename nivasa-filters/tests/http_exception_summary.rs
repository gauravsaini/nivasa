use nivasa_common::{HttpException, RequestContext};
use nivasa_filters::{http_exception_summary, ArgumentsHost, WsArgumentsHost};

#[derive(Debug, PartialEq)]
struct RequestSnapshot {
    path: &'static str,
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
