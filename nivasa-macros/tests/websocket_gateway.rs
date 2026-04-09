use nivasa_macros::websocket_gateway;
use trybuild::TestCases;

#[websocket_gateway("/ws")]
struct ChatGateway;

#[test]
fn websocket_gateway_macro_emits_gateway_metadata() {
    assert_eq!(ChatGateway::__nivasa_websocket_gateway_metadata(), "/ws");
}

#[test]
fn websocket_gateway_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/websocket_gateway_invalid_target.rs");
}
