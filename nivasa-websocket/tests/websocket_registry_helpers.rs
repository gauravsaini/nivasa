use nivasa_websocket::{
    ClientEventRegistry, NamespaceRegistry, RoomEventRegistry, RoomRegistry, ServerEventRegistry,
};

#[test]
fn websocket_client_event_handle_reports_identity_and_registry_emptiness() {
    let mut events = ClientEventRegistry::new();

    assert!(events.is_empty());

    {
        let mut client = events.client("client-1");
        assert_eq!(client.client(), &"client-1");
        assert_eq!(client.emit("message", "hello"), 1);
    }

    assert_eq!(
        events.events_for(&"client-1"),
        vec![("message".to_string(), "hello".to_string())]
    );
    assert!(!events.is_empty());
}

#[test]
fn websocket_server_registry_disconnects_and_reports_connected_clients() {
    let mut server = ServerEventRegistry::new();
    server.connect("client-1");
    server.connect("client-2");
    server.connect("client-3");

    let mut connected = server.connected_clients();
    connected.sort_unstable();
    assert_eq!(connected, vec!["client-1", "client-2", "client-3"]);

    assert!(server.disconnect(&"client-2"));
    assert!(!server.disconnect(&"client-2"));
    let mut remaining = server.connected_clients();
    remaining.sort_unstable();
    assert_eq!(remaining, vec!["client-1", "client-3"]);
    assert!(server.events_for(&"client-2").is_empty());
}

#[test]
fn websocket_namespace_registry_disconnects_every_room_for_a_client() {
    let mut namespaces = NamespaceRegistry::new();

    assert!(namespaces.join("/chat", "general", "client-1"));
    assert!(namespaces.join("/chat", "ops", "client-1"));
    assert!(namespaces.join("/admin", "general", "client-2"));
    assert!(namespaces.disconnect(&"client-1"));
    assert!(!namespaces.contains("/chat", "general", &"client-1"));
    assert!(!namespaces.contains("/chat", "ops", &"client-1"));
    assert!(!namespaces.has_namespace("/chat"));
    assert!(namespaces.has_namespace("/admin"));
    assert!(!namespaces.disconnect(&"client-1"));
}

#[test]
fn websocket_room_registry_disconnects_client_and_cleans_empty_room_state() {
    let mut rooms = RoomEventRegistry::new();

    rooms.connect("client-1");
    rooms.connect("client-2");
    assert!(rooms.join("lobby", "client-1"));
    assert!(rooms.join("lobby", "client-2"));
    assert_eq!(rooms.to("lobby").emit("notice", "hello"), 2);

    assert!(rooms.disconnect(&"client-1"));
    assert!(rooms.events_for(&"client-1").is_empty());
    assert_eq!(rooms.room_members("lobby"), vec!["client-2".to_string()]);
    assert!(!rooms.disconnect(&"client-1"));
}

#[test]
fn websocket_event_handles_return_zero_when_no_recipient_matches() {
    let mut server = ServerEventRegistry::<&'static str>::new();
    assert_eq!(server.server().emit("notice", "hello"), 0);

    let mut rooms = RoomEventRegistry::with_namespace("/chat");
    rooms.connect("client-1");
    rooms.join("general", "client-1");

    assert_eq!(rooms.to("missing").emit("notice", "hello"), 0);
    assert_eq!(
        rooms.events_for(&"client-1"),
        Vec::<(String, String)>::new()
    );
}

#[test]
fn websocket_room_registry_default_namespace_and_room_registry_defaults_are_stable() {
    let mut rooms = RoomRegistry::<&'static str>::default();

    assert_eq!(rooms.namespace(), "/");
    assert!(rooms.is_empty());
    assert_eq!(rooms.room_count(), 0);

    assert!(rooms.join("general", "client-1"));
    assert!(!rooms.is_empty());
    assert_eq!(rooms.room_count(), 1);

    assert!(rooms.leave("general", &"client-1"));
    assert!(rooms.is_empty());
    assert_eq!(rooms.room_count(), 0);

    let room_events = RoomEventRegistry::<&'static str>::default();
    assert_eq!(room_events.namespace(), "/");
}
