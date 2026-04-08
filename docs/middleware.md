# Middleware

This page documents the middleware surface that is already shipped in `nivasa-http`. The runtime is still SCXML-gated: middleware can observe and transform requests, but it does not replace `RequestPipeline` or `StatechartEngine<NivasaRequestStatechart>`.

## What Ships Today

The public API currently exposes the following pieces:

1. `NivasaMiddleware` as the core async middleware trait.
1. `RequestIdMiddleware`, which propagates an incoming `X-Request-Id` or generates one when missing.
1. `LoggerMiddleware`, which logs method, path, and response status with `tracing::info!`.
1. `TowerServiceMiddleware`, which adapts a Tower `Service<NivasaRequest, Response = NivasaResponse, Error = Infallible>` into a `NivasaMiddleware`.
1. `NivasaMiddlewareLayer`, which adapts a `NivasaMiddleware` into a Tower `Layer`.
1. `AppBootstrapConfig::use_middleware(...)`, `AppBootstrapConfig::use_interceptor(...)`, and `AppBootstrapConfig::use_global_filter(...)` as bootstrap-only facades over the transport builder.
1. Module-specific middleware registration via `NivasaServerBuilder::route_with_module_middlewares(...)`.

All of these are re-exported or reachable from the umbrella crate in [`nivasa/src/lib.rs`](../nivasa/src/lib.rs), and the public API coverage in [`nivasa/tests/public_api.rs`](../nivasa/tests/public_api.rs) keeps those names honest.

## Ordering And Scope

The runtime middleware chain is ordered as:

1. Global middleware registered with `NivasaServerBuilder::middleware(...)`.
1. Module middleware attached to a route via `route_with_module_middlewares(...)`.
1. Route-specific middleware registered with `apply(...).for_routes(...)`.

Middleware exclusion is also live through `apply(...).exclude(...)`, which lets a route binding skip a middleware for exact paths.

The focused proofs in [`nivasa-http/tests/middleware_foundation.rs`](../nivasa-http/tests/middleware_foundation.rs) cover the shipped behavior:

1. Global middleware runs on every request.
1. Module middleware only runs for the routes it is bound to.
1. Middleware ordering stays `global -> module -> route`.
1. `exclude(...)` prevents a route from seeing a bound middleware.
1. Functional middleware closures are supported.
1. `RequestIdMiddleware` forwards or generates `X-Request-Id`.
1. `LoggerMiddleware` logs the request and preserves the response.

## Tower Compatibility

The adapter pair is intentionally narrow and still useful:

1. [`TowerServiceMiddleware`](../nivasa-http/src/lib.rs) wraps an existing Tower service so it behaves like a Nivasa middleware.
1. [`NivasaMiddlewareLayer`](../nivasa-http/src/lib.rs) wraps a Nivasa middleware so it can sit inside a Tower stack.

The proof test in [`nivasa-http/tests/tower_middleware.rs`](../nivasa-http/tests/tower_middleware.rs) shows the intended composition with real Tower ecosystem code:

```rust
let layered_service = NivasaMiddlewareLayer::new(PrefixMiddleware).layer(service);
let compat_service = HttpRequestCompatService::new(layered_service);
let mut cors_service = tower_http::cors::CorsLayer::permissive().layer(compat_service);
```

That test proves three things:

1. A Nivasa middleware can still run inside a Tower stack.
1. A real `tower_http::cors::CorsLayer` can wrap the compatibility path.
1. The inner Nivasa middleware still sees the request and response body unchanged except for the middleware work it performs.

## Bootstrap Boundary

The bootstrap API stays config-only. `AppBootstrapConfig::use_middleware(...)`, `use_interceptor(...)`, and `use_global_filter(...)` are thin facades over the transport builder, while `AppBootstrapConfig::enable_versioning(...)` and `global_prefix()` remain pure configuration helpers.

Module middleware is not a bootstrap-global toggle. It is attached where the route is registered, which is why the module middleware proof lives in the HTTP runtime tests instead of the bootstrap API tests.

## Practical Boundary

1. Keep lifecycle decisions in the SCXML request pipeline.
1. Use Tower adapters for middleware composition, not for bypassing request handling.
1. Treat the current bridge as a compatibility layer while richer built-in middleware lands later.
