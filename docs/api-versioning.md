# API Versioning

This document describes the public versioning surface in Nivasa and how far it is wired into runtime today. `global_prefix` is exposed through `ServerOptions`, and `AppBootstrapConfig::global_prefix()` now exposes the normalized value for bootstrap-time route setup, but it is still not wired into runtime route registration yet. `ServerOptions.cors` now also toggles the minimal transport-side CORS bridge, while richer middleware/CorsOptions work stays outside this document's runtime contract.

## What Exists Today

The umbrella crate now exports app-facing config types in `nivasa`:

1. `VersioningStrategy` with `Uri`, `Header`, and `MediaType` variants.
1. `VersioningOptions`, which stores the selected strategy plus an optional default version.
1. `ServerOptions`, which groups `host`, `port`, `cors`, `global_prefix`, and `versioning`.
1. `AppBootstrapConfig`, which currently wraps `ServerOptions` as a pure bootstrap boundary and exposes `global_prefix()` for route setup.

These types are available from the crate root and the prelude, and their builders normalize simple input forms such as `1` into `v1`. The `cors` flag is the transport bridge toggle, not a richer policy object yet.

The HTTP layer also already understands versioned route registration:

1. URI versioning can map controller metadata into `/v1/...` style paths.
1. `NivasaServer::builder()` can register header-versioned routes and media-type-versioned routes.
1. Request dispatch looks at `X-API-Version` first and then `Accept` before it filters the route registry.
1. The dispatch path still preserves the existing method-aware `404` vs `405` behavior.

## What Is Wired Into Runtime

Versioning is currently a transport and routing concern, not an application-bootstrap setting.

The SCXML request pipeline remains the owner of request lifecycle transitions, but version parsing happens before route matching:

1. The transport layer parses version hints from request headers.
1. It filters the route registry to the versioned or unversioned routes that should be considered for the request.
1. `RequestPipeline` then continues through its SCXML-driven lifecycle and calls `match_route` on that filtered registry.
1. `AppBootstrapConfig` is still just the data handoff boundary for app-level configuration, not a runtime bootstrap executor.
1. `global_prefix` follows the same pattern today: it can be supplied in config and read through `AppBootstrapConfig::global_prefix()`, but runtime route registration does not read or apply it yet.

## What Is Not Yet Wired

The new app-facing config surface is intentionally ahead of runtime integration:

1. `AppBootstrapConfig` is exported, but it remains pure configuration rather than a runtime bootstrap object.
1. `ServerOptions.versioning` exists, but the server does not read it yet.
1. `AppBootstrapConfig::global_prefix()` exists, but the runtime server does not consume it yet.
1. `ServerOptions.cors` now toggles the minimal transport-side bridge, but middleware/CorsOptions-based CORS policy remains future work.
1. There is no `NestApplication`-style bootstrap path wired up to consume `AppBootstrapConfig` or `VersioningOptions` at application start.

## Practical Notes

1. Keep versioning logic separate from the SCXML request pipeline contract.
1. Treat URI versioning as the baseline route shape, with header and media-type versioning handled by the HTTP transport layer until app-level wiring lands.
