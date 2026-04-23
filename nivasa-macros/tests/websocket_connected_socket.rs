use nivasa_macros::{controller, impl_controller};
use trybuild::TestCases;

#[controller("/ws")]
struct ChatGateway;

#[impl_controller]
impl ChatGateway {
    #[nivasa_macros::post("/messages")]
    #[allow(dead_code)]
    fn publish(#[nivasa_macros::connected_socket] client: String) {
        let _ = client;
    }
}

#[test]
fn websocket_connected_socket_emits_parameter_metadata() {
    assert_eq!(
        ChatGateway::__nivasa_controller_parameter_metadata(),
        vec![("publish", vec![("connected_socket", None)])],
    );
}

#[test]
fn websocket_connected_socket_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/websocket_connected_socket_invalid_args.rs");
}
