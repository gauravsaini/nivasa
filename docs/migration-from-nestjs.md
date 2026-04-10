# Migration from NestJS

This guide maps common NestJS patterns to the current Nivasa surface. It is
honest about what is already landed and what is still future work.

## SCXML Rule

Nivasa is not "NestJS with different names". Lifecycle behavior is still
SCXML-gated, and request/module transitions must stay inside the generated
statechart contracts. Use the equivalent Nivasa surface, not ad hoc runtime
hooks.

## Mental Model

Most NestJS concepts already have a close Nivasa counterpart:

- `@Module()` -> `#[module({ ... })]`
- `@Injectable()` -> `#[injectable]`
- `@Controller()` -> `#[controller("/path")]`
- `@Get()`, `@Post()`, ... -> `#[get]`, `#[post]`, ...
- Guards, interceptors, pipes, filters -> the matching Nivasa crates and macros
- `ConfigModule.forRoot()` -> `ConfigModule::for_root(...)`
- `ConfigModule.forFeature()` -> `ConfigModule::for_feature(...)`

The important difference is that Nivasa stays more explicit today. Some pieces
are macro-driven metadata, some are runtime hooks, and some NestJS conveniences
are still future work.

## Modules

NestJS:

```ts
@Module({
  imports: [AuthModule],
  controllers: [UserController],
  providers: [UserService],
  exports: [UserService],
})
export class UserModule {}
```

Nivasa:

```rust
use nivasa::prelude::*;

#[module({
    imports: [AuthModule],
    controllers: [UserController],
    providers: [UserService],
    exports: [UserService],
})]
pub struct UserModule;
```

What to know:

1. `imports` control visibility through the module graph.
1. `exports` decide what consumers can see.
1. `is_global: true` widens visibility, but it does not bypass export rules.
1. `DynamicModule` is the current runtime-configurable module escape hatch.

Current gap:

1. Auto-registration for generated modules is still not a finished CLI workflow.
1. The runtime lifecycle is still owned by module runtime/orchestrator code,
   not by module metadata alone.

## Controllers And Routes

NestJS controller and method decorators map directly to Nivasa controller
macros.

```rust
use nivasa::prelude::*;

#[controller("/users")]
pub struct UsersController;

#[impl_controller]
impl UsersController {
    #[get("/")]
    pub fn list(&self) -> &'static str {
        "users"
    }
}
```

Route metadata, prefix merging, and route conflict checks are already landed.
The current boundary is still metadata-driven route registration, not full
automatic controller invocation for every marker.

## Services And DI

NestJS:

```ts
@Injectable()
export class UsersService {}
```

Nivasa:

```rust
use nivasa::prelude::*;

#[injectable]
pub struct UsersService;
```

Current parity:

1. Singleton, scoped, and transient provider support exist.
1. Optional and lazy dependency shapes exist.
1. `ConfigService` is a real injectable surface.

Current gap:

1. The DI story is solid for current phases, but some advanced NestJS testing
   and auto-mocking helpers are still not complete.

## Guards

NestJS:

```ts
@UseGuards(AuthGuard)
```

Nivasa:

```rust
use nivasa::prelude::*;

#[guard(AuthGuard)]
#[controller("/private")]
pub struct PrivateController;
```

What to know:

1. Guard metadata is captured.
1. Runtime guard evaluation exists in the SCXML-backed request pipeline.
1. `Reflector` is the read-only metadata lookup helper.

Current gap:

1. Some controller- and module-wide guard policies are still metadata-first or
   partially wired.

## Interceptors

NestJS:

```ts
@UseInterceptors(LoggingInterceptor)
```

Nivasa:

```rust
use nivasa::prelude::*;

#[interceptor(LoggingInterceptor)]
#[controller("/events")]
pub struct EventsController;
```

What ships today:

1. Bootstrap/server-builder interceptor hooks are real.
1. `LoggingInterceptor`, `TimeoutInterceptor`, `CacheInterceptor`, and
   `ClassSerializerInterceptor` exist.
1. Interceptor metadata is captured on controllers and methods.

Current gap:

1. Full decorator-driven module wiring for interceptors is still incomplete.

## Pipes

NestJS pipes map to the current `nivasa-pipes` crate and the controller
extractor metadata.

```rust
use nivasa::prelude::*;

#[controller("/users")]
pub struct UsersController;

#[impl_controller]
impl UsersController {
    #[post("/")]
    pub fn create(
        #[body]
        dto: CreateUserDto,
    ) -> String {
        dto.name
    }
}
```

Current gap:

1. Full decorator-driven wiring for every future pipe shape is still growing.

## Configuration

NestJS:

```ts
ConfigModule.forRoot({ isGlobal: true })
```

Nivasa:

```rust
let module = ConfigModule::for_root(ConfigOptions::new().with_global(true));
```

Current parity:

1. `ConfigModule::load_env(...)`
1. `ConfigService`
1. Global config visibility through module metadata
1. `for_root` and `for_feature`

Current gap:

1. Schema validation and required-key enforcement are still future work.

## WebSocket

NestJS gateway patterns map to the current websocket macro and runtime
metadata surfaces.

```rust
use nivasa_macros::{subscribe_message, websocket_gateway};

#[websocket_gateway({ path: "/ws", namespace: "/chat" })]
pub struct ChatGateway;

impl ChatGateway {
    #[subscribe_message("chat.join")]
    pub fn join(&self, room: String) -> String {
        format!("joined:{room}")
    }
}
```

Current parity:

1. Gateway metadata exists.
1. Room and namespace registry surfaces exist.
1. `#[subscribe_message]`, `#[message_body]`, and `#[connected_socket]` are
   already in the macro surface.

Current gap:

1. A fully wired transport example is still minimal.
1. Interceptor/guard/controller integration for websocket handlers is still
   evolving.

## Testing

NestJS testing helpers do not yet have a one-to-one Nivasa equivalent. The
current repo instead uses focused unit and integration tests around the real
module, routing, request, and config surfaces.

Current gap:

1. `TestClient`
1. `TestingModule`
1. Mock provider helpers

## App Bootstrap

NestJS `NestFactory.create(...)` maps most closely to the current
`NestApplication::create(...).build()` flow.

```rust
use nivasa::prelude::*;

let app = NestApplication::create(AppModule)
    .build()
    .expect("app shell should build");
```

Current gap:

1. Full transport startup/shutdown ergonomics are still partial.
1. The docs and examples intentionally stop at the honest current app shell.

## Migration Advice

1. Start with modules and controller metadata, not transport glue.
1. Move providers into `#[injectable]` services.
1. Use the current config and WebSocket helpers where they already exist.
1. Treat missing conveniences as gaps, not as bugs in your port.
1. Keep SCXML lifecycle boundaries intact while you migrate.
