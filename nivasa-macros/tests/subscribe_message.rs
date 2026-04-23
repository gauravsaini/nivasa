use nivasa_macros::{interceptor, subscribe_message, websocket_gateway};
use trybuild::TestCases;

#[allow(dead_code)]
struct RoomGuard;
#[allow(dead_code)]
struct AuditInterceptor;
#[allow(dead_code)]
struct MetricsInterceptor;

#[websocket_gateway("/ws")]
struct ChatGateway;

impl ChatGateway {
    #[nivasa_macros::guard(RoomGuard)]
    #[interceptor(AuditInterceptor, MetricsInterceptor)]
    #[subscribe_message("chat.join")]
    fn on_join(&self, room: String) -> String {
        format!("joined:{room}")
    }
}

#[test]
fn subscribe_message_macro_emits_handler_metadata() {
    assert_eq!(
        ChatGateway::__nivasa_subscribe_message_metadata_for_on_join(),
        ("on_join", "chat.join")
    );
    assert_eq!(
        ChatGateway::__nivasa_subscribe_message_guard_metadata_for_on_join(),
        vec!["RoomGuard"],
    );
    assert_eq!(
        ChatGateway::__nivasa_subscribe_message_interceptor_metadata_for_on_join(),
        vec!["AuditInterceptor", "MetricsInterceptor"],
    );
    let gateway = ChatGateway;
    assert_eq!(gateway.on_join("general".to_string()), "joined:general");
}

#[allow(dead_code)]
struct DocMarkerGuard;
#[allow(dead_code)]
struct DocMarkerInterceptorA;
#[allow(dead_code)]
struct DocMarkerInterceptorB;

#[websocket_gateway("/doc-ws")]
struct DocMarkerGateway;

impl DocMarkerGateway {
    /// nivasa-guard: DocMarkerGuard
    /// nivasa-interceptor: DocMarkerInterceptorA, DocMarkerInterceptorB
    #[subscribe_message("chat.leave")]
    fn on_leave(&self, room: String) -> String {
        format!("left:{room}")
    }
}

#[test]
fn subscribe_message_macro_parses_doc_markers() {
    assert_eq!(
        DocMarkerGateway::__nivasa_subscribe_message_guard_metadata_for_on_leave(),
        vec!["DocMarkerGuard"],
    );
    assert_eq!(
        DocMarkerGateway::__nivasa_subscribe_message_interceptor_metadata_for_on_leave(),
        vec!["DocMarkerInterceptorA", "DocMarkerInterceptorB"],
    );
    let gateway = DocMarkerGateway;
    assert_eq!(gateway.on_leave("general".to_string()), "left:general");
}

#[test]
fn subscribe_message_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/subscribe_message_invalid_target.rs");
    t.compile_fail("tests/trybuild/subscribe_message_invalid_args.rs");
    t.compile_fail("tests/trybuild/subscribe_message_empty_name.rs");
    t.compile_fail("tests/trybuild/subscribe_message_guard_requires_arg.rs");
    t.compile_fail("tests/trybuild/subscribe_message_interceptor_requires_arg.rs");
    t.compile_fail("tests/trybuild/subscribe_message_invalid_guard_marker.rs");
    t.compile_fail("tests/trybuild/subscribe_message_invalid_interceptor_marker.rs");
}
