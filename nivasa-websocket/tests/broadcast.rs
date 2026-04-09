use nivasa_websocket::ServerEventRegistry;

#[test]
fn websocket_server_emit_broadcasts_to_all_connected_clients() {
    let mut registry = ServerEventRegistry::new();
    registry.connect("client-1");
    registry.connect("client-2");
    registry.connect("client-3");

    let delivered = {
        let mut server = registry.server();
        server.emit("notice", "hello")
    };

    assert_eq!(delivered, 3);
    assert_eq!(
        registry.events_for(&"client-1"),
        vec![("notice".to_string(), "hello".to_string())]
    );
    assert_eq!(
        registry.events_for(&"client-2"),
        vec![("notice".to_string(), "hello".to_string())]
    );
    assert_eq!(
        registry.events_for(&"client-3"),
        vec![("notice".to_string(), "hello".to_string())]
    );
}
