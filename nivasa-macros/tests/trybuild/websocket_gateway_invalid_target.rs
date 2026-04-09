use nivasa_macros::websocket_gateway;

#[websocket_gateway("/ws")]
fn not_a_gateway() {}

fn main() {}
