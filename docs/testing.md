# Testing

This page covers the testing surface that exists today and the testing APIs
that are still planned.

## What Exists Today

The framework already gives you a few concrete building blocks for tests:

1. `DependencyContainer` can register values and factories, resolve providers, and create child scopes.
1. `ModuleRegistry` and `ModuleOrchestrator` can be used in integration-style tests to prove import/export visibility and module bootstrap order.
1. The public API coverage in [`nivasa/tests/public_api.rs`](../nivasa/tests/public_api.rs) keeps the umbrella exports honest.
1. The example in [`examples/testing/`](../examples/testing/) shows the current pattern for test-style provider overrides with a scoped container.

That means you can already write tests that look like:

```rust
let container = DependencyContainer::new();
container.register_value(MyService::default()).await;

let test_scope = container.create_scope();
test_scope.register_value(MyService::mock()).await;

let service = test_scope.resolve::<MyService>().await?;
```

## What Is Still Upcoming

The checklist still calls out a few testing APIs that are not shipped yet:

1. `TestingModule` with `create_testing_module(...)`, provider overrides, and `compile()`.
1. `TestClient` for in-memory HTTP dispatch without TCP.
1. `MockProvider<T>` with call recording, canned values, and argument assertions.

Those are still future work, so this document does not pretend they exist yet.

## Practical Guidance

1. Use `DependencyContainer` and child scopes for focused provider tests today.
1. Use module and public API integration tests when you need to prove visibility or bootstrap behavior.
1. Keep `TestingModule` and `TestClient` references out of production code until those APIs are actually implemented.
