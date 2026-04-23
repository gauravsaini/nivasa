# Controller Extractors

This page documents the controller parameter surface that the macros recognize today and separates it from what the runtime can actually extract at request time.

The public request extractor for `HeaderMap` is now landed in `nivasa-http`, and controller-side runtime slices for `#[body]`, `#[header("name")]`, `#[headers]`, `#[ip]`, `#[session]`, `#[custom_param(...)]`, `#[res]`, and multipart `#[file]` / `#[files]` helpers are also landed. Those slices are intentionally narrow and run after route dispatch, so they do not bypass the SCXML request pipeline.

## Compile-Time Surface

`#[impl_controller]` recognizes these parameter markers when it scans handler signatures on routed controller methods:

- `#[body]`
- `#[param("name")]`
- `#[query]`
- `#[header("name")]`
- `#[headers]`
- `#[req]`
- `#[res]`
- `#[ip]`
- `#[session]`
- `#[file]`
- `#[files]`
- `#[custom_param(MyExtractor)]`

The macro records them as controller metadata. Runtime helper functions cover the landed slices, while fully automatic argument binding remains a future convenience layer.

The current compile-time guardrails are:

- A handler parameter can use only one extractor attribute.
- `#[param]`, `#[query]`, `#[header]`, and `#[custom_param]` require a non-empty argument.
- `#[body]`, `#[headers]`, `#[req]`, and `#[res]` may appear with or without an optional string label, but `#[req = "..."]` and `#[res = "..."]` are rejected in favor of bare or list syntax.
- `#[ip]`, `#[session]`, `#[file]`, and `#[files]` do not accept arguments.
- Parameter extractors are only accepted on methods that also have an HTTP route marker such as `#[get]` or `#[post]`; otherwise `#[impl_controller]` rejects the method with `controller metadata requires an HTTP method attribute`.

The macro also validates the obvious shape errors up front:

- Empty extractor names are rejected.
- `#[custom_param(...)]` must name a type.
- Invalid attribute forms fail during macro expansion instead of leaking into runtime.

## Runtime Extraction Today

The request layer in `nivasa-http` currently exposes concrete extraction support through `NivasaRequest::extract<T>()` and `FromRequest` implementations for:

- `NivasaRequest`
- `RoutePathCaptures`
- `HeaderMap`
- `Body`
- `Vec<u8>`
- `String`
- `serde_json::Value`
- `Json<T>`
- `Query<T>`

That gives the runtime support we have today for the following controller markers:

| Marker | Runtime support today |
| --- | --- |
| `#[body]` | Request body access through `Body`, `String`, `Vec<u8>`, `serde_json::Value`, or typed `Json<T>` |
| `#[param("name")]` | Captured path parameters through `RoutePathCaptures` and `path_param_typed` |
| `#[query]` | Full query parsing through `Query<T>` plus single-value helpers on `NivasaRequest` |
| `#[header("name")]` | Single-header lookup through `header()` and typed lookup through `header_typed()` |
| `#[headers]` | Full header-map extraction through `run_controller_action_with_headers(...)` |
| `#[req]` | Raw request access through `NivasaRequest` |
| `#[res]` | Mutable controller response access through `ControllerResponse` and `NivasaResponseBuilder`; this is the first landed runtime slice and remains intentionally narrow |
| `#[ip]` | Client-IP extraction through `run_controller_action_with_ip(...)` using a `ClientIp` extension or common proxy headers |
| `#[session]` | Typed session payload extraction through `run_controller_action_with_session(...)` from request extensions |
| `#[file]` | Single-file multipart helper via `run_controller_action_with_file(...)` after route dispatch |
| `#[files]` | Multi-file multipart helper via `run_controller_action_with_files(...)` after route dispatch |
| `#[custom_param(MyExtractor)]` | Custom extractor support through `ControllerParamExtractor<T>` and `run_controller_action_with_custom_param(...)` |

Fully automatic controller argument binding is still not shipped; route handlers call the focused runtime helpers explicitly.

## A Small Naming Note

`#[header("name")]` on a handler parameter is request extraction metadata.

Do not confuse it with the method-level response metadata form `#[header("key", "value")]`, which is documented separately.
