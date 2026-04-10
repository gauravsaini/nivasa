# Config Env Example

Minimal example showing how to load config from environment files and process
environment values.

## Run

```bash
cargo run --manifest-path examples/config-env/Cargo.toml
```

The example loads `.env` if present, then overlays process environment values.
