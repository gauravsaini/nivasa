use nivasa_websocket::RoomEventRegistry;

#[test]
fn websocket_room_registry_uses_its_configured_namespace() {
    let mut chat = RoomEventRegistry::with_namespace("/chat");
    let mut admin = RoomEventRegistry::with_namespace("/admin");

    chat.connect("client-1");
    chat.connect("client-2");
    admin.connect("client-9");

    assert_eq!(chat.namespace(), "/chat");
    assert_eq!(admin.namespace(), "/admin");

    assert!(chat.join("general", "client-1"));
    assert!(chat.join("general", "client-2"));
    assert!(admin.join("general", "client-9"));

    let mut chat_members = chat.room_members("general");
    chat_members.sort_unstable();
    assert_eq!(
        chat_members,
        vec!["client-1".to_string(), "client-2".to_string()]
    );
    assert_eq!(admin.room_members("general"), vec!["client-9".to_string()]);

    let delivered = {
        let mut room = chat.to("general");
        room.emit("notice", "hello chat")
    };

    assert_eq!(delivered, 2);
    assert_eq!(
        chat.events_for(&"client-1"),
        vec![("notice".to_string(), "hello chat".to_string())]
    );
    assert_eq!(
        chat.events_for(&"client-2"),
        vec![("notice".to_string(), "hello chat".to_string())]
    );
    assert!(admin.events_for(&"client-9").is_empty());
}
