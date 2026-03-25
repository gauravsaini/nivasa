# Server Core

This page describes the current `nivasa-http` transport shell, the app-facing bootstrap config boundary that now sits in front of it, and the runtime boundaries we are keeping explicit.

## SCXML Rule

Every request must enter the SCXML request pipeline. The transport layer may adapt network I/O into framework requests, but it must hand lifecycle control to `RequestPipeline` and let `StatechartEngine<NivasaRequestStatechart>` drive the legal transitions. There is no direct state mutation path, and the transport shell must not be treated as a second request engine.

## What Is Implemented

The current server-core surface provides:

1. `NivasaServer` and `NivasaServerBuilder` as the transport entry points.
1. `ServerOptions` and `AppBootstrapConfig` as pure app-facing configuration surfaces.
1. App-facing route registration for static, header-versioned, and media-type-versioned dispatch.
1. Transport policy knobs for request timeouts, request body size limits, and custom shutdown signals.
1. A Hyper-to-framework adapter that turns accepted connections into `NivasaRequest` values.
1. Request handoff into `RequestPipeline` so lifecycle progression stays SCXML-gated.
1. Optional TLS accept support behind the `tls` feature via `rustls` and `tokio-rustls`.
1. Smoke coverage for startup, shutdown, routing, size limits, timeouts, and TLS transport behavior.

## What Is Still Bounded

These are the important boundaries to keep in mind while the transport shell remains small:

1. The server shell is still a transport adapter, not a full application runtime.
1. `AppBootstrapConfig` is a configuration boundary, not a `NestApplication` runtime surface.
1. TLS is feature-gated and transport-scoped; it does not imply broader runtime integration.
1. The SCXML request pipeline remains the only legal place for lifecycle decisions.
1. Any request-path behavior that would bypass `RequestPipeline` is still out of bounds.

## Practical Notes

1. Keep transport code focused on I/O, request construction, and builder-level policy.
1. Keep lifecycle decisions in the SCXML pipeline.
1. Treat the server shell as an adapter, not a second request engine.
