use nivasa_macros::{subscribe_message, websocket_gateway};

#[websocket_gateway({ path: "/ws", namespace: "/chat" })]
pub struct ChatGateway;

impl ChatGateway {
    #[subscribe_message("chat.join")]
    pub fn join(&self, room: String) -> String {
        format!("joined:{room}")
    }

    #[subscribe_message("chat.message")]
    pub fn message(&self, message: String) -> String {
        format!("message:{message}")
    }
}
