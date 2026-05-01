# Nivasa

<p align="center">
  <img src="logo.svg" alt="Nivasa Logo" width="200" />
</p>

Welcome to the **Nivasa** project!

## Getting Started

- [Getting Started](./docs/getting-started.md) - install, scaffold, and run the hello-world app shell.
- [First Steps](./docs/first-steps.md) - build a small app from services, controllers, and modules.

## Core Architecture

- [Request Lifecycle](./docs/request-lifecycle.md) - the SCXML-gated request flow and transition gate.
- [Server Core](./docs/server-core.md) - transport shell, bootstrap boundary, and current TLS/runtime limits.
- [DI Container](./docs/di-container.md) - provider registration, scopes, lazy deps, and resolution behavior.
- [Module System](./docs/module-system.md) - module metadata, imports/exports, globals, dynamic modules, and lifecycle shape.

## HTTP Surface

- [nivasa-http Surface](./docs/http-surface.md) - request/response wrappers, controller runtime slices, multipart helpers, streaming, and response mapping.
- [Controllers & Routing](./docs/controllers-routing.md) - controller macros, route registry, parameter extraction, versioning, and response types.
- [Controller Extractors](./docs/controller-extractors.md) - compile-time extractor metadata plus the current runtime extraction split.
- [Controller Response Metadata](./docs/controller-response-metadata.md) - `#[http_code]` and `#[header]` metadata and current runtime limits.
- [API Versioning](./docs/api-versioning.md) - versioned route config and bootstrap-time prefix helpers.

## Policy And Runtime Hooks

- [Guards](./docs/guards.md) - guard traits, `Reflector`, `#[guard]`, `#[roles]`, and the metadata/runtime split.
- [Interceptors](./docs/interceptors.md) - interceptor trait, built-ins, and the current transport-side hook surface.
- [Pipes](./docs/pipes.md) - built-in pipes, `ValidationPipe`, and the current argument-transform pipeline.
- [Middleware](./docs/middleware.md) - request middleware ordering, Tower compatibility, and bootstrap facades.
- [Exception Filters](./docs/exception-filters.md) - shipped filters, precedence, and fallback behavior.
- [Configuration](./docs/configuration.md) - env loading, `ConfigService`, and the current config module surface.
- [Testing](./docs/testing.md) - container-based test patterns and the still-upcoming `TestingModule` / `TestClient` APIs.

## Realtime And Docs

- [WebSocket Support](./docs/websocket.md) - gateway traits, metadata, room membership, and the current macro/runtime split.
- [OpenAPI / Swagger](./docs/openapi-swagger.md) - controller metadata to OpenAPI, plus Swagger UI route serving.
- [CLI](./docs/cli.md) - `info`, project scaffolding, generators, and SCXML tooling.
- [Migration from NestJS](./docs/migration-from-nestjs.md) - current Nivasa equivalents and known gaps.
- [Comparison with other Rust Frameworks](./docs/comparison-rust-frameworks.md) - high-level tradeoffs versus Axum, Actix Web, Rocket, and others.
- [Publishing Order](./docs/publishing-order.md) - crate release order and release-prep notes.
