# Guards

This page covers the guard surface that exists today and the metadata path
that feeds it.

## What Ships Today

The current guard stack gives you:

1. `Guard` and `ExecutionContext` in [`nivasa-guards`](../nivasa-guards/src/lib.rs).
1. The `AuthGuard` skeleton for bearer-token style checks.
1. `Reflector` in [`nivasa-core`](../nivasa-core/src/reflector.rs) as the read-only metadata lookup helper.
1. `#[guard(...)]` and `#[roles(...)]` metadata capture in [`nivasa-macros`](../nivasa-macros/src/controller.rs).
1. `#[set_metadata(...)]` as the lower-level metadata capture primitive.
1. Bootstrap-time guard registration through `AppBootstrapConfig::use_global_guard(...)` and the transport builder path.

The runtime side is already SCXML-gated. Guards run as part of the request
pipeline, not as a side channel around it.

## Metadata Split

The important split today is:

1. Macros capture guard and role metadata on controllers and handlers.
1. `RequestContext` carries that metadata at runtime.
1. `Reflector` reads that metadata in a typed, read-only way.
1. `RolesGuard` and other guard code consume the metadata during request evaluation.

That means the docs should not pretend `#[roles]` is a standalone runtime
policy engine. It is metadata capture plus runtime lookup.

## Proof Points

The current behavior is covered by focused tests:

1. [`nivasa-http/tests/controller_system.rs`](../nivasa-http/tests/controller_system.rs) proves controller and handler role metadata flow into guard evaluation.
1. [`nivasa-http/tests/request_lifecycle_integration.rs`](../nivasa-http/tests/request_lifecycle_integration.rs) proves guards run inside the SCXML request lifecycle.
1. [`nivasa-core/src/reflector.rs`](../nivasa-core/src/reflector.rs) has direct coverage for handler, class, and custom metadata lookups.
1. [`nivasa-macros/tests/trybuild/controller_roles_pass.rs`](../nivasa-macros/tests/trybuild/controller_roles_pass.rs) proves the `#[roles(...)]` macro shape.

## What Is Still Upcoming

The guard surface is still incomplete in a few places:

1. Controller and module-level `#[guard(...)]` metadata is captured, but broader module-wide runtime enforcement is still future work.
1. `AuthGuard` is only a skeleton for bearer-token shape checks.
1. Richer policy helpers like fully plumbed RBAC/claims logic are still upcoming.

## Practical Notes

1. Use `Reflector` when you need typed metadata reads.
1. Keep guard logic inside the SCXML-backed request pipeline.
1. Treat `#[roles(...)]` as metadata that a guard can interpret, not as a separate runtime system.
