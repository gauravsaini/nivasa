use nivasa_macros::websocket_gateway;

#[websocket_gateway({ path: "/ws", room: "/chat" })]
struct InvalidGateway;

fn main() {}
