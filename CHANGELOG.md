# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
when releases are cut.

## [Unreleased]

## [0.1.0] - 2026-05-01

Initial public release of Nivasa, a NestJS-inspired Rust web framework with
SCXML-backed lifecycle enforcement across application, module, provider, and
request flows.

### Added

- Workspace crates for the umbrella API, core DI/module runtime, statecharts,
  macros, HTTP, routing, guards, interceptors, pipes, filters, validation,
  configuration, WebSocket, GraphQL, scheduling, common types, and CLI tooling.
- SCXML-backed request, module, provider, and application lifecycle enforcement
  with parser, validator, build-time code generation, runtime transition gate,
  tracer hooks, and parity checks.
- Dependency injection and module composition with provider scopes, lazy and
  optional dependencies, dynamic/global modules, imports/exports, lifecycle
  hooks, and testing helpers.
- Controller, routing, extractor, response metadata, API versioning, middleware,
  guard, interceptor, pipe, exception-filter, and HTTP server surfaces.
- Config loading and lookup surface via `ConfigModule`, `ConfigOptions`, and
  `ConfigService`.
- WebSocket gateway, room/broadcast helper, OpenAPI/Swagger, GraphQL, event
  emitter, scheduling, health-check, logging, and validation surfaces.
- CLI statechart commands for validation, parity, visualization, diffing, and inspection.
- CLI project scaffolding and generator commands for modules, controllers, services, guards, interceptors, pipes, filters, middleware, and resources.
- Example applications for hello-world, config-env, testing, auth-jwt, crud-rest-api, and websocket-chat.
- Documentation pages for getting started, first steps, core architecture, HTTP
  surfaces, runtime hooks, realtime/docs features, testing, migration, framework
  comparisons, and publishing order.

### Changed

- Request, provider, and module lifecycles now stay aligned with SCXML contracts.
- The umbrella crate now exposes the current app/bootstrap, config, DI, routing, and WebSocket surfaces consistently.
- Bootstrap-time configuration and OpenAPI/Swagger route setup are documented as current, honest surfaces rather than future work.

### Fixed

- SCXML gate coverage now rejects invalid transitions instead of allowing ad hoc runtime paths.
- Config loading and lookup behavior now has focused tests for env file loading, overrides, coercion, and global visibility.
- CLI generator output is now backed by focused compile checks and example scaffolds.

### Docs

- Added configuration, DI container, and Rust-framework comparison docs.
- Added getting started, first steps, testing, WebSocket, module system, and controller docs.

### Tests

- Expanded integration coverage for SCXML compliance, HTTP request flow, OpenAPI, WebSocket, config, and CLI surfaces.
- Added release-prep gates for workspace check/test/clippy/fmt/docs, SCXML
  validation/parity, package mirror checks, coverage, and benchmark smoke tests.

### Known Gaps

- This is a `0.1.0` release: public APIs remain early, final API review and
  fuller rustdoc examples are still tracked as release-prep follow-ups.
