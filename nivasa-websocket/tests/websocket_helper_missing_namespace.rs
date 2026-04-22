use nivasa_websocket::NamespaceRegistry;

#[test]
fn websocket_client_room_helper_leave_is_noop_for_missing_namespace() {
    let mut namespaces = NamespaceRegistry::new();
    assert!(namespaces.join("/admin", "ops", "admin-1"));

    {
        let mut client = namespaces.client("/chat", "client-1");
        assert!(!client.leave("general"));
    }

    assert!(!namespaces.has_namespace("/chat"));
    assert!(namespaces.has_namespace("/admin"));
    assert!(namespaces.contains("/admin", "ops", &"admin-1"));
}
