//! # nivasa-websocket
//!
//! Nivasa framework — websocket.
//!
//! This crate currently exposes the bootstrap-facing `WebSocketGateway` trait.
//! Gateway macros, adapters, rooms, and lifecycle hooks land in later slices.

/// Minimal public trait for websocket gateway types.
///
/// This stays intentionally small until the richer websocket runtime lands.
/// It gives downstream crates a concrete public abstraction to implement
/// without implying any transport, room, or message-routing behavior yet.
pub trait WebSocketGateway: Send + Sync + 'static {}

impl<T> WebSocketGateway for T where T: Send + Sync + 'static {}

#[cfg(test)]
mod tests {
    use super::WebSocketGateway;

    struct DemoGateway;

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
}
