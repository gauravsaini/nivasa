# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
when releases are cut.

## [Unreleased]

## [0.1.0] - 2026-05-31

Initial public release of Nivasa, a NestJS-inspired Rust web framework with
SCXML-backed lifecycle enforcement across application, module, provider, and
request flows.

### Highlights

- **17 workspace crates** covering the full framework surface
- **SCXML statechart-driven lifecycles** — every state transition is formally
  defined, code-generated, and enforced at compile time and runtime
- **95%+ line coverage** gate in CI with mutation testing on changed code
- **NestJS-compatible patterns** — controllers, modules, DI, guards,
  interceptors, pipes, filters, middleware, and more

### Added

#### Core Architecture
- `nivasa-core` — Dependency injection container with singleton, scoped, and
  transient provider lifecycles; topological initialization; circular dependency
  detection; optional and lazy dependency support.
- `nivasa-core` — Module system with metadata, imports/exports, global modules,
  dynamic modules (`for_root`/`for_feature`), and ordered lifecycle hooks
  (`OnModuleInit`, `OnModuleDestroy`, `OnApplicationBootstrap`,
  `OnApplicationShutdown`).
- `nivasa-statechart` — W3C SCXML parser, validator, build-time Rust code
  generator, runtime `StatechartEngine` with transition enforcement, tracer
  hooks, and debug introspection endpoints.

#### HTTP & Routing
- `nivasa-http` — Hyper-based HTTP server with graceful shutdown, TLS support,
  configurable request timeout and body size limits, CORS, compression (gzip,
  deflate, brotli), and structured logging.
- `nivasa-routing` — Route pattern matching (static, parameterized, optional,
  wildcard), method-aware dispatch registry, and API versioning (URI, header,
  media type).
- Controller macros (`#[controller]`, `#[get]`, `#[post]`, etc.) with full
  parameter extraction (`#[body]`, `#[param]`, `#[query]`, `#[headers]`,
  `#[ip]`, `#[session]`, `#[file]`/`#[files]`, `#[custom_param]`).
- Response types: JSON, text, HTML, streaming, SSE, file download, redirect.
- Multipart file upload with configurable size and MIME type limits.

#### Policy & Runtime Hooks
- `nivasa-guards` — Guard trait with `AuthGuard`, `RolesGuard`, and
  `ThrottlerGuard`; metadata-driven access control via `Reflector`.
- `nivasa-interceptors` — Interceptor trait with `LoggingInterceptor`,
  `TimeoutInterceptor`, `CacheInterceptor`, and `ClassSerializerInterceptor`.
- `nivasa-pipes` — Pipe trait with `ParseIntPipe`, `ParseFloatPipe`,
  `ParseBoolPipe`, `ParseUuidPipe`, `ParseEnumPipe`, `TrimPipe`,
  `DefaultValuePipe`, and `ValidationPipe`.
- `nivasa-filters` — Exception filter trait with handler → controller → global
  precedence and built-in `HttpExceptionFilter`.
- Middleware system with Tower compatibility layer, route-specific application
  with exclusion support, and built-in helmet/compression/request-id middleware.
- Rate limiting via `ThrottlerModule` with pluggable storage backends and
  per-route `#[throttle]`/`#[skip_throttle]` overrides.

#### Validation & Configuration
- `nivasa-validation` — `Validate` trait, `ValidationContext` with groups,
  structured `ValidationErrors`, and helper functions (`is_url`,
  `matches_regex`, `is_not_empty`).
- `nivasa-macros` — `#[derive(Dto)]` and `#[derive(PartialDto)]` with 18+
  validation attributes (`#[is_email]`, `#[min]`, `#[max]`, `#[min_length]`,
  `#[validate_nested]`, etc.).
- `nivasa-config` — `ConfigModule` with `.env` file loading, variable
  interpolation, `ConfigService` with typed accessors, and
  `#[derive(ConfigSchema)]` for startup validation.

#### Realtime & Integrations
- `nivasa-websocket` — WebSocket gateway traits, lifecycle hooks
  (`OnGatewayInit`, `OnGatewayConnection`, `OnGatewayDisconnect`),
  room/namespace registries, and event broadcasting.
- `nivasa-graphql` — `GraphQLModule` wrapping `async-graphql` with schema
  registration in the DI container and Apollo Federation support.
- `nivasa-scheduling` — In-memory scheduler with cron, interval, and timeout
  jobs; dynamic job registration/removal at runtime.
- Event emitter module with wildcard pattern matching.
- Health check module (`TerminusModule`) with disk, memory, HTTP, and database
  indicators.
- OpenAPI/Swagger spec generation from controller metadata with Swagger UI
  serving.

#### Developer Experience
- `nivasa-cli` — `nivasa new`, `nivasa generate` (module, controller, service,
  guard, interceptor, pipe, filter, middleware, resource), `nivasa info`, and
  `nivasa statechart` (validate, parity, visualize, diff, inspect).
- `nivasa-common` — 20+ typed HTTP exceptions with structured JSON serialization
  and cause chaining.
- Testing utilities: `TestingModule` builder with provider overrides,
  `TestClient` for in-memory HTTP dispatch, and `MockProvider` with call
  recording.
- Example applications: hello-world, crud-rest-api, auth-jwt, websocket-chat,
  config-env, and testing.

#### CI/CD & Quality
- GitHub Actions CI with check, test, clippy, fmt, docs, coverage (≥95%),
  benchmarks, mutation testing, cargo-deny, and SCXML validation/parity.
- Release workflow with gate, dry-run, and crates.io publish jobs.
- Benchmark suite comparing hello-world latency against Actix Web and Axum.
- `SECURITY.md` with responsible disclosure policy.

### Changed

- Request, provider, and module lifecycles are enforced by SCXML statecharts —
  invalid transitions are rejected at runtime (panic in debug, `Err` in release).
- The umbrella `nivasa` crate re-exports all commonly needed types via
  `nivasa::prelude::*`.

### Known Limitations

- This is a `0.1.0` release — public APIs may change in future minor versions.
- WebSocket gateway traits use blanket implementations; concrete transport
  wiring with actual TCP WebSocket connections is planned for 0.2.0.
- `AuthGuard` validates JWT structure (3 dot-separated segments) but does not
  perform cryptographic signature verification — bring your own JWT library.
- Some proc macro attributes (`#[body]`, `#[param]`, `#[req]`, etc.) are
  metadata markers that inform code generation but do not transform the item
  themselves.

[Unreleased]: https://github.com/gauravsaini/nivasa/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/gauravsaini/nivasa/releases/tag/v0.1.0
