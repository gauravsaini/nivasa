# API Versioning

This document describes the current versioning shape in the Nivasa codebase.

## What Is Implemented

URI versioning is implemented in the routing layer. Versioned controller metadata can be turned into `/v1/...` style paths, and the dispatch layer can resolve versioned routes while preserving the existing method-aware `404` vs `405` behavior.

The routing surfaces currently relevant to versioning are:

1. `ControllerMetadata` for controller-level path and version metadata.
1. `RouteDispatchRegistry` for registering versioned routes.
1. `RouteDispatchOutcome` for matching, `404`, and `405` results.

## What Is In Progress

Header versioning and media type versioning are being added in the routing layer as explicit version-aware registration paths. The intended inputs are:

1. `X-API-Version: 1`
1. `Accept: application/vnd.app.v1+json`

Those routes should remain compatible with the existing route ordering and capture behavior.

## What Is Not Yet Present

The app-level `VersioningOptions` configuration is not wired into `NestApplication` yet. That means versioning is still primarily a routing concern, not a global application setting.

## Practical Notes

1. Keep versioning logic separate from the SCXML request pipeline. The request pipeline should continue to delegate transition control to `RequestPipeline` and `StatechartEngine`.
1. Treat URI versioning as the baseline. Header and media type versioning can build on the same routing primitives once the application-level config exists.
