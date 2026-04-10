# Module System

This guide covers the current Nivasa module surface: metadata, imports,
exports, globals, dynamic modules, registry visibility, and lifecycle hooks.
It stays aligned with the code that exists today.

## SCXML Rule

Module lifecycle behavior is still expected to stay aligned with the SCXML
statechart contract. `ModuleMetadata` describes structure, while runtime
ordering and lifecycle transitions belong to `nivasa-core` module runtime and
orchestrator code.

## Core Shape

A module today is a Rust type plus metadata. The metadata surface is centered
around `ModuleMetadata`:

- `imports`
- `providers`
- `controllers`
- `exports`
- `middlewares`
- `is_global`

The `#[module]` macro captures that metadata and generates helper methods that
the runtime can read during build and bootstrap.

```rust
use nivasa::prelude::*;

#[module({
    imports: [AuthModule],
    controllers: [UserController],
    providers: [UserService],
    exports: [UserService],
    middlewares: [LoggingMiddleware],
})]
pub struct UserModule;
```

## Imports And Exports

Imports form the module graph. Exports decide which providers are visible to
consumers of that module. The registry only treats an export as valid if it is
provided locally or re-exported from an imported module.

That means:

- Imported modules can contribute visibility through their exports.
- Non-exported providers stay hidden from consumers.
- Re-exporting is explicit, not automatic.

The runtime visibility rules are implemented in `ModuleRegistry` and its
`visible_exports(...)` / `visible_exports_by_id(...)` paths.

## Global Modules

Global modules are marked with `is_global = true`. They are visible without an
explicit import, but they still need valid exports.

```rust
use nivasa::prelude::*;

#[module({
    providers: [ConfigService],
    exports: [ConfigService],
})]
pub struct ConfigModule;
```

The current config surface also uses dynamic-module helpers like
`ConfigModule::for_root(...)` and `ConfigModule::for_feature(...)` to set
module metadata and global visibility.

## Dynamic Modules

Dynamic modules let a module produce metadata at runtime. The current pattern is:

- `DynamicModule::new(metadata)`
- `.with_providers(...)`
- `.with_global(true|false)`
- `ConfigurableModule::for_root(options)`
- `ConfigurableModule::for_feature(options)`

That is how the config module and similar surfaces carry bootstrap-time options
without requiring a separate manual metadata type.

## Runtime Lifecycle

The current module runtime is still SCXML-backed. Module lifecycle hooks are
separate traits:

- `OnModuleInit`
- `OnModuleDestroy`
- `OnApplicationBootstrap`
- `OnApplicationShutdown`

The runtime keeps lifecycle ordering in `nivasa-core` rather than in the module
metadata itself. In practice, that means:

1. Modules are registered in a dependency graph.
1. Imports are resolved before consumers.
1. Global modules are added to visible scope.
1. Lifecycle hooks run through the orchestrator/runtime path, not ad hoc calls.

## Controller Registrations

Modules can also expose controller route registrations through
`ModuleControllerRegistration`. That is how controller metadata becomes part of
the module build step.

Keep controllers, routes, and module metadata consistent. The module macro and
registry assume the metadata is honest.

## Practical Example

```rust
use nivasa::prelude::*;

#[injectable]
pub struct UserService;

#[controller("/users")]
pub struct UserController;

#[impl_controller]
impl UserController {
    #[get("/")]
    pub fn list(&self) -> &'static str {
        "users"
    }
}

#[module({
    controllers: [UserController],
    providers: [UserService],
    exports: [UserService],
})]
pub struct UserModule;
```

That module can now participate in app assembly, route registration, and the
module graph, as long as its exports match what it really provides.

## What To Remember

1. `imports` shape visibility.
1. `exports` decide what consumers can see.
1. `is_global` widens visibility, but does not bypass export rules.
1. `DynamicModule` is the current escape hatch for runtime-configured module
   metadata.
1. Lifecycle hooks still belong to the runtime/orchestrator layer, not the
   metadata layer.
