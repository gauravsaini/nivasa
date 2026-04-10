# Interceptors

Nivasa ships a small interceptor foundation in `nivasa-interceptors`, with runtime hooks exposed through the bootstrap and server builder surfaces.

## What Ships Today

The current public surface includes:

1. `Interceptor` as the core async interceptor trait.
1. `ExecutionContext` for request, handler, class, and custom metadata.
1. `CallHandler` for deferred handler invocation.
1. `AppBootstrapConfig::use_interceptor(...)` and `AppBootstrapConfig::use_global_interceptor(...)` as bootstrap-only facades.
1. `NivasaServerBuilder::interceptor(...)` for the transport-side onion chain.
1. `LoggingInterceptor`, `TimeoutInterceptor`, `CacheInterceptor`, and `ClassSerializerInterceptor` as built-in helpers.
1. `class_serialize(...)` for JSON projection before class-serialization filtering.

The runtime chain is ordered and composable. Repeated `.interceptor(...)` calls wrap the next handler in onion order, and the request pipeline still stays SCXML-gated.

## Built-Ins

`LoggingInterceptor` records method, path, handler, class, status, and duration.

`TimeoutInterceptor` returns `408 Request Timeout` when the next handler exceeds the configured duration.

`CacheInterceptor` memoizes successful responses in a process-local store, with optional TTL support and request-shape key resolution.

`ClassSerializerInterceptor` works on `serde_json::Value`, so you can exclude or expose fields before a response is sent.

## Usage

### Logging

```rust
use nivasa::prelude::*;

let interceptor = LoggingInterceptor::new(
    |entry| tracing::info!("{entry}"),
    |response: &NivasaResponse| response.status().as_u16().to_string(),
);

let builder = AppBootstrapConfig::default()
    .use_interceptor(interceptor);
```

### Cache

```rust
use nivasa::prelude::*;
use std::time::Duration;

let interceptor = CacheInterceptor::<NivasaResponse>::with_ttl(Duration::from_secs(60));

let builder = AppBootstrapConfig::default()
    .use_interceptor(interceptor);
```

### Class Serialization

```rust
use nivasa::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
struct Profile {
    id: u32,
    email: String,
    password: String,
}

let interceptor = ClassSerializerInterceptor::new()
    .with_excluded_fields(["password"]);

let _json = class_serialize(&Profile {
    id: 7,
    email: "dev@example.com".to_string(),
    password: "secret".to_string(),
});
```

## Module And Decorator Boundary

Interceptor metadata is already captured by `#[interceptor(...)]` on controllers and methods, but automatic module/controller runtime wiring is still incomplete. For now:

1. Use the bootstrap or server-builder interceptor hooks for live runtime composition.
1. Treat `#[interceptor(...)]` metadata as the handoff point for future decorator-driven wiring.
1. Keep request behavior SCXML-gated through `RequestPipeline`.

## Notes

1. `CacheInterceptor` only stores successful responses.
1. `TimeoutInterceptor` measures wall-clock duration around the next handler.
1. `ClassSerializerInterceptor` is JSON-object aware and does not mutate non-object payloads.
