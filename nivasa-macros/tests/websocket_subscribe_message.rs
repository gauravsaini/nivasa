use nivasa_macros::{subscribe_message, websocket_gateway};
use trybuild::TestCases;

#[websocket_gateway("/ws")]
struct ChatGateway;

impl ChatGateway {
    #[subscribe_message("chat.join")]
    fn on_join(&self) {}
}

#[test]
fn subscribe_message_macro_emits_handler_metadata() {
    assert_eq!(
        ChatGateway::__nivasa_subscribe_message_metadata_for_on_join(),
        ("on_join", "chat.join")
    );
}

#[test]
fn subscribe_message_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/subscribe_message_invalid_target.rs");
    t.compile_fail("tests/trybuild/subscribe_message_invalid_args.rs");
}
