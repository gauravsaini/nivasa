use nivasa_macros::{interceptor, on_event, websocket_gateway};
use trybuild::TestCases;

struct AuditInterceptor;
struct MetricsInterceptor;

#[websocket_gateway("/events")]
struct EventGateway;

impl EventGateway {
    #[nivasa_macros::guard(RoomGuard)]
    #[interceptor(AuditInterceptor, MetricsInterceptor)]
    #[on_event("user.created")]
    fn on_user_created(&self, user_id: String) -> String {
        format!("created:{user_id}")
    }
}

struct RoomGuard;

#[test]
fn on_event_macro_emits_handler_metadata() {
    assert_eq!(
        EventGateway::__nivasa_on_event_metadata_for_on_user_created(),
        ("on_user_created", "user.created")
    );
    assert_eq!(
        EventGateway::__nivasa_on_event_guard_metadata_for_on_user_created(),
        vec!["RoomGuard"],
    );
    assert_eq!(
        EventGateway::__nivasa_on_event_interceptor_metadata_for_on_user_created(),
        vec!["AuditInterceptor", "MetricsInterceptor"],
    );

    let gateway = EventGateway;
    assert_eq!(gateway.on_user_created("42".to_string()), "created:42");
}

#[test]
fn on_event_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/on_event_invalid_target.rs");
    t.compile_fail("tests/trybuild/on_event_invalid_args.rs");
    t.compile_fail("tests/trybuild/on_event_empty_name.rs");
}
