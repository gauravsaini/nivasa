# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
when releases are cut.

## [Unreleased]

### Added

- SCXML-backed request, module, provider, and application lifecycle enforcement.
- Config loading and lookup surface via `ConfigModule`, `ConfigOptions`, and `ConfigService`.
- CLI statechart commands for validation, parity, visualization, diffing, and inspection.
- CLI project scaffolding and generator commands for modules, controllers, services, guards, interceptors, pipes, filters, middleware, and resources.
- Example applications for hello-world, config-env, testing, auth-jwt, crud-rest-api, and websocket-chat.
- Documentation pages for configuration, DI container behavior, examples, and framework comparisons.

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
