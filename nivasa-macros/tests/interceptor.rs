use nivasa_macros::{controller, impl_controller, interceptor};

struct AuditInterceptor;
struct TraceInterceptor;
struct MetricsInterceptor;

#[interceptor(AuditInterceptor, TraceInterceptor)]
#[controller("/events")]
struct EventController;

#[impl_controller]
impl EventController {
    #[nivasa_macros::get("/")]
    #[interceptor(MetricsInterceptor)]
    fn list(&self) {}

    #[nivasa_macros::post("/")]
    fn create(&self) {}
}

#[test]
fn interceptor_macro_emits_controller_metadata_helpers() {
    assert_eq!(
        EventController::__nivasa_controller_interceptors(),
        vec!["AuditInterceptor", "TraceInterceptor"]
    );

    assert_eq!(
        EventController::__nivasa_controller_interceptor_metadata(),
        vec![
            ("list", vec!["MetricsInterceptor"]),
            ("create", Vec::<&'static str>::new()),
        ]
    );
}
