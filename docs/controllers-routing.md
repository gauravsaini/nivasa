# Controllers & Routing

This page covers the controller macros, route registry, extractor surface, versioning, and response types that are already landed in the repo.

## SCXML Rule

Keep controller and routing work aligned with the request SCXML contract. Route matching and controller metadata are part of the request pipeline, but they should not be treated as a separate lifecycle.

## Controller Surface

`nivasa-macros` currently recognizes these controller and route markers:

1. `#[controller("/users")]`
1. `#[controller({ path: "/users", version: "1" })]`
1. `#[impl_controller]`
1. `#[get("/path")]`
1. `#[post("/path")]`
1. `#[put("/path")]`
1. `#[delete("/path")]`
1. `#[patch("/path")]`
1. `#[head("/path")]`
1. `#[options("/path")]`
1. `#[all("/path")]`

The generated controller metadata captures route prefixes, method paths, duplicate-route checks, and parameter metadata for later runtime use.

The current guardrails are:

1. `#[impl_controller]` only accepts methods that also have an HTTP route marker.
1. Duplicate routes inside one controller are rejected.
1. Prefix merging happens in the order `global prefix -> controller prefix -> method path`.
1. Versioned controller metadata is preserved on the generated controller helpers.

See:

- [`/Users/ektasaini/Desktop/nivasa/nivasa-macros/src/controller.rs`](/Users/ektasaini/Desktop/nivasa/nivasa-macros/src/controller.rs)
- [`/Users/ektasaini/Desktop/nivasa/nivasa-routing/src/lib.rs`](/Users/ektasaini/Desktop/nivasa/nivasa-routing/src/lib.rs)

## Route Matching

`nivasa-routing` currently ships a real route registry and matching model:

1. Static segments like `/users`
1. Named parameters like `/users/:id`
1. Optional segments like `/users/:id?`
1. Wildcard segments like `/files/*path`
1. Route ordering that prefers static routes over parameterized, optional, and wildcard routes
1. Duplicate-route conflict detection
1. Prefix merging for global prefix plus controller prefix plus method path

Version-aware dispatch is also implemented:

1. URI versioning such as `/v1/users`
1. Header versioning via `X-API-Version`
1. Media type versioning via `Accept: application/vnd.app.v1+json`

The server and routing layers use this registry to decide whether a request resolves to a handler, returns `404`, or returns `405`.

## Extractors

The controller parameter surface is documented in more detail in [`docs/controller-extractors.md`](/Users/ektasaini/Desktop/nivasa/docs/controller-extractors.md), but the landed runtime support today covers:

1. `#[body]`
1. `#[param("name")]`
1. `#[query]`
1. `#[query("name")]`
1. `#[header("name")]`
1. `#[headers]`
1. `#[req]`
1. `#[res]`
1. `#[file]`
1. `#[files]`

The runtime also exposes the matching request helpers through `NivasaRequest`, `ControllerResponse`, `NivasaResponseBuilder`, and the `FromRequest` implementations in `nivasa-http`.

The compile-time-only markers are still:

1. `#[ip]`
1. `#[session]`
1. `#[custom_param(MyExtractor)]`

## Response Types

`nivasa-http` currently supports these response shapes:

1. JSON responses through `Json<T>` and `serde_json::Value`
1. Plain text responses
1. HTML responses
1. Streaming responses through `StreamBody`
1. SSE responses through `Sse`
1. File downloads through `Download`
1. Redirect responses
1. `HttpStatus` for standard status codes
1. `Result<T, HttpException>` mapping to success or JSON error payloads
1. `#[http_code(201)]` for explicit status codes
1. `#[header("key", "value")]` for response headers

See:

- [`/Users/ektasaini/Desktop/nivasa/docs/controller-extractors.md`](/Users/ektasaini/Desktop/nivasa/docs/controller-extractors.md)
- [`/Users/ektasaini/Desktop/nivasa/docs/controller-response-metadata.md`](/Users/ektasaini/Desktop/nivasa/docs/controller-response-metadata.md)
- [`/Users/ektasaini/Desktop/nivasa/docs/http-surface.md`](/Users/ektasaini/Desktop/nivasa/docs/http-surface.md)

## Current Boundary

The route registry, extractor metadata, and response helpers are real today. Full controller invocation from generated metadata is still the later wiring step, so treat this page as the honest current surface, not a promise that every controller marker is already executed automatically.
