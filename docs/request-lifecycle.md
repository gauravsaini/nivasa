# Request Lifecycle

This document maps the HTTP request flow to `statecharts/nivasa.request.scxml`, which is the source of truth for the request pipeline.

## SCXML Rule

The request lifecycle must follow the SCXML statechart. `RequestPipeline` advances a `StatechartEngine<NivasaRequestStatechart>` with typed events only, and the engine rejects invalid transitions. There is no direct state mutation path.

## Current Implemented Stages

The codebase currently exercises the first request stages:

1. `Received`
1. `MiddlewareChain`
1. `RouteMatching`

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

The statechart still defines the later lifecycle stages that are not yet fully wired into the HTTP runtime:

1. `GuardChain`
1. `InterceptorPre`
1. `PipeTransform`
1. `HandlerExecution`
1. `InterceptorPost`
1. `ErrorHandling`
1. `SendingResponse`
1. `Done`

These stages are still important because they define how guards, interceptors, pipes, handlers, and filters must be sequenced once the runtime grows beyond the initial coordinator.

## Practical Notes

1. `RequestPipeline::parse_request()` and `RequestPipeline::fail_parse()` drive the first SCXML transition pair.
1. `RequestPipeline::complete_middleware()` and `RequestPipeline::fail_middleware()` cover the middleware branch.
1. `RequestPipeline::match_route()` uses routing outcomes to move into either `GuardChain` or `ErrorHandling`.
1. Request extraction in `nivasa-http` currently supports body, query, header, and path-capture access for the pieces that already exist.
