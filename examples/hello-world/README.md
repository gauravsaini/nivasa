# Hello World Example

Minimal Nivasa app with one `GET /hello` route.

## Run

```bash
cargo run --manifest-path examples/hello-world/Cargo.toml
```

This example currently builds the app shell and route metadata. It does not
start a transport listener yet because Phase 8 application listen wiring is
still in progress.
