# Tower Middleware Compatibility

This page documents the current Tower compatibility bridge in `nivasa-http`. The bridge is intentionally narrow: it helps you compose existing Tower ecosystem middleware with Nivasa middleware, but it does not replace the SCXML request pipeline. Requests still flow through `RequestPipeline` and `StatechartEngine<NivasaRequestStatechart>`.

## What Ships Today

The public API currently exposes two adapters:

1. [`TowerServiceMiddleware`](../nivasa-http/src/lib.rs) adapts a Tower `Service<NivasaRequest, Response = NivasaResponse, Error = Infallible>` into a `NivasaMiddleware`.
1. [`NivasaMiddlewareLayer`](../nivasa-http/src/lib.rs) adapts a `NivasaMiddleware` into a Tower `Layer`.
1. Both adapters are re-exported from the umbrella crate in [`nivasa/src/lib.rs`](../nivasa/src/lib.rs) and covered by the public API test in [`nivasa/tests/public_api.rs`](../nivasa/tests/public_api.rs).

## How To Use It

Use `TowerServiceMiddleware::new(service)` when you already have a Tower service that should behave like a Nivasa middleware.

Use `NivasaMiddlewareLayer::new(middleware).layer(service)` when you want to apply a Nivasa middleware inside a Tower stack.

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

## Practical Boundary

1. Keep Nivasa lifecycle decisions in the SCXML request pipeline.
1. Use Tower adapters for middleware composition, not for bypassing request handling.
1. Treat the current bridge as a compatibility layer while richer built-in middleware lands later.
