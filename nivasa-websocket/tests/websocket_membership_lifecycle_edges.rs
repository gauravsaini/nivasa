use nivasa_websocket::{NamespaceRegistry, RoomEventRegistry};

#[test]
fn websocket_client_room_helper_cleans_namespace_only_after_last_room_leaves() {
    let mut namespaces = NamespaceRegistry::new();

    assert!(namespaces.join("/admin", "ops", "admin-1"));

    {
        let mut client = namespaces.client("/chat", "client-1");

        assert_eq!(client.namespace(), "/chat");
        assert_eq!(client.client(), &"client-1");
        assert!(client.join("general"));
        assert!(client.join("ops"));
        assert!(!client.join("general"));
        assert!(!client.leave("missing"));
        assert!(client.leave("general"));
    }

    assert!(namespaces.has_namespace("/chat"));
    assert!(namespaces.has_namespace("/admin"));
    assert!(!namespaces.contains("/chat", "general", &"client-1"));
    assert!(namespaces.contains("/chat", "ops", &"client-1"));

    {
        let mut client = namespaces.client("/chat", "client-1");
        assert!(client.leave("ops"));
        assert!(!client.leave("ops"));
    }

    assert!(!namespaces.has_namespace("/chat"));
    assert!(namespaces.has_namespace("/admin"));
    assert!(namespaces.contains("/admin", "ops", &"admin-1"));
}

#[test]
fn websocket_room_registry_reconnect_starts_with_clean_room_and_inbox_state() {
    let mut rooms = RoomEventRegistry::with_namespace("/chat");

    rooms.connect("client-1");
    assert!(rooms.join("lobby", "client-1"));
    assert_eq!(rooms.to("lobby").emit("message", "before disconnect"), 1);
    assert_eq!(
        rooms.events_for(&"client-1"),
        vec![("message".to_string(), "before disconnect".to_string())]
    );

    assert!(rooms.disconnect(&"client-1"));
    assert!(!rooms.disconnect(&"client-1"));
    assert!(rooms.room_members("lobby").is_empty());
    assert!(rooms.events_for(&"client-1").is_empty());

    rooms.connect("client-1");
    assert!(rooms.join("lobby", "client-1"));
    assert_eq!(rooms.to("lobby").emit("message", "after reconnect"), 1);
    assert_eq!(
        rooms.events_for(&"client-1"),
        vec![("message".to_string(), "after reconnect".to_string())]
    );
}
