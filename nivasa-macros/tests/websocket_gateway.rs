use nivasa_macros::websocket_gateway;
use trybuild::TestCases;

#[websocket_gateway("/ws")]
struct ChatGateway;

#[websocket_gateway({ path: "/ws", namespace: "/chat" })]
struct ChatNamespaceGateway;

#[test]
fn websocket_gateway_macro_emits_gateway_metadata() {
    assert_eq!(
        ChatGateway::__nivasa_websocket_gateway_metadata(),
        ("/ws", None)
    );
    assert_eq!(
        ChatNamespaceGateway::__nivasa_websocket_gateway_metadata(),
        ("/ws", Some("/chat"))
    );
}

#[test]
fn websocket_gateway_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/websocket_gateway_invalid_target.rs");
    t.compile_fail("tests/trybuild/websocket_gateway_invalid_args.rs");
}
