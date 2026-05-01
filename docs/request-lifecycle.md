# Request Lifecycle

This document maps the HTTP request flow to
`statecharts/nivasa.request.scxml`. That SCXML file is the source of truth for
request lifecycle order, short-circuits, and error transitions.

## SCXML Rule

Every request lifecycle transition must go through
`StatechartEngine<NivasaRequestStatechart>::send_event`.
`RequestPipeline` owns that engine and advances it only through typed
`NivasaRequestEvent` values generated from a private HTTP-side `RequestEvent`
bridge.

There is no direct state mutation path. Runtime code must not skip a stage by
setting state manually or by creating a parallel request lifecycle outside
`RequestPipeline`.

Short-circuits are still SCXML transitions:

1. Parse failure uses `error.parse`.
1. Middleware failure uses `error.middleware`.
1. Route miss uses `route.not_found`.
1. Method mismatch uses `route.method_not_allowed`.
1. Guard denial uses `guard.denied`.
1. Guard failure uses `error.guard`.
1. Interceptor failure uses `error.interceptor` or `error.interceptor_post`.
1. Pipe and validation failures use `error.pipe` or `error.validation`.
1. Handler failure uses `error.handler`.
1. Filter fallback uses `error.filter.unhandled`.
1. Send failure uses `error.send`.

## Runtime Order

Current `nivasa-http` runtime drives the full request lifecycle:

1. `Received`
1. `MiddlewareChain`
1. `RouteMatching`
1. `GuardChain`
1. `InterceptorPre`
1. `PipeTransform`
1. `HandlerExecution`
1. `InterceptorPost`
1. `SendingResponse`
1. `Done`

When a stage fails, the pipeline enters `ErrorHandling` and then continues to
`SendingResponse` before reaching `Done`.

The statechart sequence is:

```text
Received
  -> MiddlewareChain
  -> RouteMatching
  -> GuardChain
  -> InterceptorPre
  -> PipeTransform
  -> HandlerExecution
  -> InterceptorPost
  -> SendingResponse
  -> Done
```

Error path:

```text
any supported stage error
  -> ErrorHandling
  -> SendingResponse
  -> Done
```

## HTTP Coordinator

`dispatch_nivasa_request` now keeps the request path inside the SCXML-backed
pipeline:

1. Seeds `x-request-id` and request context metadata.
1. Runs global middleware before the formal pipeline when configured.
1. Creates `RequestPipeline` in `Received`.
1. Calls `parse_request()` and `complete_middleware()`.
1. Filters routes by request version metadata.
1. Calls `match_route()` so `404`, `405`, and matched routes all move through SCXML.
1. Runs module middleware, route module middleware, and route-scoped middleware.
1. Evaluates guards through `evaluate_guard_chain()`.
1. Marks interceptor pre-processing complete.
1. Applies global pipes and writes transformed body back to the request.
1. Executes controller handler directly or through interceptor chain.
1. Completes handler, post-interceptor, response, and final response stages.
1. Finalizes response headers, request id, and CORS headers.

Middleware short-circuits are finalized as responses, but middleware errors still
enter the SCXML error transition. Route, guard, pipe, interceptor, and handler
errors also enter the SCXML error path before exception filters produce or
fallback to a response.

## Implemented Surfaces

The runtime request path currently covers:

1. Global middleware.
1. Module middleware.
1. Route module middleware.
1. Route-scoped middleware.
1. Static, parameterized, optional, wildcard, and versioned route dispatch.
1. `404` and `405` route outcomes.
1. Route path capture attachment.
1. Global guards and guard-chain short-circuits.
1. Global pipes over request body data.
1. Controller handler execution.
1. Interceptor pre/post execution and short-circuit handling.
1. Handler, controller, and global exception filter selection.
1. CORS and request-id response finalization.
1. `StatechartSnapshot` debug inspection.

Controller-side multipart helpers for `#[file]` and `#[files]` stay outside the
stage model. They are convenience helpers around request extraction and upload
parsing, not separate lifecycle states.

## Practical Notes

1. `RequestPipeline::parse_request()` drives `request.parsed`.
1. `RequestPipeline::complete_middleware()` drives `middleware.complete`.
1. `RequestPipeline::match_route()` drives `route.matched`, `route.not_found`,
   or `route.method_not_allowed`.
1. `RequestPipeline::evaluate_guard_chain()` drives `guards.passed`,
   `guard.denied`, or `error.guard`.
1. `RequestPipeline::complete_interceptors_pre()` drives
   `interceptors.pre.complete`.
1. `RequestPipeline::complete_pipes()` drives `pipes.complete`.
1. `RequestPipeline::complete_handler()` drives `handler.complete`.
1. `RequestPipeline::complete_interceptors_post()` drives
   `interceptors.post.complete`.
1. `RequestPipeline::complete_response()` drives `response.sent`.
1. `RequestPipeline::snapshot()` exposes the current SCXML state for debug
   tooling.

SCXML validation and generated-code parity remain required gates:

```bash
cargo run -p nivasa-cli -- statechart validate --all
cargo run -p nivasa-cli -- statechart parity
```
