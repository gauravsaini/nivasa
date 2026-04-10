# Testing Example

Minimal example showing how to override a provider in a child DI scope for
test-style code.

## What it shows

- `DependencyContainer::register_value(...)`
- `DependencyContainer::create_scope()`
- provider override with a mock value in a child scope

## Run

```bash
cargo run --manifest-path examples/testing/Cargo.toml
```

This example is intentionally small. Dedicated `TestingModule` and `TestClient`
APIs are still future work in the main framework.
