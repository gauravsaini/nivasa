# DI Container

`nivasa-core` exposes the current dependency injection surface. It is already
usable for provider registration, scoped resolution, optional/lazy resolution,
and lifecycle-checked provider builds.

## What Exists Now

The current container surface gives you:

- `DependencyContainer` as the main registration and resolution entry point.
- `ProviderScope` with `Singleton`, `Scoped`, and `Transient`.
- `register_value(...)` for prebuilt values.
- `register_factory(...)` for factory-based providers.
- `register_injectable(...)` for `Injectable` types.
- `resolve(...)`, `resolve_optional(...)`, `has(...)`, and `remove(...)`.
- `create_scope()` for request-scoped child containers.
- `Lazy<T>` and `Option<Arc<T>>` support in injectable graphs.

## Scopes

Provider scope controls caching behavior:

1. `Singleton` is built once and reused across the container.
1. `Scoped` is cached per child scope.
1. `Transient` is rebuilt on every resolve.

`DependencyContainer::create_scope()` returns a child container that shares the
same registrations and singleton store, but keeps its own scoped cache.

## Provider Types

### Value Providers

`register_value(T)` stores a prebuilt value as a singleton provider. This is
the simplest way to inject configuration objects, test doubles, or immutable
runtime state.

### Factory Providers

`register_factory::<T>(scope, dependencies, factory)` builds a provider from a
closure. The factory receives the container and can resolve other services as
part of construction.

### Injectable Types

`register_injectable::<T>(scope, dependencies)` hooks a type that implements
`Injectable`. In practice, the `#[injectable]` macro generates that trait
implementation for you.

## Optional and Lazy Resolution

The container already supports the common cycle-breaking patterns:

- `Option<Arc<T>>` resolves to `None` when the provider is missing.
- `Lazy<T>` defers resolution until first use.

That means you can model graphs that would otherwise be circular without
forcing every dependency to be eagerly constructed up front.

## Lifecycle

Provider construction is not a bare function call. Registered providers are
wrapped in `LifecycleProvider`, which drives an SCXML-backed provider state
machine during resolution.

Current lifecycle behavior:

- provider registration moves into the registered lifecycle surface
- provider resolution advances through the SCXML lifecycle events
- `DependencyContainer::initialize()` validates the dependency graph and
  pre-instantiates singletons in dependency order

This keeps provider construction honest and keeps invalid provider transitions
out of the runtime path.

## Runtime Operations

- `resolve::<T>()` returns a typed `Arc<T>`.
- `resolve_optional::<T>()` returns `Ok(None)` when the provider is absent.
- `has::<T>()` checks whether a provider is registered.
- `remove::<T>()` deregisters the provider and invalidates cached instances.

## Honest Limits

This container is still focused on the current core surface. It does not yet
document or expose:

- a public testing container builder
- mock-provider utilities
- automatic module-wide provider discovery outside existing registration flow
- higher-level startup schema validation

## Example

```rust
use nivasa_core::{DependencyContainer, ProviderScope};

let container = DependencyContainer::new();

container
    .register_value(String::from("localhost"))
    .await;

container
    .register_factory::<u16>(ProviderScope::Singleton, vec![], |_| {
        Box::pin(async { Ok(3000) })
    })
    .await;

let host = container.resolve::<String>().await?;
let port = container.resolve::<u16>().await?;
```
