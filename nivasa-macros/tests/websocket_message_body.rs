use nivasa_macros::{controller, impl_controller};
use trybuild::TestCases;

#[controller("/ws")]
struct ChatGateway;

#[impl_controller]
impl ChatGateway {
    #[nivasa_macros::post("/messages")]
    fn publish(
        #[nivasa_macros::pipe(TrimPipe)]
        #[nivasa_macros::message_body("payload")] payload: String,
        #[nivasa_macros::message_body] raw: String,
    ) {
        let _ = (payload, raw);
    }
}

#[test]
fn websocket_message_body_emits_parameter_metadata() {
    assert_eq!(
        ChatGateway::__nivasa_controller_parameter_metadata(),
        vec![(
            "publish",
            vec![
                ("message_body", Some("payload")),
                ("message_body", None),
            ],
        )],
    );
    assert_eq!(
        ChatGateway::__nivasa_controller_parameter_pipe_metadata(),
        vec![
            (
                "publish",
                vec![vec!["TrimPipe"], vec![]],
            )
        ],
    );
}

#[test]
fn websocket_message_body_validation() {
    let t = TestCases::new();
    t.compile_fail("tests/trybuild/websocket_message_body_namevalue_invalid.rs");
}
