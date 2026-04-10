# First Steps

This guide shows how to build a tiny Nivasa app from scratch using the current
module, controller, and service surfaces. It stays aligned with what the code
actually does today: app shell creation, metadata-driven route registration, and
SCXML-gated runtime boundaries.

## SCXML Rule

The app shell and request pipeline still obey the SCXML contract. Keep lifecycle
behavior inside the generated statechart boundaries and do not treat this guide
as a shortcut around `NestApplication::build()` or the request pipeline gates.

## 1. Create A Service

Start with an injectable service. In the current codebase, services are plain
Rust structs marked with `#[injectable]`.

```rust
use nivasa::prelude::*;

#[injectable]
pub struct GreeterService;

impl GreeterService {
    pub fn greet(&self) -> &'static str {
        "hello from service"
    }
}
```

At this stage the service is just a normal Rust type with framework metadata.
The DI container picks it up through the macro-generated `Injectable`
implementation.

## 2. Create A Controller

Controllers declare route prefixes with `#[controller]` and route handlers with
HTTP method decorators inside `#[impl_controller]`.

```rust
use nivasa::prelude::*;

#[controller("/hello")]
pub struct HelloController;

#[impl_controller]
impl HelloController {
    #[get("/")]
    pub fn index(&self) -> &'static str {
        "hello world"
    }
}
```

This current slice is metadata-first. The macro generates controller metadata
that the app shell and routing layer can read during build time.

## 3. Bundle Them In A Module

Modules collect controllers, providers, and exports. The `#[module]` macro
captures that wiring.

```rust
use nivasa::prelude::*;

#[module({
    controllers: [HelloController],
    providers: [GreeterService],
    exports: [GreeterService],
})]
pub struct AppModule;
```

Keep `controllers` and `providers` in sync with the concrete types you define.
That is the contract the current module metadata surface expects.

## 4. Build The App Shell

The current app entry point creates a shell with `NestApplication::create(...)`
and resolves module metadata with `.build()`.

```rust
mod app_module;

use app_module::AppModule;
use nivasa::prelude::*;

fn main() {
    let app = NestApplication::create(AppModule)
        .build()
        .expect("app shell should build");

    println!("registered routes: {:?}", app.routes());
}
```

That is the honest boundary today. The build step resolves metadata and route
registration, but full transport startup is still a separate phase.

## 5. Run It

For a concrete example, look at the generated hello-world sample under
[`examples/hello-world/`](../examples/hello-world/). It follows the same
pattern as above and is a good starting point for new apps.

```bash
cargo run --manifest-path examples/hello-world/Cargo.toml
```

## What To Do Next

1. Add a second controller method with another HTTP verb.
1. Move shared logic into a service and reference it from the module metadata.
1. Keep route and module changes behind the existing macros so the generated
   metadata stays honest.
