use nivasa_macros::{interceptor, on_event, websocket_gateway};
use trybuild::TestCases;

#[allow(dead_code)]
struct AuditInterceptor;
#[allow(dead_code)]
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

#[allow(dead_code)]
struct RoomGuard;

#[websocket_gateway("/direct-events")]
struct DirectAttrEventGateway;

impl DirectAttrEventGateway {
    #[on_event("user.direct")]
    #[nivasa_macros::guard(RoomGuard)]
    #[interceptor(AuditInterceptor)]
    fn on_user_direct(&self) {}
}

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

    assert_eq!(
        DirectAttrEventGateway::__nivasa_on_event_guard_metadata_for_on_user_direct(),
        vec!["RoomGuard"],
    );
    assert_eq!(
        DirectAttrEventGateway::__nivasa_on_event_interceptor_metadata_for_on_user_direct(),
        vec!["AuditInterceptor"],
    );
    DirectAttrEventGateway.on_user_direct();
}

#[allow(dead_code)]
struct DocMarkerGuard;
#[allow(dead_code)]
struct DocMarkerInterceptorA;
#[allow(dead_code)]
struct DocMarkerInterceptorB;

#[websocket_gateway("/doc-events")]
struct DocMarkerEventGateway;

impl DocMarkerEventGateway {
    /// nivasa-guard: DocMarkerGuard
    /// nivasa-interceptor: DocMarkerInterceptorA, DocMarkerInterceptorB
    #[on_event("user.deleted")]
    fn on_user_deleted(&self, user_id: String) -> String {
        format!("deleted:{user_id}")
    }
}

#[test]
fn on_event_macro_parses_doc_markers() {
    assert_eq!(
        DocMarkerEventGateway::__nivasa_on_event_guard_metadata_for_on_user_deleted(),
        vec!["DocMarkerGuard"],
    );
    assert_eq!(
        DocMarkerEventGateway::__nivasa_on_event_interceptor_metadata_for_on_user_deleted(),
        vec!["DocMarkerInterceptorA", "DocMarkerInterceptorB"],
    );

    let gateway = DocMarkerEventGateway;
    assert_eq!(gateway.on_user_deleted("7".to_string()), "deleted:7");
}

#[test]
fn on_event_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/on_event_invalid_target.rs");
    t.compile_fail("tests/trybuild/on_event_invalid_args.rs");
    t.compile_fail("tests/trybuild/on_event_empty_name.rs");
    t.compile_fail("tests/trybuild/on_event_guard_requires_arg.rs");
    t.compile_fail("tests/trybuild/on_event_interceptor_requires_arg.rs");
    t.compile_fail("tests/trybuild/on_event_invalid_guard_marker.rs");
    t.compile_fail("tests/trybuild/on_event_invalid_interceptor_marker.rs");
}
