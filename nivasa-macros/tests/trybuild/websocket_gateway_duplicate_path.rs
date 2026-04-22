use nivasa_macros::websocket_gateway;

#[websocket_gateway({ path: "/ws", path: "/chat" })]
struct DuplicatePathGateway;

fn main() {}
