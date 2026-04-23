# Controller Response Metadata

This page describes the controller method metadata used to express response defaults at compile time.

## Supported Metadata

`#[http_code(201)]` marks a routed controller method with an explicit HTTP status code.
`#[header("key", "value")]` attaches response headers to the generated controller metadata.

These attributes are validated when the macro expands:

1. `#[http_code(...)]` must parse as a status code between `100` and `599`.
1. `#[header(...)]` must receive exactly two non-empty string arguments.
1. A controller method can only carry one `#[http_code(...)]` marker, but it can carry multiple header markers.
1. Response metadata is only accepted on methods that also declare an HTTP route marker such as `#[get]` or `#[post]`; otherwise expansion fails with `controller metadata requires an HTTP method attribute`.

The route-only guardrail matters because `#[impl_controller]` only records response metadata for actual controller routes. A plain inherent method with `#[http_code]` or `#[header]` is rejected instead of being silently ignored.

## What The Macro Emits

The controller macro records response metadata alongside the generated route metadata.
The generated controller helpers expose the collected values so runtime code can inspect them:

- `__nivasa_controller_response_metadata()` returns the handler name, optional status code, and collected headers.
- `apply_controller_response_metadata(...)` applies the matching handler entry to a `NivasaResponse`.
- `ControllerResponseMetadata<'_>` names the generated metadata tuple shape for helper consumers.

Those entries are produced at compile time and reflect the route handlers that passed validation.

## Runtime Boundary

Response metadata has a focused runtime helper today. Generated controller metadata can be applied after the handler returns and before response finalization without adding a new SCXML stage.

Full automatic invocation from generated controller metadata remains future work; current route handlers call the helper explicitly when they want the metadata-applied response.

## Request Versus Response Headers

Do not confuse this attribute with the request-side `#[header("name")]` parameter extractor.

1. `#[header("key", "value")]` on a controller method is response metadata.
1. `#[header("name")]` on a handler parameter is request extraction metadata.

They share the same name, but they serve different stages of the pipeline.
