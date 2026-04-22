use nivasa_websocket::{ClientEventRegistry, NamespaceRegistry, RoomEventRegistry};

#[test]
fn websocket_room_broadcast_skips_room_members_without_connected_inboxes() {
    let mut rooms = RoomEventRegistry::with_namespace("/chat");

    rooms.connect("client-1");
    assert!(rooms.join("general", "client-1"));
    assert!(rooms.join("general", "ghost"));

    let delivered = {
        let mut room = rooms.to("general");
        room.emit("notice", "hello")
    };

    assert_eq!(delivered, 1);
    assert_eq!(
        rooms.events_for(&"client-1"),
        vec![("notice".to_string(), "hello".to_string())]
    );
    assert!(rooms.events_for(&"ghost").is_empty());
    let mut members = rooms.room_members("general");
    members.sort_unstable();
    assert_eq!(members, vec!["client-1".to_string(), "ghost".to_string()]);
}

#[test]
fn websocket_room_disconnect_cleans_membership_even_without_connected_client() {
    let mut rooms = RoomEventRegistry::with_namespace("/chat");

    assert!(rooms.join("general", "ghost"));
    assert!(rooms.disconnect(&"ghost"));
    assert!(!rooms.disconnect(&"ghost"));
    assert!(rooms.room_members("general").is_empty());
    assert!(rooms.events_for(&"ghost").is_empty());
}

#[test]
fn websocket_client_event_registry_creates_inbox_on_first_emit_for_disconnected_client() {
    let mut events = ClientEventRegistry::new();

    {
        let mut client = events.client("client-9");
        assert_eq!(client.emit("notice", "offline"), 1);
    }

    assert_eq!(
        events.events_for(&"client-9"),
        vec![("notice".to_string(), "offline".to_string())]
    );
    assert!(!events.is_empty());
}

#[test]
fn websocket_namespace_disconnect_keeps_namespace_when_other_members_remain() {
    let mut namespaces = NamespaceRegistry::new();

    assert!(namespaces.join("/chat", "general", "client-1"));
    assert!(namespaces.join("/chat", "general", "client-2"));

    assert!(namespaces.disconnect(&"client-1"));
    assert!(namespaces.has_namespace("/chat"));
    assert!(!namespaces.contains("/chat", "general", &"client-1"));
    assert!(namespaces.contains("/chat", "general", &"client-2"));
    assert!(!namespaces.disconnect(&"ghost"));
}
