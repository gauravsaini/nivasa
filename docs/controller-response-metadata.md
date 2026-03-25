# Controller Response Metadata

This page describes the controller method metadata used to express response defaults at compile time.

## Supported Metadata

`#[http_code(201)]` marks a controller method with an explicit HTTP status code.
`#[header("key", "value")]` attaches response headers to the generated controller metadata.

These attributes are validated when the macro expands:

1. `#[http_code(...)]` must parse as a status code between `100` and `599`.
1. `#[header(...)]` must receive exactly two non-empty string arguments.
1. A controller method can only carry one `#[http_code(...)]` marker, but it can carry multiple header markers.

## What The Macro Emits

The controller macro records response metadata alongside the generated route metadata.
The generated controller helpers expose the collected values so later runtime code can inspect them.

## Current Limitations

The response metadata is compile-time only today.
It is not yet automatically applied to `NivasaResponse` construction or to the request pipeline at runtime.

That means:

1. The metadata exists for later wiring into a response executor.
1. Handlers still need to build their responses explicitly.
1. The runtime does not yet use the metadata to override status codes or inject headers automatically.

## Request Versus Response Headers

Do not confuse this attribute with the request-side `#[header("name")]` parameter extractor.

1. `#[header("key", "value")]` on a controller method is response metadata.
1. `#[header("name")]` on a handler parameter is request extraction metadata.

They share the same name, but they serve different stages of the pipeline.
