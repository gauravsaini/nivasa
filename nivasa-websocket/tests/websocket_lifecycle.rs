use nivasa_websocket::{OnGatewayConnection, OnGatewayDisconnect, OnGatewayInit, RoomEventRegistry};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct LifecycleGateway {
    initialized: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    disconnected: Arc<AtomicBool>,
}

impl OnGatewayInit for LifecycleGateway {
    fn on_gateway_init(&self) {
        self.initialized.store(true, Ordering::SeqCst);
    }
}

impl OnGatewayConnection for LifecycleGateway {
    type Client = String;

    fn on_gateway_connection(&self, client: &Self::Client) {
        assert_eq!(client, "client-1");
        self.connected.store(true, Ordering::SeqCst);
    }
}

impl OnGatewayDisconnect for LifecycleGateway {
    type Client = String;

    fn on_gateway_disconnect(&self, client: &Self::Client) {
        assert_eq!(client, "client-1");
        self.disconnected.store(true, Ordering::SeqCst);
    }
}

#[test]
fn websocket_lifecycle_connect_subscribe_message_disconnect() {
    let gateway = LifecycleGateway {
        initialized: Arc::new(AtomicBool::new(false)),
        connected: Arc::new(AtomicBool::new(false)),
        disconnected: Arc::new(AtomicBool::new(false)),
    };
    let client = String::from("client-1");
    let mut room_events = RoomEventRegistry::new();

    gateway.on_gateway_init();
    room_events.connect(client.clone());
    gateway.on_gateway_connection(&client);

    assert!(room_events.join("lobby", client.clone()));
    assert_eq!(room_events.to("lobby").emit("message", "hello"), 1);
    assert_eq!(
        room_events.events_for(&client),
        vec![("message".to_string(), "hello".to_string())]
    );

    gateway.on_gateway_disconnect(&client);
    assert!(room_events.disconnect(&client));
    assert!(room_events.room_members("lobby").is_empty());
    assert!(room_events.events_for(&client).is_empty());

    assert!(gateway.initialized.load(Ordering::SeqCst));
    assert!(gateway.connected.load(Ordering::SeqCst));
    assert!(gateway.disconnected.load(Ordering::SeqCst));
}
