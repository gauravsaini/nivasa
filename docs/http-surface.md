# nivasa-http Surface

This page summarizes the current `nivasa-http` request and response surface after the transport, extraction, server-core, `Result<HttpException>` mapping, buffered streaming helper, SSE helper, and file-download helper work landed.

## SCXML Rule

The request path must stay SCXML-driven. `NivasaServer` may adapt network I/O into framework requests, but every request still flows through `RequestPipeline` and `StatechartEngine<NivasaRequestStatechart>`. There is no direct state mutation path, and response helpers do not bypass that pipeline.

## Implemented

The crate currently exposes these pieces:

1. `Body` as the core request/response payload wrapper.
1. `NivasaRequest` with method, URI, headers, body, and route-capture access.
1. `NivasaRequest::extract::<HeaderMap>()` plus `FromRequest for HeaderMap` for full header-map access.
1. `NivasaResponse` plus `NivasaResponseBuilder`.
1. `FromRequest` for request, `HeaderMap`, body, JSON, query, and route-capture extraction.
1. `IntoResponse` for common response shapes, including `Result<T, HttpException>` so endpoint handlers can return success or HTTP error values directly and have `HttpException` serialize to the JSON error payload.
1. `StreamBody` plus `NivasaResponse::stream()` for buffered generic streaming responses.
1. `Sse` plus `NivasaResponse::sse()` for buffered server-sent events responses.
1. `Download` plus `NivasaResponse::download()` for byte-backed file attachment responses that set `Content-Disposition`.
1. Controller-side multipart helpers for `#[file]` and `#[files]` that stay on controller side without taking over request lifecycle ownership.
1. `ControllerResponse` plus `NivasaResponseBuilder` for the first `#[res]` runtime slice.
1. `apply_controller_response_metadata(...)` for applying `#[http_code(...)]` and response-header metadata after handler execution.
1. `RequestPipeline` for the SCXML request coordinator.
1. `NivasaServer` as the transport shell entry point.
1. Exception-filter runtime support via `use_global_filter(...)`, `#[use_filters(...)]`, `#[catch]`, `#[catch_all]`, and the built-in `HttpExceptionFilter`; see [`docs/exception-filters.md`](./exception-filters.md) for the shipped matching and fallback behavior.
1. Middleware composition via `RequestIdMiddleware`, `LoggerMiddleware`, `TowerServiceMiddleware`, and `NivasaMiddlewareLayer`, with a real `tower_http::cors::CorsLayer` proof in [`nivasa-http/tests/tower_middleware.rs`](../nivasa-http/tests/tower_middleware.rs) and focused middleware proofs in [`nivasa-http/tests/middleware_foundation.rs`](../nivasa-http/tests/middleware_foundation.rs) plus [`nivasa-http/tests/logger_middleware.rs`](../nivasa-http/tests/logger_middleware.rs).
1. Request dispatch for URI, header, and media-type versioned routes through the server and routing layers.
1. Focused integration tests for wrappers, controller extraction, request pipeline, and server core.

## Still Open

These pieces are still intentionally out of scope or only partially wired:

1. Full controller invocation from generated metadata.
1. `ErrorHandling` as the remaining SCXML future-stage caveat.
1. Filesystem-backed or streaming download responses, range handling, and other richer attachment behavior.

## Practical Notes

1. Keep transport code focused on I/O and request construction.
1. Keep lifecycle decisions in the SCXML pipeline.
1. Keep response helpers small and composable so later runtime wiring can build on them, and treat buffered streaming as a wrapper-layer response helper rather than transport-level streaming.
1. For middleware composition details, see [`docs/middleware.md`](./middleware.md) and the proof tests in [`nivasa-http/tests/middleware_foundation.rs`](../nivasa-http/tests/middleware_foundation.rs), [`nivasa-http/tests/logger_middleware.rs`](../nivasa-http/tests/logger_middleware.rs), and [`nivasa-http/tests/tower_middleware.rs`](../nivasa-http/tests/tower_middleware.rs).
1. Treat controller-side `#[headers]`, `#[ip]`, `#[session]`, and `#[custom_param(...)]` as focused helper-based runtime slices today; fully automatic argument binding is still future work.
1. Treat controller-side `#[file]` and `#[files]` support as post-route multipart helpers, not as a new request-pipeline stage.
1. Use the attachment helper for simple byte downloads, but do not treat it as a full download subsystem yet; it is still byte-backed rather than stream- or filesystem-backed.
