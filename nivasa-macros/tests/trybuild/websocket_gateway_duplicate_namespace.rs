use nivasa_macros::websocket_gateway;

#[websocket_gateway({ path: "/ws", namespace: "/chat", namespace: "/chat-2" })]
struct DuplicateNamespaceGateway;

fn main() {}
