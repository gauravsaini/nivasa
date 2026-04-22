use nivasa_macros::websocket_gateway;

#[websocket_gateway({ namespace: "/chat" })]
struct MissingPathGateway;

fn main() {}
