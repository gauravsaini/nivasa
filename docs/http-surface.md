# nivasa-http Surface

This page summarizes the current `nivasa-http` request and response surface after the transport, extraction, and server-core work landed.

## SCXML Rule

The request path must stay SCXML-driven. `NivasaServer` may adapt network I/O into framework requests, but every request still flows through `RequestPipeline` and `StatechartEngine<NivasaRequestStatechart>`. There is no direct state mutation path.

## Implemented

The crate currently exposes these pieces:

1. `Body` as the core request/response payload wrapper.
1. `NivasaRequest` with method, URI, headers, body, and route-capture access.
1. `NivasaResponse` plus `NivasaResponseBuilder`.
1. `FromRequest` for request, headers, body, JSON, query, and route-capture extraction.
1. `IntoResponse` for common response shapes.
1. `RequestPipeline` for the SCXML request coordinator.
1. `NivasaServer` as the transport shell entry point.
1. Request dispatch for URI, header, and media-type versioned routes through the server and routing layers.
1. Focused integration tests for wrappers, controller extraction, request pipeline, and server core.

## Still Open

These pieces are still intentionally out of scope or only partially wired:

1. Full controller invocation from generated metadata.
1. Automatic runtime handling for every controller marker.
1. Request body size limits.
1. Request timeouts.
1. TLS via `rustls`.
1. The later SCXML pipeline stages beyond the current coordinator cut.
1. App-level `VersioningOptions`.

## Practical Notes

1. Keep transport code focused on I/O and request construction.
1. Keep lifecycle decisions in the SCXML pipeline.
1. Keep response helpers small and composable so later runtime wiring can build on them.
