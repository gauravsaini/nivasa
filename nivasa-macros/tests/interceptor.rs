use nivasa_http::NivasaRequest;
use nivasa_macros::{controller, impl_controller, interceptor};

struct AuditInterceptor;
struct TraceInterceptor;
struct MetricsInterceptor;

#[interceptor(AuditInterceptor, TraceInterceptor)]
#[controller("/events")]
struct EventController;

#[impl_controller]
impl EventController {
    #[allow(dead_code)]
    #[nivasa_macros::get("/")]
    #[interceptor(MetricsInterceptor)]
    fn list(&self) -> &'static str {
        "listed"
    }

    #[allow(dead_code)]
    #[nivasa_macros::post("/")]
    fn create(&self) -> &'static str {
        "created"
    }

    #[allow(dead_code)]
    #[nivasa_macros::get("/request-aware")]
    fn request_aware(&self, request: &NivasaRequest) -> String {
        request.path().to_string()
    }
}

#[test]
fn interceptor_macro_emits_controller_metadata_helpers() {
    let _ = (AuditInterceptor, TraceInterceptor, MetricsInterceptor);
    let controller = EventController;
    let _ = controller;

    assert_eq!(
        EventController::__nivasa_controller_interceptors(),
        vec!["AuditInterceptor", "TraceInterceptor"]
    );

    assert_eq!(
        EventController::__nivasa_controller_interceptor_metadata(),
        vec![
            ("list", vec!["MetricsInterceptor"]),
            ("create", Vec::<&'static str>::new()),
            ("request_aware", Vec::<&'static str>::new()),
        ]
    );
}

#[test]
fn interceptor_macro_registers_dispatch_handlers_for_supported_signatures() {
    assert_eq!(
        EventController::__nivasa_controller_routes(),
        vec![
            ("GET", "/events".to_string(), "list"),
            ("POST", "/events".to_string(), "create"),
            ("GET", "/events/request-aware".to_string(), "request_aware"),
        ]
    );

    assert!(nivasa_http::resolve_controller_route_handler("/events", "list").is_some());
    assert!(nivasa_http::resolve_controller_route_handler("/events", "create").is_some());
    assert!(
        nivasa_http::resolve_controller_route_handler("/events/request-aware", "request_aware")
            .is_some()
    );
}
