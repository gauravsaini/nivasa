# Exception Filters

This page summarizes the exception-filter surface that is already shipped today.

## What Exists

The runtime already supports:

1. `#[catch(ExceptionType)]` and `#[catch_all]` on filter structs via `nivasa-macros`.
1. Handler-level `#[use_filters(...)]` and controller-level `#[use_filters(...)]`.
1. Global HTTP filters through `NivasaServer::builder().use_global_filter(...)`.
1. The built-in `HttpExceptionFilter` adapter for the standard `HttpException` JSON shape.
1. Filter matching by exception type, with exact matches preferred over catch-all filters.
1. Filter precedence in this order: handler, controller, global.
1. A fallback path for completely unhandled exceptions that logs and returns a 500 response shape.

## How It Behaves

The public entry point is the server builder surface in [`nivasa-http/src/server.rs`](../nivasa-http/src/server.rs). The request path still goes through the SCXML-gated pipeline, and exception handling happens after the interceptor stage when a handler returns `HttpException`.

The fallback path is intentionally narrow. It does not invent new runtime behavior; it only makes sure unexpected failures still become a standard internal-server-error response.

## Proof Points

The shipped behavior is covered by focused tests:

1. [`nivasa-http/tests/global_filters.rs`](../nivasa-http/tests/global_filters.rs) proves request-aware global filters and the panic fallback path.
1. [`nivasa-http/tests/global_filter_matching.rs`](../nivasa-http/tests/global_filter_matching.rs) proves exact-vs-catch-all matching.
1. [`nivasa-http/tests/filter_precedence.rs`](../nivasa-http/tests/filter_precedence.rs) proves handler → controller → global precedence.
1. [`nivasa-http/tests/http_exception_response.rs`](../nivasa-http/tests/http_exception_response.rs) proves the built-in `HttpExceptionFilter` and the standard JSON response shape.

## Notes

Keep new work aligned with the shipped surface above. If a future change needs new filter kinds or richer runtime behavior, update the runtime and tests first, then expand this page.
