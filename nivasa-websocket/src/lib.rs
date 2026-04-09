//! # nivasa-websocket
//!
//! Nivasa framework — websocket.
//!
//! This crate currently exposes the bootstrap-facing websocket gateway,
//! adapter, and lifecycle trait surfaces. Gateway macros, concrete adapters,
//! rooms, and richer runtime hooks land in later slices.

/// Minimal public trait for websocket gateway types.
///
/// This stays intentionally small until the richer websocket runtime lands.
/// It gives downstream crates a concrete public abstraction to implement
/// without implying any transport, room, or message-routing behavior yet.
pub trait WebSocketGateway: Send + Sync + 'static {}

impl<T> WebSocketGateway for T where T: Send + Sync + 'static {}

/// Minimal public trait for pluggable websocket backends.
///
/// This stays as a marker-style abstraction until the concrete transport layer
/// lands. It gives downstream crates a stable public trait to reference
/// without implying any handshake, subscription, or room behavior yet.
pub trait WebSocketAdapter: Send + Sync + 'static {}

impl<T> WebSocketAdapter for T where T: Send + Sync + 'static {}

/// Hook for gateways that want a callback when the websocket runtime starts.
pub trait OnGatewayInit: Send + Sync + 'static {
    fn on_gateway_init(&self) {}
}

/// Hook for gateways that want a callback when a client connects.
pub trait OnGatewayConnection: Send + Sync + 'static {
    type Client: Send + Sync + 'static;

    fn on_gateway_connection(&self, _client: &Self::Client) {}
}

/// Hook for gateways that want a callback when a client disconnects.
pub trait OnGatewayDisconnect: Send + Sync + 'static {
    type Client: Send + Sync + 'static;

    fn on_gateway_disconnect(&self, _client: &Self::Client) {}
}

#[cfg(test)]
mod tests {
    use super::{
        OnGatewayConnection, OnGatewayDisconnect, OnGatewayInit, WebSocketAdapter,
        WebSocketGateway,
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct DemoGateway;
    struct DemoAdapter;
    struct DemoLifecycleGateway {
        initialized: Arc<AtomicBool>,
        connected: Arc<AtomicBool>,
        disconnected: Arc<AtomicBool>,
    }

    #[test]
    fn concrete_types_can_implement_the_gateway_trait() {
        fn assert_gateway<T: WebSocketGateway>() {}

        assert_gateway::<DemoGateway>();
    }

    #[test]
    fn trait_objects_can_reference_gateway_values() {
        let gateway: &(dyn WebSocketGateway + Send + Sync) = &DemoGateway;
        let _ = gateway;
    }

    #[test]
    fn concrete_types_can_implement_the_adapter_trait() {
        fn assert_adapter<T: WebSocketAdapter>() {}

        assert_adapter::<DemoAdapter>();
    }

    impl OnGatewayInit for DemoLifecycleGateway {
        fn on_gateway_init(&self) {
            self.initialized.store(true, Ordering::SeqCst);
        }
    }

    impl OnGatewayConnection for DemoLifecycleGateway {
        type Client = &'static str;

        fn on_gateway_connection(&self, client: &Self::Client) {
            assert_eq!(*client, "client-1");
            self.connected.store(true, Ordering::SeqCst);
        }
    }

    impl OnGatewayDisconnect for DemoLifecycleGateway {
        type Client = &'static str;

        fn on_gateway_disconnect(&self, client: &Self::Client) {
            assert_eq!(*client, "client-1");
            self.disconnected.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn lifecycle_traits_can_be_implemented_by_gateway_types() {
        fn assert_init<T: OnGatewayInit>() {}
        fn assert_connect<T: OnGatewayConnection<Client = &'static str>>() {}
        fn assert_disconnect<T: OnGatewayDisconnect<Client = &'static str>>() {}

        assert_init::<DemoLifecycleGateway>();
        assert_connect::<DemoLifecycleGateway>();
        assert_disconnect::<DemoLifecycleGateway>();
    }

    #[test]
    fn lifecycle_hooks_can_be_invoked_directly() {
        let gateway = DemoLifecycleGateway {
            initialized: Arc::new(AtomicBool::new(false)),
            connected: Arc::new(AtomicBool::new(false)),
            disconnected: Arc::new(AtomicBool::new(false)),
        };

        gateway.on_gateway_init();
        gateway.on_gateway_connection(&"client-1");
        gateway.on_gateway_disconnect(&"client-1");

        assert!(gateway.initialized.load(Ordering::SeqCst));
        assert!(gateway.connected.load(Ordering::SeqCst));
        assert!(gateway.disconnected.load(Ordering::SeqCst));
    }
}
