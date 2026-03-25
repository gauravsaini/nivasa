# Server Core

This page describes the current `nivasa-http` transport shell and the scope we are intentionally keeping narrow for now.

## SCXML Rule

Every request must enter the SCXML request pipeline. The transport layer may adapt network I/O into framework requests, but it must hand lifecycle control to `RequestPipeline` and let `StatechartEngine<NivasaRequestStatechart>` drive the legal transitions. There is no direct state mutation path.

## What Is Implemented

The current server-core batch provides:

1. `NivasaServer` as the transport shell entry point.
1. A builder-based setup for starting the server.
1. Graceful shutdown support.
1. A Hyper-to-framework adapter that creates `NivasaRequest`.
1. Request handoff into `RequestPipeline` for SCXML-driven progression.
1. Basic smoke coverage for startup, shutdown, and request dispatch.

## What Is Intentionally Out Of Scope

These items are still reserved for later batches:

1. TLS via `rustls`.
1. Request body size limits.
1. Request timeouts.
1. Any direct handler execution that bypasses `RequestPipeline`.

## Practical Notes

1. Keep transport code focused on I/O and request construction.
1. Keep lifecycle decisions in the SCXML pipeline.
1. Treat the server shell as an adapter, not a second request engine.
