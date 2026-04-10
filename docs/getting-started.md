# Getting Started

This quickstart gets you from clone to a working Nivasa app shell.

## 1. Install

Install a recent stable Rust toolchain first:

```bash
rustup toolchain install stable
rustup default stable
```

## 2. Create An App

Use the CLI scaffold to create a new project:

```bash
cargo run -p nivasa-cli -- new myapp
```

That creates a starter app layout with `Cargo.toml`, `src/main.rs`, `src/app_module.rs`, `.env`, `.gitignore`, and default SCXML files.

## 3. Run Hello World

The repo includes a minimal hello-world example that builds the app shell and route metadata:

```bash
cargo run --manifest-path examples/hello-world/Cargo.toml
```

It prints the resolved route list for the example app.

## 4. Add Your First Route

Inside your app module, define a controller and route:

```rust
use nivasa::prelude::*;

#[controller("/hello")]
pub struct HelloController;

#[impl_controller]
impl HelloController {
    #[get("/")]
    pub fn greet(&self) -> &'static str {
        "hello world"
    }
}
```

Attach the controller to your module:

```rust
#[module({
    controllers: [HelloController],
})]
pub struct AppModule;
```

## 5. Verify

Check the example builds cleanly:

```bash
cargo check --manifest-path examples/hello-world/Cargo.toml
```

## Note

This guide covers the current scaffold and build-shell flow. The full app listen API is still under Phase 8, so the starter example stops at building and inspecting route metadata rather than starting a live server.
