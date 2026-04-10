# Auth JWT Example

Minimal Nivasa example that shows an auth-flavored controller with guard
metadata and a single protected route.

## Run

```bash
cargo run --manifest-path examples/auth-jwt/Cargo.toml
```

This example builds the app shell and route metadata. It keeps the slice small
so it can compile without a full JWT backend yet.
