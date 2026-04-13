# Request Lifecycle

This document maps the HTTP request flow to `statecharts/nivasa.request.scxml`, which is the source of truth for the request pipeline.

## SCXML Rule

The request lifecycle must follow the SCXML statechart. `RequestPipeline` advances a `StatechartEngine<NivasaRequestStatechart>` through typed request events only, and `StatechartEngine::send_event` remains the transition gate. There is no direct state mutation path, and guard denials plus stage errors are expressed as SCXML transitions rather than ad-hoc branching.

The HTTP coordinator uses a private `RequestEvent` bridge to narrow request-lifecycle actions to the generated `NivasaRequestEvent` enum before handing them to `send_event`. In the current runtime cut, that bridge covers the pipeline transitions driven by `RequestPipeline`, including guard denial and late-stage error paths, while keeping the surface aligned with the SCXML contract.

## Current Implemented Stages

The codebase currently exercises the early request stages that are wired into `nivasa-http`:

1. `Received`
1. `MiddlewareChain`
1. `RouteMatching`

That is no longer the runtime boundary. `nivasa-http` now drives requests past route matching into controller execution, including guard, interceptor, pipe, handler, post-interceptor, and response-completion flow. A narrow controller-side `#[res]` response-builder slice exists, and multipart `#[file]` / `#[files]` helpers also run post-route without creating a new SCXML stage.

The implemented coordinator in `nivasa-http` now:

1. Starts each request in `Received`.
1. Advances to `MiddlewareChain` after `RequestParsed`.
1. Advances to `RouteMatching` after `MiddlewareComplete`.
1. Routes `RouteMatched`, `RouteNotFound`, and `RouteMethodNotAllowed` through the SCXML engine.
1. Attaches route path captures to the request when a route matches.
1. Surfaces `StatechartSnapshot` for debug inspection.

The routing layer already supports:

1. Static routes.
1. Named parameters.
1. Optional segments.
1. Wildcard segments.
1. Method-aware dispatch with `404` vs `405` outcomes.
1. Path capture extraction from matched routes.

## Remaining SCXML Stages

The statechart still defines the later lifecycle stages, and most of them now drive in `nivasa-http`. `ErrorHandling` remains the main future stage boundary:

1. `GuardChain`
1. `InterceptorPre`
1. `PipeTransform`
1. `HandlerExecution`
1. `InterceptorPost`
1. `ErrorHandling`
1. `SendingResponse`
1. `Done`

These stages are still important because they define the SCXML contract for controller execution and response completion. Keep `ErrorHandling` as the remaining future-stage caveat.

## Practical Notes

1. `RequestPipeline::parse_request()` and `RequestPipeline::fail_parse()` drive the first SCXML transition pair.
1. `RequestPipeline::complete_middleware()` and `RequestPipeline::fail_middleware()` cover the middleware branch.
1. `RequestPipeline::match_route()` uses routing outcomes to drive the generated SCXML transition table through `send_event`, including the SCXML short-circuit paths for guard denial and stage errors.
1. Request extraction in `nivasa-http` currently supports body, query, header, and path-capture access for the pieces that already exist.
1. Multipart `#[file]` and `#[files]` helpers are post-route conveniences; keep them out of the SCXML stage model.
1. SCXML validation and parity now build cleanly on this machine through the repo-local `pkg-config` wrapper, so `nivasa statechart validate --all` and `nivasa statechart parity` do not need manual `PKG_CONFIG` setup here.
