# WebSocket Support

This page documents the websocket surface that is already landed in `nivasa-websocket` and the matching macro metadata surface in `nivasa-macros`.

## SCXML Rule

Keep websocket work aligned with the SCXML-backed request lifecycle. Gateway setup, handler metadata, room membership, and adapter behavior are framework surfaces, but they should not bypass the request pipeline or invent a separate lifecycle model.

## Implemented Surface

`nivasa-websocket` currently ships these building blocks:

1. `WebSocketGateway` as the public gateway marker trait.
1. `WebSocketAdapter` as the public pluggable backend trait.
1. `DefaultWebSocketAdapter` backed by `tokio-tungstenite`.
1. `OnGatewayInit`, `OnGatewayConnection`, and `OnGatewayDisconnect` lifecycle hooks.
1. `RoomRegistry` for single-namespace room membership.
1. `NamespaceRegistry` for namespace-scoped room membership.
1. `ClientEventRegistry` for `client.emit(...)` style event recording.
1. `ServerEventRegistry` for `server.emit(...)` style broadcast recording.
1. `RoomEventRegistry` for `server.to("room").emit(...)` style broadcast recording.

The registry types are intentionally in-memory and test-friendly. They model the behavior surface the framework wants, without pretending to be a full transport runtime.

## Macro Surface

The websocket macro surface currently lives in `nivasa-macros`:

1. `#[websocket_gateway("/ws")]`
1. `#[websocket_gateway({ path: "/ws", namespace: "/chat" })]`
1. `#[subscribe_message("event_name")]`
1. `#[message_body]`
1. `#[connected_socket]`

The shipped metadata helpers also support gateway-method guard and interceptor capture:

1. `#[guard(...)]` on websocket handler methods records guard metadata.
1. `#[interceptor(...)]` on websocket handler methods records interceptor metadata.

That means the macro layer can describe websocket routes and handler policy today, while the deeper runtime wiring remains a later step.

## What Is Landed Today

The current tests prove these slices:

1. Gateway trait and adapter trait implementations compile.
1. The default adapter reports the `tokio-tungstenite` server role.
1. Client, server, and room registries track emitted events.
1. Room membership respects both namespace and room boundaries.
1. Disconnect cleanup removes empty rooms and empty namespaces.
1. `#[subscribe_message]` exposes handler metadata and the recorded guard/interceptor metadata helpers.

See:

- [`/Users/ektasaini/Desktop/nivasa/nivasa-websocket/src/lib.rs`](/Users/ektasaini/Desktop/nivasa/nivasa-websocket/src/lib.rs)
- [`/Users/ektasaini/Desktop/nivasa/nivasa-macros/src/subscribe_message.rs`](/Users/ektasaini/Desktop/nivasa/nivasa-macros/src/subscribe_message.rs)
- [`/Users/ektasaini/Desktop/nivasa/nivasa-macros/tests/websocket_subscribe_message.rs`](/Users/ektasaini/Desktop/nivasa/nivasa-macros/tests/websocket_subscribe_message.rs)

## Practical Notes

1. Use namespaces when you need isolated websocket domains.
1. Use rooms for targeted fan-out inside one namespace.
1. Use the default adapter only as the current transport shell.
1. Treat handler guard/interceptor metadata as a contract surface, not a promise of full runtime execution yet.
