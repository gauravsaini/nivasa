mod chat_gateway;

use chat_gateway::ChatGateway;
use nivasa_websocket::{NamespaceRegistry, RoomRegistry};

fn main() {
    let gateway = ChatGateway;
    let (path, namespace) = ChatGateway::__nivasa_websocket_gateway_metadata();

    let mut namespaces = NamespaceRegistry::new();
    let mut room = RoomRegistry::new("/chat");
    let joined = room.join("general", "client-1".to_string());
    let _ = namespaces.join(namespace.unwrap_or("/chat"), "general", "client-1".to_string());

    println!("gateway path: {path}");
    println!("gateway namespace: {}", namespace.unwrap_or("/chat"));
    println!("joined room: {joined}");
    println!("{}", gateway.join("general".to_string()));
    println!("{}", gateway.message("hello world".to_string()));
}
