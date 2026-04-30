use nivasa_websocket::{NamespaceRegistry, RoomEventRegistry, ServerEventRegistry};

#[test]
fn websocket_server_connect_is_idempotent_and_disconnect_drops_inbox() {
    let mut server = ServerEventRegistry::new();

    server.connect("client-1");
    server.connect("client-1");

    let delivered = {
        let mut broadcaster = server.server();
        broadcaster.emit("notice", "hello")
    };

    assert_eq!(delivered, 1);
    assert_eq!(
        server.events_for(&"client-1"),
        vec![("notice".to_string(), "hello".to_string())]
    );

    let mut connected = server.connected_clients();
    connected.sort_unstable();
    assert_eq!(connected, vec!["client-1"]);

    assert!(server.disconnect(&"client-1"));
    assert!(!server.disconnect(&"client-1"));
    assert!(server.events_for(&"client-1").is_empty());
}

#[test]
fn websocket_room_member_can_join_before_connect_and_receive_after_connect() {
    let mut rooms = RoomEventRegistry::with_namespace("/chat");

    assert!(rooms.join("general", "client-1"));
    assert!(rooms.events_for(&"client-1").is_empty());

    rooms.connect("client-1");

    let delivered = {
        let mut room = rooms.to("general");
        room.emit("notice", "welcome back")
    };

    assert_eq!(delivered, 1);
    assert_eq!(
        rooms.events_for(&"client-1"),
        vec![("notice".to_string(), "welcome back".to_string())]
    );
}

#[test]
fn websocket_namespace_members_returns_empty_for_missing_room_and_namespace() {
    let mut namespaces = NamespaceRegistry::new();

    assert!(namespaces.join("/chat", "general", "client-1"));
    assert!(namespaces.members("/chat", "missing").is_empty());
    assert!(namespaces.members("/missing", "general").is_empty());
}

#[test]
fn websocket_disconnect_returns_true_for_connected_client_without_room_membership() {
    let mut rooms = RoomEventRegistry::with_namespace("/chat");

    rooms.connect("client-1");

    assert!(rooms.disconnect(&"client-1"));
    assert!(!rooms.disconnect(&"client-1"));
    assert!(rooms.room_members("general").is_empty());
    assert!(rooms.events_for(&"client-1").is_empty());
}
