# Docs

Documentation root for Nivasa.

## Getting Started

- [Getting Started](./getting-started.md) - install, scaffold, and run the hello-world app shell.
- [First Steps](./first-steps.md) - build a small app from services, controllers, and modules.

## Core Architecture

- [Request Lifecycle](./request-lifecycle.md) - the SCXML-gated request flow and transition gate.
- [Server Core](./server-core.md) - transport shell, bootstrap boundary, and current TLS/runtime limits.
- [DI Container](./di-container.md) - provider registration, scopes, lazy deps, and resolution behavior.
- [Module System](./module-system.md) - module metadata, imports/exports, globals, dynamic modules, and lifecycle shape.

## HTTP Surface

- [nivasa-http Surface](./http-surface.md) - request/response wrappers, controller runtime slices, multipart helpers, streaming, and response mapping.
- [Controllers & Routing](./controllers-routing.md) - controller macros, route registry, parameter extraction, versioning, and response types.
- [Controller Extractors](./controller-extractors.md) - compile-time extractor metadata plus the current runtime extraction split.
- [Controller Response Metadata](./controller-response-metadata.md) - `#[http_code]` and `#[header]` metadata and current runtime limits.
- [API Versioning](./api-versioning.md) - versioned route config and bootstrap-time prefix helpers.

## Policy And Runtime Hooks

- [Guards](./guards.md) - guard traits, `Reflector`, `#[guard]`, `#[roles]`, and the metadata/runtime split.
- [Interceptors](./interceptors.md) - interceptor trait, built-ins, and the current transport-side hook surface.
- [Pipes](./pipes.md) - built-in pipes, `ValidationPipe`, and the current argument-transform pipeline.
- [Middleware](./middleware.md) - request middleware ordering, Tower compatibility, and bootstrap facades.
- [Exception Filters](./exception-filters.md) - shipped filters, precedence, and fallback behavior.
- [Configuration](./configuration.md) - env loading, `ConfigService`, and the current config module surface.
- [Testing](./testing.md) - container-based test patterns and the still-upcoming `TestingModule` / `TestClient` APIs.

## Realtime And Docs

- [WebSocket Support](./websocket.md) - gateway traits, metadata, room membership, and the current macro/runtime split.
- [OpenAPI / Swagger](./openapi-swagger.md) - controller metadata to OpenAPI, plus Swagger UI route serving.
