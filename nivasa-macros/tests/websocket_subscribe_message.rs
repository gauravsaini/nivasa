use nivasa_macros::{subscribe_message, websocket_gateway};
use trybuild::TestCases;

struct RoomGuard;

#[websocket_gateway("/ws")]
struct ChatGateway;

impl ChatGateway {
    #[nivasa_macros::guard(RoomGuard)]
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
    let gateway = ChatGateway;
    assert_eq!(gateway.on_join("general".to_string()), "joined:general");
}

#[test]
fn subscribe_message_macro_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/subscribe_message_invalid_target.rs");
    t.compile_fail("tests/trybuild/subscribe_message_invalid_args.rs");
}
