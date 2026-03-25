# Controller Extractors

This page documents the controller parameter surface that the macros recognize today and separates it from what the runtime can actually extract at request time.

The public request extractor for `HeaderMap` is now landed in `nivasa-http`, but controller-side `#[headers]` binding is still waiting on the later SCXML handler-execution path.

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

The macro records them as controller metadata, but it does not automatically execute an argument binder at runtime yet.

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
| `#[headers]` | Full header-map extraction is available through `NivasaRequest::extract::<HeaderMap>()`, but controller-side execution still waits on SCXML handler binding |
| `#[req]` | Raw request access through `NivasaRequest` |

The remaining markers are compile-time metadata only today:

- `#[res]`
- `#[ip]`
- `#[session]`
- `#[file]`
- `#[files]`
- `#[custom_param(MyExtractor)]`

For `#[custom_param(MyExtractor)]`, the macro records the extractor type name, but the runtime does not yet have a controller executor that consumes that metadata automatically.

## A Small Naming Note

`#[header("name")]` on a handler parameter is request extraction metadata.

Do not confuse it with the method-level response metadata form `#[header("key", "value")]`, which is documented separately.
