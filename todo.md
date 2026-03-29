# Nivasa Framework — TODO

> **Nivasa** (निवास) — A modular, decorator-based Rust web framework with 100% NestJS pattern compliance.
>
> **Reference plan:** [nivasa-framework-implementation-plan.md](./nivasa-framework-implementation-plan.md)
>
> **Architectural backbone:** SCXML (W3C State Chart XML) — every lifecycle is a formally defined statechart.
> All state transitions are code-generated from `.scxml` files and enforced at compile time + runtime.
> See: [SCXML Architecture](./docs/scxml-architecture.md) · [SCXML Enforcement Strategy](./docs/scxml-enforcement.md)

---

## Phase 0: Project Bootstrap

### 0.1 — Repository & Workspace
- [x] Initialize git repository
- [x] Add `.gitignore` (Rust template + IDE files)
- [x] Add `LICENSE` file (decide: MIT / Apache-2.0 dual license)
- [x] Add `README.md` with project overview, badges, and "why Nivasa" section
- [x] Create top-level `Cargo.toml` as workspace root (list all member crates)
- [x] Define workspace-level dependency versions (`[workspace.dependencies]`) for: `tokio`, `serde`, `hyper`, `tower`, `tracing`, `thiserror`, `uuid`, `bytes`, `http`, `quick-xml`
- [x] Decide and document Minimum Supported Rust Version (MSRV) — recommend 1.75+
- [x] Set up `rustfmt.toml` with project formatting rules
- [x] Set up `clippy.toml` / `.clippy.toml` with lint policy
- [x] Set up `deny.toml` (cargo-deny) for license and vulnerability auditing

### 0.2 — Crate Scaffolding
- [x] Create `nivasa/` — main umbrella re-export crate
- [x] Create `nivasa-core/` — DI container, module system, application lifecycle
- [x] Create `nivasa-statechart/` — SCXML engine, codegen, and runtime enforcement
- [x] Create `nivasa-macros/` — all procedural macros (`proc-macro = true`)
- [x] Create `nivasa-http/` — HTTP server, request/response wrappers
- [x] Create `nivasa-routing/` — route registry, matching, param extraction
- [x] Create `nivasa-guards/` — guard trait and execution pipeline
- [x] Create `nivasa-interceptors/` — interceptor trait and chain
- [x] Create `nivasa-pipes/` — pipe trait and built-in pipes
- [x] Create `nivasa-filters/` — exception filter trait and built-in filters
- [x] Create `nivasa-validation/` — validation decorators and engine
- [x] Create `nivasa-config/` — configuration module and service
- [x] Create `nivasa-common/` — shared types: `HttpException`, DTOs, result types
- [x] Create `nivasa-websocket/` — WebSocket gateway and adapter
- [x] Create `nivasa-cli/` — CLI scaffolding tool
- [x] Create `statecharts/` directory — all `.scxml` definitions live here (the source of truth)
- [x] Each crate: add `lib.rs` with module doc comment and basic exports
- [x] Verify `cargo check --workspace` passes on empty crates

### 0.3 — CI / Tooling
- [x] Set up GitHub Actions CI: `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt --check`
- [x] Set up code coverage reporting (tarpaulin or llvm-cov)
- [x] Set up `cargo doc` generation in CI
- [x] **Add SCXML CI step:** `nivasa statechart validate --all` — validates all `.scxml` files are well-formed
- [x] **Add SCXML CI step:** `nivasa statechart parity` — verifies generated Rust code matches current `.scxml` files
- [x] **Add CI step:** verify generated SCXML artifacts in CI with `cargo test -p nivasa-statechart --test generated_statecharts` plus `cargo run -p nivasa-cli -- statechart parity` — the repo does not check in `src/generated/`; build.rs emits generated files into `OUT_DIR`
- [x] Create `examples/` directory with placeholder READMEs for `basic/`, `auth/`, `websocket/`
- [x] Create `tests/` directory for workspace-level integration tests
- [x] Create `docs/` directory for book-style documentation

### 0.4 — Umbrella Crate Re-export Strategy
- [x] Design `nivasa::prelude::*` — users should only need one import
- [x] Re-export key traits and runtime types: `Controller`, `Module`, `Injectable`, plus the landed DI/module/runtime surface; `GuardExecutionContext`, `GuardExecutionOutcome`, `Interceptor`, `Reflector`, `ExceptionFilter`, and `Middleware` (the `NivasaMiddleware` alias) are now re-exported from the umbrella crate, and the `filters`/`pipes` umbrella namespaces are also re-exported; `Pipe` still needs upstream exports
- [x] Re-export key macros: `#[module]`, `#[injectable]`, `#[controller]`, `#[get]`, `#[post]`, `#[put]`, `#[delete]`, `#[patch]`, `#[head]`, `#[options]`, `#[all]`, `#[impl_controller]`, `#[scxml_handler]`
- [x] Re-export `ServerOptions`, `HttpException`, and the existing HTTP/server surface
- [x] Re-export `StatechartEngine`, generated state/event enums from `nivasa-statechart`
- [x] Feature-gate optional sub-crates (e.g., `features = ["websocket", "config", "validation"]`)

---

## Phase 0.5: SCXML Statechart Engine (MUST complete before Phase 1)

> **This phase is the architectural spine.** Nothing else ships without it.
> Every subsequent phase starts with "author the SCXML" and ends with "validate transitions match the statechart."

### 0.5.1 — Author Foundation Statecharts (`statecharts/`)
- [x] Write `statecharts/nivasa.application.scxml` — top-level app lifecycle (Created → Bootstrapping → Running → ShuttingDown → Terminated)
- [x] Write `statecharts/nivasa.module.scxml` — module lifecycle template (Unloaded → Loading → Loaded → Initialized → Active → Destroying → Destroyed)
- [x] Write `statecharts/nivasa.provider.scxml` — DI provider lifecycle (Unregistered → Registered → Resolving → Constructing → Resolved → Disposing → Disposed)
- [x] Write `statecharts/nivasa.request.scxml` — HTTP request pipeline (Received → Middleware → Routing → Guards → InterceptorPre → Pipes → Handler → InterceptorPost → ErrorHandling → Response → Done)
- [x] Validate all SCXML files against W3C SCXML XSD schema
- [x] Verify each statechart: no unreachable states, no dead-ends without `<final>`, deterministic transitions
- [x] Commit these files as the first meaningful commit — _the statecharts are the spec_

### 0.5.2 — SCXML Parser (`nivasa-statechart`)
- [x] Add `quick-xml` crate dependency for XML parsing
- [x] Implement `ScxmlDocument` struct (parsed representation of an `.scxml` file)
- [x] Parse `<scxml>` root element: name, initial, version, datamodel
- [x] Parse `<state>` elements: id, initial, children, is-compound vs atomic
- [x] Parse `<parallel>` elements: id, children
- [x] Parse `<final>` elements: id, donedata
- [x] Parse `<transition>` elements: event, cond, target, type (internal/external)
- [x] Parse `<onentry>` / `<onexit>` placeholders (action references)
- [x] Parse `<history>` elements: id, type (shallow/deep)
- [x] Parse `<invoke>` elements: type, id, src
- [x] Parse `<datamodel>` / `<data>` elements
- [x] Build in-memory state tree from parsed elements
- [x] Unit tests: parse each SCXML construct, round-trip test

### 0.5.3 — SCXML Validation Engine (`nivasa-statechart`)
- [x] Implement reachability analysis — every state must be reachable from `initial`
- [x] Implement completeness check — every non-final state must have at least one outgoing transition
- [x] Implement determinism check — no two transitions from the same state match the same event+cond
- [x] Implement well-formedness check — compound states have children, atomic states don't
- [x] Implement event name validation — dot-separated hierarchical format
- [x] Implement target validation — all transition targets reference existing state IDs
- [x] Output structured validation errors with line numbers from SCXML file
- [x] Unit tests for each validation rule (valid doc, each type of violation)

### 0.5.4 — Build-Time Code Generation (`nivasa-statechart/build.rs` + codegen module)

This is the **primary enforcement mechanism.** The `.scxml` files are read at build time and Rust code is generated. Developers cannot add states, events, or transitions without updating the SCXML first.

- [x] Implement SCXML-to-Rust codegen pipeline (`fn generate_rust(scxml: &ScxmlDocument) -> String`)
- [x] **Generate State enum:** one variant per `<state>`, `<parallel>`, `<final>` from the SCXML
- [x] **Generate Event enum:** one variant per unique `event` attribute across all `<transition>` elements
- [x] **Generate transition table:** `fn transition(state: &State, event: &Event) -> Option<State>` as exhaustive `match`
- [x] **Generate Handler trait:** one required `async fn on_enter_{state_id}()` method per state with `<onentry>` — compiler forces implementation
- [x] **Generate valid_events_for():** returns the set of valid events for each state (for error messages and introspection)
- [x] **Generate `StatechartSpec` trait impl:** ties State enum, Event enum, Handler trait, and transition fn together
- [x] Embed SCXML content hash in generated code (`const SCXML_HASH: &str = "sha256:..."`) for parity checking
- [x] Write generated files to `OUT_DIR` and include via `include!(concat!(env!("OUT_DIR"), "/request.rs"))`
- [x] Implement `build.rs` that scans `statecharts/` directory and triggers codegen for each `.scxml` file
- [x] Add `cargo:rerun-if-changed=statecharts/` to rebuild on any SCXML change
- [x] Unit tests: given a known SCXML, verify the generated Rust code compiles and has the correct enums/variants

### 0.5.5 — Statechart Runtime Engine (`nivasa-statechart::engine`)

The engine is the **only way to transition state at runtime.** There is no `set_state()`. Invalid transitions are rejected.

- [x] Implement `StatechartEngine<S: StatechartSpec>` struct
- [x] Store `current_state: S::State` as **private** field (no public setter)
- [x] Implement `send_event(event: S::Event) -> Result<S::State, InvalidTransitionError>` — the only public state-changing method
- [x] On valid transition: update state, call `on_exit` handler, call `on_enter` handler, return new state
- [x] On invalid transition (debug builds): **panic** with diagnostic: current state, received event, list of valid events
- [x] On invalid transition (release builds): return `Err(InvalidTransitionError)` with same diagnostic info
- [x] Implement `current_state() -> &S::State` — read-only accessor
- [x] Implement `is_in_final_state() -> bool`
- [x] Implement `valid_events() -> Vec<S::Event>` — what events are valid from current state
- [x] Support optional `StatechartTracer` callback for logging every transition
- [x] Unit tests: drive engine through full lifecycle, test invalid transition rejection, test final state detection

### 0.5.6 — Proc Macro: `#[scxml_handler]` (`nivasa-macros`)

Compile-time validation that user-annotated handlers correspond to real SCXML states.

- [x] Implement `#[scxml_handler(statechart = "request", state = "guard_chain")]` attribute macro
- [x] At macro expansion time: load the referenced SCXML file, verify `state` exists
- [x] Emit compile error if the referenced state does not exist in the SCXML
- [x] Emit compile error if the referenced statechart file does not exist
- [x] Write trybuild tests: valid annotation compiles, invalid state name fails

### 0.5.7 — Statechart Introspection (Debug Mode)
- [x] Implement `StatechartTracer` trait: `fn on_transition(from, event, to)`
- [x] Implement `LoggingTracer` — logs every transition via `tracing`
- [x] Build serializable debug snapshot helpers for current state, raw SCXML, and recent transitions
- [x] Implement debug endpoint: `GET /_nivasa/statechart` — returns current state config as JSON
- [x] Implement debug endpoint: `GET /_nivasa/statechart/scxml` — returns raw SCXML document
- [x] Implement debug endpoint: `GET /_nivasa/statechart/transitions` — returns recent transition log
- [x] All introspection endpoints gated behind `#[cfg(debug_assertions)]` — zero cost in release

### 0.5.8 — CLI: `nivasa statechart` Commands (`nivasa-cli`)
- [x] Implement `nivasa statechart validate --all` — validate all SCXML files
- [x] Implement `nivasa statechart validate <file>` — validate one SCXML file
- [x] Implement `nivasa statechart parity` — verify generated Rust matches current SCXML
- [x] Implement `nivasa statechart visualize --format svg` — generate SVG diagrams from SCXML
- [x] Implement `nivasa statechart diff HEAD~1` — show statechart changes between commits
- [x] Implement `nivasa statechart inspect --port 3000` — query running app's statechart state

### 0.5.9 — SCXML Engine Tests
- [x] Test: Application lifecycle — Created → Bootstrapping → Running → ShuttingDown → Terminated
- [x] Test: Invalid event in Created state → panic (debug) / Err (release)
- [x] Test: Module lifecycle — full happy path
- [x] Test: Module lifecycle — load failure transitions to FailedState
- [x] Test: Provider lifecycle — full happy path
- [x] Test: Request pipeline — happy path through all states
- [x] Test: Request pipeline — guard denied → ErrorHandling → Response
- [x] Test: Request pipeline — validation error → ErrorHandling → Response
- [x] Test: Request pipeline — handler error → ErrorHandling → Response
- [x] Test: StatechartTracer receives all transition events
- [x] Test: Generated code parity — round-trip: parse SCXML → generate Rust → compile → validate transitions match

---

## Phase 1: Core Foundation (Weeks 1–2)

### 1.1 — DI Container (`nivasa-core`)

#### 1.1.1 — Provider Types & Traits
- [x] Define `Provider` trait (interface for all providers)
- [x] Define `ProviderScope` enum: `Singleton`, `Scoped`, `Transient`
- [x] Define `ProviderMetadata` struct (type id, scope, factory fn, dependencies list)
- [x] Implement `ProviderRegistry` to store provider metadata keyed by `TypeId`
- [x] Define `FactoryProvider` — register a provider via closure/factory fn
- [x] Define `ValueProvider` — register a pre-built instance directly
- [x] Define `ClassProvider` — register a type to be constructed by the container

#### 1.1.2 — Dependency Container
- [x] Implement `DependencyContainer` struct
- [x] Implement `register<T: Injectable>()` — register a provider by type
- [x] Implement `register_value<T>(instance: T)` — register an existing value
- [x] Implement `register_factory<T>(factory: F)` — register a factory closure
- [x] Implement `resolve<T>() -> Result<Arc<T>, DiError>` — resolve a provider
- [x] Implement singleton caching (resolve once, return `Arc` clone)
- [x] Implement scoped provider support (per-request `ScopeGuard`)
- [x] Implement transient provider support (new instance per `resolve`)
- [x] Implement `has<T>() -> bool` — check if provider is registered
- [x] Implement `remove<T>()` — deregister a provider
- [x] Implement `Container::create_scope()` — create child scope for request-scoped DI

#### 1.1.3 — Circular Dependency Detection
- [x] Build dependency graph from provider registrations (adjacency list)
- [x] Implement topological sort for initialization order
- [x] Detect cycles via DFS and emit clear compile-time or startup error messages
- [x] Include the full cycle path in error messages (e.g., `A -> B -> C -> A`)
- [x] Write unit tests: simple cycle, transitive cycle, diamond (no cycle), self-cycle

#### 1.1.4 — Optional & Lazy Dependencies
- [x] Support `Option<Arc<T>>` injection (resolves to `None` if missing)
- [x] Support `Lazy<Arc<T>>` injection (resolves on first access, breaks cycles)
- [x] Write tests for optional dependency resolution
- [x] Write tests for lazy dependency resolution

#### 1.1.5 — `#[injectable]` Attribute Macro (in `nivasa-macros`)
- [x] Parse struct definition annotated with `#[injectable]`
- [x] Parse optional scope: `#[injectable(scope = "transient")]`
- [x] Extract `#[inject]` fields and their types
- [x] Generate `impl Injectable for T` with `fn build(container: &Container) -> Result<Self>`
- [x] Generate provider registration code (auto-register with container)
- [x] Handle generics in injectable structs (bounded or monomorphized)
- [x] Emit clear compile error if `#[inject]` is used on non-Arc field
- [x] Write macro expansion tests using `trybuild`

#### 1.1.6 — DI Container Unit Tests
- [x] Test basic singleton registration and resolution
- [x] Test scoped provider — same instance within scope, different across scopes
- [x] Test transient provider — new instance every resolve
- [x] Test resolution failure with clear error when provider not registered
- [x] Test optional dependency resolves `None` when missing, `Some` when present
- [x] Test multiple providers depending on shared singleton (diamond pattern)
- [x] Test `register_value` with pre-built instance
- [x] Test `register_factory` with closure

### 1.2 — Module System (`nivasa-core` + `nivasa-macros`)

> ⚠️ **SCXML Rule:** The module lifecycle is driven by `statecharts/nivasa.module.scxml`.
> All module state transitions MUST go through the `StatechartEngine<ModuleStatechart>`.
> Adding a new lifecycle state requires updating the SCXML first → rebuild → implement new handler.

#### 1.2.1 — Module Trait
- [ ] Define `Module` trait with `fn configure(&self, container: &mut DependencyContainer)`
- [x] Define `ModuleMetadata` struct: `imports`, `controllers`, `providers`, `exports`
- [x] Define `OnModuleInit` trait with `async fn on_module_init(&self)`
- [x] Define `OnModuleDestroy` trait with `async fn on_module_destroy(&self)`
- [x] Define `OnApplicationBootstrap` trait (fires after all modules init)
- [x] Define `OnApplicationShutdown` trait (fires before modules destroy)

#### 1.2.2 — `#[module]` Attribute Macro (in `nivasa-macros`)
- [x] Parse `#[module({ imports: [...], controllers: [...], providers: [...], exports: [...] })]`
- [x] Validate attribute syntax and emit helpful errors on typos
- [x] Generate `impl Module for T` with metadata accessor methods
- [x] Generate provider registration calls for listed providers
- [x] Generate controller registration calls
- [ ] Generate import resolution (pull in imported module's exported providers)
- [ ] Generate export filtering (only exports are visible to importing modules)
- [x] Support `middlewares: [...]` in module config

#### 1.2.3 — Dynamic Modules (NestJS `forRoot` / `forFeature`)
- [ ] Define `DynamicModule` struct (metadata + extra providers)
- [ ] Implement `ConfigurableModule` trait with `fn for_root(options) -> DynamicModule`
- [ ] Implement `fn for_feature(options) -> DynamicModule`
- [ ] Support `is_global: true` to make a dynamic module globally available
- [ ] Test dynamic module with `for_root` provides config to all consumers
- [ ] Test `for_feature` creates isolated instance per importing module

#### 1.2.4 — Module Registry & Dependency Graph
- [x] Implement `ModuleRegistry` to track all registered modules
- [x] Build module dependency graph from `imports` lists
- [x] Resolve initialization order via topological sort
- [x] Detect circular module imports and emit clear error
- [x] Support `@Global()` equivalent — module's exports available everywhere

#### 1.2.5 — Module Initialization Lifecycle (driven by `nivasa.module.scxml`)
- [x] Create a `StatechartEngine<ModuleStatechart>` per module instance
- [x] Implement ordered module initialization (deepest dependency first)
- [x] Module enters `Loading` state → engine sends `module.load` event
- [x] Call `OnModuleInit` hooks as the `<onentry>` of the `Initialized` state
- [x] Call `OnApplicationBootstrap` after ALL module engines reach `Active` state
- [x] On shutdown: engine sends `module.destroy` event → `Destroying` state → `<onentry>` calls `OnModuleDestroy`
- [x] Call `OnModuleDestroy` hooks in reverse initialization order
- [x] Implement module-scoped DI containers (provider encapsulation)
- [x] **Verify:** invalid lifecycle transitions (e.g., `Active` → `Loading`) are rejected by the engine

#### 1.2.6 — Import / Export Resolution
- [x] Implement export filtering — non-exported providers are invisible to importers
- [x] Implement transitive import resolution
- [x] Test importing a module and accessing its exported provider
- [x] Test that non-exported providers are NOT accessible (compile/runtime error)
- [x] Test re-exporting an imported module's provider

#### 1.2.7 — Module System Unit Tests
- [x] Test simple module with one provider
- [x] Test module with imports and exports
- [x] Test nested modules (A imports B imports C)
- [x] Test lifecycle hooks fire in correct order
- [x] Test circular module import detection
- [x] Test global module (available everywhere without explicit import)
- [x] Test dynamic module via `for_root`
- [x] Test `for_feature` creates isolated instance per importing module

---

## Phase 2: Routing and Controllers (Weeks 3–4)

### 2.1 — Controller System (`nivasa-routing` + `nivasa-macros`)

#### 2.1.1 — `#[controller]` Attribute Macro
- [x] Parse `#[controller("/path")]` on struct
- [x] Store route prefix metadata on the struct
- [x] Support versioned controller: `#[controller({ path: "/users", version: "1" })]`
- [x] Generate controller trait impl with prefix accessor

#### 2.1.2 — HTTP Method Attributes
- [x] Implement `#[get("/path")]` attribute macro
- [x] Implement `#[post("/path")]` attribute macro
- [x] Implement `#[put("/path")]` attribute macro
- [x] Implement `#[delete("/path")]` attribute macro
- [x] Implement `#[patch("/path")]` attribute macro
- [x] Implement `#[head("/path")]` attribute macro
- [x] Implement `#[options("/path")]` attribute macro
- [x] Implement `#[all("/path")]` (match any HTTP method)

#### 2.1.3 — `#[impl_controller]` Macro
- [x] Parse `impl` block annotated with `#[impl_controller]`
- [x] Discover all methods with HTTP method attributes
- [x] Generate route registration for each handler method
- [x] Combine controller prefix with method path
- [x] Validate no duplicate routes within a controller

#### 2.1.4 — Parameter Extraction
> ⚠️ **SCXML / controller boundary:** the request pipeline still stops at route dispatch. The landed controller runtime slices are `#[body]` request extraction, `#[req]` raw request access, `#[param("name")]` path-param extraction, `#[query]` full query DTO extraction, and `#[res]` response-builder access; the remaining controller markers stay partial or pending until the later SCXML handler-execution path lands.

- [x] Strip and record controller parameter extractor metadata in `#[impl_controller]`
- [x] Implement `#[body]` extractor — deserialize JSON request body to typed DTO
- [x] Implement `#[param("name")]` extractor — extract path parameter
- [x] Implement `#[query]` extractor — deserialize full query string to struct
- [x] Implement `#[query("name")]` extractor — extract single query param
- [x] Implement `#[headers]` extractor — access all request headers as map
- [x] Implement `#[header("name")]` extractor — extract single header value
- [x] Implement `#[req]` extractor — raw `NivasaRequest` access
- [x] Implement `#[res]` extractor — first runtime slice for mutable response builder access
- [x] Implement `#[ip]` extractor — client IP address
- [x] Implement `#[session]` extractor — session data (if session module loaded)
- [x] Implement `#[file]` / `#[files]` extractor — multipart file upload
- [x] Support custom parameter decorators: `#[custom_param(MyExtractor)]`

#### 2.1.5 — Route Registration & Matching
- [x] Implement `RouteRegistry` to store all routes
- [x] Implement path matching: static segments (`/users`)
- [x] Implement path matching: named parameters (`/users/:id`)
- [x] Implement path matching: wildcard / catch-all (`/files/*path`)
- [x] Implement path matching: optional segments (`/users/:id?`)
- [x] Implement route conflict detection (duplicate routes → startup error)
- [x] Implement route ordering (static > parameterized > optional > wildcard)
- [x] Implement route prefix merging: global prefix + controller prefix + method path

#### 2.1.6 — Response Types
- [x] Implement JSON response (auto-serialize via Serde)
- [x] Implement plain text response
- [x] Implement HTML response
- [x] Implement streaming response bodies
- [x] Implement SSE response helper
- [x] Implement file download response
- [x] Implement redirect response (301, 302, 307, 308)
- [x] Implement `HttpStatus` enum for all standard status codes
- [x] Implement `Result<T, HttpException>` return type handling
- [x] Implement `#[http_code(201)]` to override default status code
- [x] Implement `#[header("key", "value")]` to set response headers

#### 2.1.7 — API Versioning
- [x] Support URI versioning: `/v1/users`, `/v2/users`
- [x] Support header versioning: `X-API-Version: 1`
- [x] Support media type versioning: `Accept: application/vnd.app.v1+json`
- [x] Expose `VersioningOptions` on the bootstrap/config surface via `AppBootstrapConfig`
- [x] Test versioned routes resolve correctly

#### 2.1.8 — Controller System Tests
- [x] Test basic GET route registration and invocation
- [x] Test POST route with JSON body extraction
- [x] Test path parameter extraction and type coercion
- [x] Test query parameter extraction (single + struct)
- [x] Test multiple routes on one controller
- [x] Test controller prefix concatenation
- [x] Test 404 for unmatched routes
- [x] Test 405 for wrong HTTP method on existing path
- [x] Test route conflict detection at startup
- [x] Test versioned routes

### 2.2 — HTTP Server Integration (`nivasa-http`)

#### 2.2.1 — Server Core
- [x] Add `hyper` + `hyper-util` dependencies
- [x] Implement `NivasaServer` struct with builder pattern
- [x] Implement `listen(port, host)` to start HTTP server on Tokio runtime
- [x] Implement graceful shutdown via `tokio::signal` (SIGTERM, SIGINT, Ctrl+C)
- [x] Implement configurable request body size limit
- [x] Implement configurable request timeout
- [x] Implement optional TLS via `rustls` (feature-gated)

#### 2.2.2 — Request / Response Wrappers
- [x] Implement `NivasaRequest` wrapping `http::Request<Body>` with convenience methods
- [x] Implement `NivasaResponse` wrapping `http::Response<Body>` with builder
- [x] Implement `FromRequest` trait for custom extractors
- [x] Implement `IntoResponse` trait for custom response types
- [x] Implement `Body` abstraction (streaming, collected, empty)

#### 2.2.3 — Request Pipeline (Execution Order — driven by `nivasa.request.scxml`)

> ⚠️ **SCXML Rule:** The request pipeline is driven by `statecharts/nivasa.request.scxml`.
> A `StatechartEngine<RequestStatechart>` is created per request. Each pipeline stage is a state.
> Each handler returns a `RequestEvent` which drives the transition. The engine rejects invalid transitions.

- [x] Document the full request lifecycle (reference the SCXML statechart diagram)
- [x] Create a `StatechartEngine<RequestStatechart>` per incoming request
- [x] Drive pipeline via engine: `Received` → event → `MiddlewareChain` → event → `RouteMatching` (route dispatch is the current SCXML stop; the first `#[res]` runtime slice begins on the controller side, and full controller execution plus later SCXML stages remain future work) → ...
- [x] Each pipeline stage handler returns a `RequestEvent` that the engine uses to transition
- [x] Pipeline short-circuits are SCXML transitions: GuardDenied → `ErrorHandling` (not ad-hoc if/else)
- [x] Errors at any stage raise `error.*` events → engine transitions to `ErrorHandling` state
- [x] **Verify:** attempting to skip a pipeline stage (e.g., jump from Middleware to Handler) is rejected by the engine

#### 2.2.4 — Multipart / File Upload
- [x] Add `multer` crate dependency for multipart parsing
- [x] Implement `UploadedFile` struct (filename, content_type, bytes)
- [x] Implement `FileInterceptor` (single file)
- [x] Implement `FilesInterceptor` (multiple files)
- [x] Implement configurable file size limits
- [x] Implement configurable allowed MIME types

#### 2.2.5 — HTTP Server Tests
- [x] Test server starts and responds to GET /
- [x] Test graceful shutdown completes in-flight requests
- [x] Test request body parsing (JSON)
- [x] Test response serialization (JSON, text, HTML)
- [x] Test 404 for unknown routes
- [x] Test request body size limit enforcement
- [x] Test file upload via multipart
  - verified with `PATH=/opt/homebrew/bin:$PATH cargo test -p nivasa-http --test upload_contract --test upload_limits --test upload_interceptors`

---

## Phase 3: Middleware and Guards (Weeks 5–6)

### 3.1 — Guard System (`nivasa-guards` + `nivasa-macros`)

> Shared context note: `nivasa-common::RequestContext` is now the canonical per-request context foundation; the guard runtime surface can converge onto it via the existing adapter path in later slices.

#### 3.1.1 — Guard Trait
- [x] Define `Guard` trait: `async fn can_activate(&self, context: &ExecutionContext) -> Result<bool, HttpException>`
- [x] Define `ExecutionContext` struct (request, handler metadata, class metadata, custom data map)
- [ ] Support DI in guard structs (guards are injectable)

#### 3.1.2 — `#[guard]` Attribute Macro
- [x] Parse `#[guard(GuardType)]` on handler methods
- [x] Parse `#[guard(GuardType)]` on controller struct (metadata capture only; runtime apply-to-all-routes still open)
- [x] Parse `#[guard(GuardType)]` on module (metadata capture only; runtime apply to all module routes still open)
- [x] Support multiple guards: `#[guard(Guard1, Guard2)]` (metadata capture)

#### 3.1.3 — Guard Execution Pipeline
- [x] Implement guard chain execution (AND logic: all must pass)
- [x] Implement short-circuit on first failure
- [ ] Return `ForbiddenException` on guard failure (configurable)
- [x] Support guard returning custom exception on failure
- [x] Support async guard execution

#### 3.1.4 — Reflector / Metadata (NestJS `SetMetadata`)
- [x] Implement `#[set_metadata(key, value)]` decorator (metadata capture only; handler/controller/module capture landed; runtime Reflector/guard enforcement still open)
- [x] Implement `Reflector` service — read metadata in guards/interceptors
- [x] Implement `#[roles("admin", "editor")]` as sugar over `set_metadata` (metadata capture only; handler/controller/module capture landed; runtime authorization and module-wide enforcement still open)
- [x] Test reflector reads metadata set on handler

#### 3.1.5 — Built-in Guards
- [ ] Implement `AuthGuard` skeleton (JWT validation pattern)
- [x] Implement `RolesGuard` (check roles via Reflector + `#[roles(...)]`)
- [ ] Implement `ThrottlerGuard` (rate limiting — see Phase 3.4)

#### 3.1.6 — Guard Tests
- [x] Test guard that always allows → handler executes
- [x] Test guard that always denies → 403 response
- [x] Test multiple guards — all pass
- [x] Test multiple guards — one fails → short-circuit
- [x] Test guard with injected service dependency
- [x] Test controller-level guard applies to all its routes
- [x] Test controller guard metadata applies to every route
- [x] Test reflector reads `#[roles]` metadata correctly

### 3.2 — Interceptor System (`nivasa-interceptors` + `nivasa-macros`)

> Shared context note: `nivasa-common::RequestContext` is now the canonical per-request context foundation; the interceptor runtime surface should converge onto it via the existing adapter path in later slices.

#### 3.2.1 — Interceptor Trait
- [x] Define `Interceptor` trait: `async fn intercept(&self, context: &ExecutionContext, next: CallHandler) -> Result<Response>`
- [x] Define `CallHandler` struct: `async fn handle(self) -> Result<Response>`
- [ ] Support DI in interceptor structs

#### 3.2.2 — `#[interceptor]` Attribute Macro
- [x] Parse `#[interceptor(InterceptorType)]` on handler methods
- [x] Parse `#[interceptor(InterceptorType)]` on controller struct
- [x] Parse `#[interceptor(InterceptorType)]` on module (metadata capture only; runtime wiring still open)
- [x] Support multiple interceptors: `#[interceptor(I1, I2)]` (execute in order)

#### 3.2.3 — Interceptor Chain Execution
- Landed execution slices: `NivasaServerBuilder::interceptor(...)` now supports a thin server-side interceptor hook around matched route handlers, repeated `.interceptor(...)` calls execute as an ordered onion chain while `RequestPipeline` remains the owner of `InterceptorPre` / `InterceptorPost` transitions, `AppBootstrapConfig::use_interceptor(...)` now forwards into that hook, and the response-mapping hook now wraps mapped bodies before final send. Decorator-driven registration and module wiring remain open.
- [ ] Implement interceptor chain (onion/RxJS-style: pre → next.handle() → post)
- [x] Implement response transformation in post-processing
- [x] Implement response mapping (map the body before sending)
- [x] Support async interceptor execution

#### 3.2.4 — Built-in Interceptors
- [ ] Implement `LoggingInterceptor` (log method, path, status, duration)
- [x] Implement `TimeoutInterceptor` (fail with 408 after N ms via `tokio::time::timeout`)
- [ ] Implement `CacheInterceptor` (in-memory TTL cache, skip handler on cache hit)
- [ ] Implement `ClassSerializerInterceptor` (transform response using `#[exclude]` / `#[expose]` on fields)

#### 3.2.5 — Interceptor Tests
- [ ] Test pre-processing interceptor adds header to request
- [x] Test post-processing interceptor wraps response in `{ data: ... }`
- [x] Test interceptor chain execution order (I1.pre → I2.pre → handler → I2.post → I1.post)
- [x] Test timeout interceptor returns 408 on slow handler
- [ ] Test cache interceptor returns cached response on second call

### 3.3 — Middleware System (`nivasa-http` + `nivasa-macros`)

#### 3.3.1 — Middleware Trait
- [x] Define `NivasaMiddleware` trait: `async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse`
- [ ] Support DI in middleware structs (`#[inject]` on fields)
- [x] Support functional middleware (closure-based, no struct needed)

#### 3.3.2 — `#[middleware]` Attribute Macro
- [x] Parse `#[middleware]` on struct
- [x] Generate middleware registration

#### 3.3.3 — Middleware Pipeline
- Landed execution slice: `NivasaServerBuilder::middleware(...)` runs one `NivasaMiddleware` around a `NextMiddleware` capture point before `complete_middleware()`. `AppBootstrapConfig::use_middleware(...)` now forwards straight into that hook, while module-level wiring, route ordering, and exclusion remain open.
- [x] Implement global middleware registration via `NestApplication::use_()` (bootstrap-only facade via `AppBootstrapConfig::use_middleware(...)`)
- [ ] Implement module-level middleware registration via `#[module({ middlewares: [...] })]`
- [ ] Implement route-specific middleware (`.apply(Mw).forRoutes("/users")`)
- [ ] Implement middleware exclusion (`.apply(Mw).exclude("/health")`)
- [ ] Implement middleware execution order: global → module → route-specific

#### 3.3.4 — Tower Compatibility Layer
- [ ] Implement adapter: `Tower Service<Request> → NivasaMiddleware`
- [ ] Implement adapter: `NivasaMiddleware → Tower Layer`
- [ ] Test wrapping a Tower middleware (e.g., `tower-http::cors`) for future Nivasa middleware support
- [ ] Document how to use existing Tower ecosystem middleware

#### 3.3.5 — Built-in Middleware
- [ ] Implement `CorsMiddleware` (configurable origins, methods, headers, credentials)
- [ ] Implement `HelmetMiddleware` (security headers: CSP, HSTS, X-Frame-Options, etc.)
- [ ] Implement `CompressionMiddleware` (gzip, brotli, deflate — feature-gated)
- [ ] Implement `RequestIdMiddleware` (generate/propagate `X-Request-Id` header)
- [ ] Implement `LoggerMiddleware` (structured request logging via `tracing`)

#### 3.3.6 — Middleware Tests
- [ ] Test global middleware runs on every request
- [ ] Test module-level middleware runs only for that module's routes
- [ ] Test middleware ordering (global before module before route)
- [ ] Test richer CORS middleware/CorsOptions integration adds correct headers
- [ ] Test middleware exclusion (`.exclude()`)
- [ ] Test Tower middleware adapter works

### 3.4 — Rate Limiting / Throttling

- [ ] Implement `ThrottlerModule` (configurable: TTL, limit per window)
- [ ] Implement `ThrottlerGuard` (apply via `#[guard(ThrottlerGuard)]`)
- [ ] Implement in-memory store (default)
- [ ] Define `ThrottlerStorage` trait for pluggable backends (Redis, etc.)
- [ ] Implement `#[throttle(limit = 10, ttl = 60)]` per-route override
- [ ] Implement `#[skip_throttle]` to exempt specific routes
- [ ] Test rate limit enforcement (N+1th request returns 429)
- [ ] Test per-route override

---

## Phase 4: Pipes and Validation (Weeks 7–8)

### 4.1 — Pipe System (`nivasa-pipes` + `nivasa-macros`)

#### 4.1.1 — Pipe Trait
- [ ] Define `Pipe` trait: `fn transform(&self, value: Value, metadata: ArgumentMetadata) -> Result<Value, HttpException>`
- [ ] Define `ArgumentMetadata` struct (param name, metatype, data type, index)
- [ ] Support DI in pipe structs

#### 4.1.2 — `#[pipe]` Attribute Macro
- [ ] Parse `#[pipe(PipeType)]` on handler methods (applies to all params)
- [ ] Parse `#[pipe(PipeType)]` on individual parameters
- [ ] Parse `#[pipe(PipeType)]` on controller (applies to all handlers)
- [ ] Support pipe chaining: `#[pipe(Pipe1, Pipe2)]` (left to right)

#### 4.1.3 — Built-in Pipes
- [ ] Implement `ValidationPipe` (validate DTO fields, return 400 with error details)
- [ ] Implement `ParseIntPipe` (parse string to `i32`/`i64`, 400 on failure)
- [ ] Implement `ParseFloatPipe` (parse string to `f32`/`f64`)
- [ ] Implement `ParseBoolPipe` (parse string to `bool`)
- [ ] Implement `ParseUuidPipe` (parse string to `Uuid`)
- [ ] Implement `ParseEnumPipe` (parse string to enum variant)
- [ ] Implement `DefaultValuePipe` (provide default if value is missing/null)
- [ ] Implement `TrimPipe` (trim whitespace from string values)

#### 4.1.4 — Pipe Tests
- [ ] Test ParseIntPipe with valid input → returns i32
- [ ] Test ParseIntPipe with "abc" → 400 with message
- [ ] Test ValidationPipe with valid DTO → passes through
- [ ] Test ValidationPipe with invalid DTO → 400 with field-level errors
- [ ] Test pipe chaining (TrimPipe → ValidationPipe)
- [ ] Test ParseUuidPipe with valid/invalid UUID
- [ ] Test DefaultValuePipe provides fallback

### 4.2 — Validation Integration (`nivasa-validation`)

#### 4.2.1 — Validation Decorators (Attribute Macros)
- [ ] Implement `#[is_email]` — validate email format
- [ ] Implement `#[is_string]` — validate is string type
- [ ] Implement `#[is_number]` — validate is numeric type
- [ ] Implement `#[is_int]` — validate is integer
- [ ] Implement `#[is_boolean]` — validate is boolean
- [ ] Implement `#[min(n)]` — minimum value (for numbers)
- [ ] Implement `#[max(n)]` — maximum value (for numbers)
- [ ] Implement `#[min_length(n)]` — minimum string/array length
- [ ] Implement `#[max_length(n)]` — maximum string/array length
- [ ] Implement `#[is_not_empty]` — validate non-empty string/vec
- [ ] Implement `#[matches(regex)]` — regex pattern match
- [ ] Implement `#[is_optional]` — field is optional (skip if absent)
- [ ] Implement `#[is_enum(EnumType)]` — validate value is valid enum variant
- [ ] Implement `#[is_url]` — validate URL format
- [ ] Implement `#[is_uuid]` — validate UUID format
- [ ] Implement `#[array_min_size(n)]` / `#[array_max_size(n)]`
- [ ] Implement `#[validate_nested]` — validate nested DTO recursively
- [ ] Implement `#[custom_validate(fn)]` — custom validation function

#### 4.2.2 — Validation Engine
- [ ] Integrate `validator` crate or build custom validation engine
- [ ] Collect ALL validation errors for a DTO (don't fail on first)
- [ ] Format validation errors as structured JSON: `{ field, constraints: { rule: message } }`
- [ ] Support nested DTO validation (recursive)
- [ ] Support `Vec<T>` element validation
- [ ] Support conditional validation (validate field X only if field Y has value Z)
- [ ] Support validation groups (e.g., "create" vs "update" different rules)

#### 4.2.3 — DTO Derive Macro
- [ ] Implement `#[derive(Dto)]` to auto-generate `Validate` impl
- [ ] Generate `validate() -> Result<(), Vec<ValidationError>>` from annotated fields
- [ ] Support `#[derive(PartialDto)]` for patch/update operations (all fields optional)

#### 4.2.4 — Validation Tests
- [ ] Test `#[is_email]` with valid and invalid emails
- [ ] Test `#[min_length(6)]` on password field
- [ ] Test multiple validation errors returned together
- [ ] Test nested DTO validation
- [ ] Test optional field skips validation when absent
- [ ] Test `#[validate_nested]` on vec of DTOs
- [ ] Test custom validation function

---

## Phase 5: Exception Handling (Weeks 9–10)

### 5.1 — Exception Filters (`nivasa-filters` + `nivasa-macros`)

#### 5.1.1 — ExceptionFilter Trait
- [ ] Define `ExceptionFilter<E>` trait: `async fn catch(&self, exception: E, host: ArgumentsHost) -> NivasaResponse`
- [ ] Define `ArgumentsHost` struct (access to request, response, next, underlying context)
- [ ] Define `HttpArgumentsHost` for HTTP-specific context
- [ ] Define `WsArgumentsHost` for WebSocket-specific context (future)

#### 5.1.2 — `#[catch]` Attribute Macro
- [ ] Parse `#[catch(ExceptionType)]` on filter struct
- [ ] Parse `#[catch_all]` to catch any exception
- [ ] Support handler-level: `#[use_filters(MyFilter)]`
- [ ] Support controller-level: `#[use_filters(MyFilter)]`
- [ ] Support global filters via `NestApplication::use_global_filter()`

#### 5.1.3 — Filter Execution
- [ ] Implement filter matching by exception type (most specific first)
- [ ] Implement filter precedence: handler → controller → global
- [ ] Implement fallback filter for completely unhandled exceptions (500 + log)
- [ ] Ensure filters can themselves throw (caught by next-level filter)

#### 5.1.4 — Built-in Filters
- [ ] Implement `HttpExceptionFilter` (catch all `HttpException` variants)
- [ ] Implement default global filter (standard error response shape)

#### 5.1.5 — Filter Tests
- [ ] Test global filter catches unhandled HttpException
- [ ] Test handler-level filter overrides global for specific exception
- [ ] Test filter formats response correctly (`{ statusCode, message, error }`)
- [ ] Test unhandled non-HttpException returns 500 Internal Server Error
- [ ] Test filter has access to request via ArgumentsHost

### 5.2 — Custom Exceptions (`nivasa-common`)

#### 5.2.1 — Base Exception Types
- [ ] Implement `HttpException` base struct (status: u16, message: String, description: Option<String>)
- [ ] Derive `thiserror::Error` for all exception types
- [ ] Implement `BadRequestException` (400)
- [ ] Implement `UnauthorizedException` (401)
- [ ] Implement `PaymentRequiredException` (402)
- [ ] Implement `ForbiddenException` (403)
- [ ] Implement `NotFoundException` (404)
- [ ] Implement `MethodNotAllowedException` (405)
- [ ] Implement `NotAcceptableException` (406)
- [ ] Implement `RequestTimeoutException` (408)
- [ ] Implement `ConflictException` (409)
- [ ] Implement `GoneException` (410)
- [ ] Implement `PayloadTooLargeException` (413)
- [ ] Implement `UnsupportedMediaTypeException` (415)
- [ ] Implement `UnprocessableEntityException` (422)
- [ ] Implement `TooManyRequestsException` (429)
- [ ] Implement `InternalServerErrorException` (500)
- [ ] Implement `NotImplementedException` (501)
- [ ] Implement `BadGatewayException` (502)
- [ ] Implement `ServiceUnavailableException` (503)
- [ ] Implement `GatewayTimeoutException` (504)

#### 5.2.2 — Exception Serialization
- [ ] Implement `Serialize` for `HttpException`
- [ ] Implement standard error response shape: `{ statusCode, message, error }`
- [ ] Support custom error details/payload via `.with_details(json!(...))`
- [ ] Support error cause chaining (`.with_cause(inner_error)`)

#### 5.2.3 — Exception Tests
- [ ] Test each exception type returns correct status code
- [ ] Test exception serialization to JSON matches expected shape
- [ ] Test custom exception with additional details
- [ ] Test `Display` / `Error` trait implementations

---

## Phase 6: Configuration, Logging & Testing (Weeks 11–12)

### 6.1 — Configuration Module (`nivasa-config`)

#### 6.1.1 — ConfigModule
- [ ] Implement `ConfigModule` struct
- [ ] Implement `ConfigModule::for_root(options: ConfigOptions) -> DynamicModule`
- [ ] Implement `ConfigModule::for_feature(options: ConfigOptions) -> DynamicModule`
- [ ] Support `is_global: true` (register ConfigService globally)
- [ ] Support `env_file_path: ".env"` option (single or vec of paths)
- [ ] Support `ignore_env_file: true` (only use process env vars)
- [ ] Support `validate_config: schema` (validate config at startup)

#### 6.1.2 — Environment Loading
- [ ] Support `.env` file loading via `dotenvy` crate
- [ ] Support multiple env files: `.env`, `.env.local`, `.env.development`, `.env.production`
- [ ] Support env variable override order: process env > .env.local > .env.{NODE_ENV} > .env
- [ ] Support `expand_variables: true` (variable interpolation in .env: `URL=$HOST:$PORT`)
- [ ] Support custom env file path

#### 6.1.3 — ConfigService
- [ ] Implement `ConfigService` as injectable provider
- [ ] Implement `get<T: FromStr>(key: &str) -> Option<T>` with type coercion
- [ ] Implement `get_or_default<T>(key: &str, default: T) -> T`
- [ ] Implement `get_or_throw(key: &str) -> Result<String, ConfigException>`
- [ ] Implement namespace support: `get("database.host")`
- [ ] Implement validation of required config keys at startup

#### 6.1.4 — Type-Safe Config (Config Schema)
- [ ] Support config schema definition via `#[derive(ConfigSchema)]`
- [ ] Auto-validate loaded config against schema at module init
- [ ] Emit clear startup error listing all missing/invalid config keys
- [ ] Support default values in schema

#### 6.1.5 — Config Tests
- [ ] Test loading from .env file
- [ ] Test process env variable overrides .env
- [ ] Test `get::<i32>` type coercion
- [ ] Test `get::<bool>` type coercion
- [ ] Test `get_or_throw` with missing key → startup error
- [ ] Test global config is accessible from any module
- [ ] Test config schema validation at startup

### 6.2 — Structured Logging (`tracing` integration)

- [ ] Add `tracing` + `tracing-subscriber` as workspace dependencies
- [ ] Implement `LoggerModule` with configurable log levels
- [ ] Implement `LoggerService` injectable provider wrapping `tracing`
- [ ] Support structured JSON logging (for production)
- [ ] Support pretty console logging (for development)
- [ ] Support log context propagation (request ID, user ID, module name)
- [ ] Implement request logging span (method, path, status, duration)
- [ ] Support configurable log levels per module
- [ ] Test log output contains expected fields
- [ ] Test log level filtering

### 6.3 — Testing Utilities (`nivasa-testing` or `nivasa` main crate)

#### 6.3.1 — Test Application Builder
- [ ] Implement `Test::create_testing_module(metadata)` builder
- [ ] Implement `.override_provider::<T>().use_value(mock)` for mock injection
- [ ] Implement `.override_provider::<T>().use_factory(|| mock)` for factory mock
- [ ] Implement `.compile() -> TestingModule` to build test DI container
- [ ] Implement `testing_module.get::<T>()` to resolve providers in tests

#### 6.3.2 — HTTP Test Client
- [ ] Implement `TestClient` struct wrapping in-memory HTTP dispatch (no TCP)
- [ ] Implement `.get("/path")`, `.post("/path")`, `.put("/path")`, `.delete("/path")`
- [ ] Implement `.header("key", "value")` — set request headers
- [ ] Implement `.body(json)` — set request body
- [ ] Implement `.send() -> TestResponse`
- [ ] Implement `TestResponse::status() -> u16`
- [ ] Implement `TestResponse::json::<T>() -> T`
- [ ] Implement `TestResponse::text() -> String`
- [ ] Implement `TestResponse::header("key") -> Option<String>`

#### 6.3.3 — Mock Providers
- [ ] Implement `MockProvider<T>` utility
- [ ] Support recording calls (method name, arguments)
- [ ] Support returning predefined values
- [ ] Support asserting call counts
- [ ] Support asserting call arguments

#### 6.3.4 — Testing Tests
- [ ] Test creating a testing module with mock providers
- [ ] Test HTTP test client sends and receives correctly
- [ ] Test provider override replaces real provider with mock
- [ ] Test e2e test flow: create module → HTTP client → assert response

### 6.4 — CLI Tool (`nivasa-cli`)

#### 6.4.1 — CLI Core
- [ ] Add `clap` dependency for argument parsing (derive API)
- [ ] Implement `nivasa new <project-name>` — scaffold new project (includes `statecharts/` directory with default SCXML files)
- [ ] Implement `nivasa generate module <name>` (alias: `nivasa g module <name>`)
- [ ] Implement `nivasa generate controller <name>` (alias: `nivasa g controller <name>`)
- [ ] Implement `nivasa generate service <name>` (alias: `nivasa g service <name>`)
- [ ] Implement `nivasa generate guard <name>`
- [ ] Implement `nivasa generate interceptor <name>`
- [ ] Implement `nivasa generate pipe <name>`
- [ ] Implement `nivasa generate filter <name>`
- [ ] Implement `nivasa generate resource <name>` (full CRUD: module + controller + service + DTOs)
- [ ] Implement `nivasa generate middleware <name>`
- [ ] Implement `nivasa info` — print framework version, Rust version, OS info
- [ ] Implement `nivasa statechart validate --all` — validate all SCXML files in project
- [ ] Implement `nivasa statechart visualize` — generate diagrams from SCXML
- [ ] Implement `nivasa statechart parity` — check generated code matches SCXML
- [ ] Implement `nivasa statechart diff` — show SCXML changes between commits

#### 6.4.2 — Project Scaffolding Templates
- [ ] Create template for new project: `Cargo.toml`, `main.rs`, `app_module.rs`, `.env`, `.gitignore`
- [ ] Create template for module file
- [ ] Create template for controller file (with example GET route)
- [ ] Create template for service file (with injectable annotation)
- [ ] Create template for guard file
- [ ] Create template for interceptor/pipe/filter files
- [ ] Create template for resource: module + controller + service + create DTO + update DTO
- [ ] Use `askama` or string templates for code generation

#### 6.4.3 — CLI Auto-Registration
- [ ] After generating a module, auto-add import to parent module's `imports` list
- [ ] After generating a controller, auto-add to module's `controllers` list
- [ ] After generating a service, auto-add to module's `providers` list
- [ ] Handle file parsing to find insertion point (regex or syn-based)

#### 6.4.4 — CLI Tests
- [ ] Test `nivasa new myapp` creates correct project structure
- [ ] Test `nivasa g module users` creates `users/users_module.rs`
- [ ] Test `nivasa g resource users` creates module + controller + service + DTOs
- [ ] Test auto-registration modifies parent module correctly
- [ ] Test `nivasa info` outputs version information

---

## Phase 7: Advanced Features (Weeks 13–14)

### 7.1 — WebSocket Support (`nivasa-websocket`)

#### 7.1.1 — WebSocket Gateway
- [ ] Implement `#[websocket_gateway("/ws")]` attribute macro
- [ ] Implement `#[websocket_gateway({ path: "/ws", namespace: "/chat" })]`
- [ ] Define `WebSocketGateway` trait
- [ ] Implement connection lifecycle events: `OnGatewayInit`, `OnGatewayConnection`, `OnGatewayDisconnect`
- [ ] Implement room/namespace support

#### 7.1.2 — WebSocket Decorators
- [ ] Implement `#[subscribe_message("event_name")]` — subscribe to named event
- [ ] Implement `#[message_body]` — extract message payload
- [ ] Implement `#[connected_socket]` — access the WebSocket client handle

#### 7.1.3 — WebSocket Adapter
- [ ] Define `WebSocketAdapter` trait for pluggable backends
- [ ] Implement default adapter using `tokio-tungstenite`
- [ ] Implement `server.emit("event", data)` — broadcast to all
- [ ] Implement `server.to("room").emit("event", data)` — emit to room
- [ ] Implement `client.emit("event", data)` — emit to specific client
- [ ] Implement `client.join("room")` / `client.leave("room")`

#### 7.1.4 — WebSocket + Guards/Pipes/Interceptors
- [ ] Support guards on WebSocket gateway methods
- [ ] Support pipes on message body extraction
- [ ] Support interceptors on WebSocket handlers

#### 7.1.5 — WebSocket Tests
- [ ] Test WebSocket connection and handshake
- [ ] Test message subscription and handler invocation
- [ ] Test broadcast to all connected clients
- [ ] Test room-based messaging
- [ ] Test disconnection cleanup

### 7.2 — Event Emitter Module

- [ ] Implement `EventEmitterModule`
- [ ] Implement `EventEmitter` injectable service
- [ ] Implement `#[on_event("event_name")]` decorator on handler methods
- [ ] Implement `event_emitter.emit("event_name", payload)` — fire event
- [ ] Support async event handlers
- [ ] Support wildcard listeners (`#[on_event("user.*")]`)
- [ ] Test event emission and handler invocation
- [ ] Test multiple handlers for same event
- [ ] Test wildcard matching

### 7.3 — Scheduling Module

- [ ] Implement `ScheduleModule`
- [ ] Implement `#[cron("0 */5 * * * *")]` decorator — cron-based scheduling
- [ ] Implement `#[interval(5000)]` decorator — run every N ms
- [ ] Implement `#[timeout(3000)]` decorator — run once after N ms
- [ ] Add `cron` crate dependency for cron expression parsing
- [ ] Support dynamic scheduling (add/remove jobs at runtime)
- [ ] Test cron job fires at expected times
- [ ] Test interval job fires repeatedly
- [ ] Test timeout job fires once

### 7.4 — Health Checks

- [ ] Implement `TerminusModule` (health check module)
- [ ] Implement `HealthCheckService` with `check()` method
- [ ] Implement `#[health_check]` on controller method (typically GET /health)
- [ ] Implement health indicators: `DiskHealthIndicator`, `MemoryHealthIndicator`
- [ ] Define `HealthIndicator` trait for custom health checks
- [ ] Support database health indicator (ping DB connection)
- [ ] Support HTTP health indicator (ping external service)
- [ ] Test health endpoint returns correct status (up/down)
- [ ] Test aggregated health with multiple indicators

### 7.5 — OpenAPI / Swagger Integration

#### 7.5.1 — OpenAPI Spec Generation
- [ ] Implement `#[api_tags("Users")]` decorator on controllers
- [ ] Implement `#[api_operation(summary = "Get all users")]` on handlers
- [ ] Implement `#[api_param(name = "id", description = "User ID")]`
- [ ] Implement `#[api_body(type = CreateUserDto)]`
- [ ] Implement `#[api_response(status = 200, type = User, description = "Success")]`
- [ ] Implement `#[api_bearer_auth]` for auth documentation
- [ ] Auto-generate OpenAPI 3.0 spec from controller/DTO metadata
- [ ] Serve spec at configurable path (default: `/api/docs/openapi.json`)

#### 7.5.2 — Swagger UI
- [ ] Bundle Swagger UI static assets (or reference CDN)
- [ ] Serve Swagger UI at configurable path (default: `/api/docs`)
- [ ] Support customizing title, description, version in Swagger UI

#### 7.5.3 — OpenAPI Tests
- [ ] Test generated spec includes all routes with correct methods
- [ ] Test spec includes request/response schemas
- [ ] Test Swagger UI endpoint serves HTML
- [ ] Test spec validates against OpenAPI 3.0 spec

### 7.6 — GraphQL Support (Optional, Deferred)

- [ ] Evaluate `async-graphql` crate for integration
- [ ] Implement `GraphQLModule` wrapping async-graphql
- [ ] Implement `#[resolver]` decorator
- [ ] Implement `#[query]`, `#[mutation]`, `#[subscription]` decorators
- [ ] Implement playground UI endpoint
- [ ] Implement federation support (stretch)

---

## Phase 8: NestApplication Entry Point (`nivasa` main crate)

- [ ] Implement `NestApplication::create(AppModule)` factory method
- [ ] Implement `.build() -> Result<App>` — resolve all modules, DI, and routes
- [ ] Implement `.listen(ServerOptions) -> Result<()>` — start HTTP server
- [x] Implement `ServerOptions` struct: `port`, `host`, `cors`, `global_prefix`, `versioning`
- [x] Introduce `AppBootstrapConfig` boundary for server-only bootstrap config
- [ ] Use `AppBootstrapConfig::global_prefix()` to prefix all routes during bootstrap
- [ ] Implement `.use_global_guard(Guard)` — apply guard to all routes
- [ ] Implement `.use_global_interceptor(Interceptor)` — apply interceptor globally
- [ ] Implement `.use_global_pipe(Pipe)` — apply pipe globally (e.g., ValidationPipe)
- [ ] Implement `.use_global_filter(Filter)` — apply exception filter globally
- [x] Implement `.enable_cors()` — minimal transport-side CORS bridge on `ServerOptions`; richer middleware/CorsOptions work remains future
- [ ] Implement `.enable_versioning(VersioningOptions)` — API versioning config
- [x] Implement `.use_(Middleware)` — apply global middleware (bootstrap-only facade via `AppBootstrapConfig::use_middleware(...)`)
- [ ] Implement startup banner with ASCII art + version
- [ ] Implement startup logging: routes registered, modules loaded, listen address
- [ ] Implement `.close()` — graceful shutdown API (for testing)

---

## Phase 9: Examples & Documentation

### 9.1 — Example Applications
- [ ] Create `examples/hello-world/` — minimal app with one GET route
- [ ] Create `examples/crud-rest-api/` — full CRUD with DTOs, validation, error handling
- [ ] Create `examples/auth-jwt/` — JWT authentication with guards, roles, protected routes
- [ ] Create `examples/websocket-chat/` — real-time chat using WebSocket gateway
- [ ] Create `examples/config-env/` — environment-based configuration
- [ ] Create `examples/testing/` — demonstrate testing utilities and mock providers
- [ ] Each example: include README with explanation and how to run

### 9.2 — Documentation
- [ ] Write "Getting Started" quickstart guide (install, hello world, run)
- [ ] Write "First Steps" tutorial (controllers, services, modules from scratch)
- [ ] Write module system deep-dive documentation
- [ ] Write DI container documentation (scopes, custom providers, lifecycle)
- [ ] Write controllers & routing documentation (all extractors, response types)
- [x] Write API versioning documentation
- [ ] Write guards documentation (including Reflector and metadata)
- [ ] Write interceptors documentation (with caching, logging examples)
- [ ] Write pipes documentation (built-in pipes, custom pipes)
- [ ] Write exception filters documentation
- [ ] Write middleware documentation (including Tower compatibility)
- [ ] Write configuration documentation (env loading, type-safe config)
- [ ] Write testing documentation (TestingModule, TestClient, mocking)
- [ ] Write CLI documentation (all generators, options)
- [ ] Write WebSocket documentation
- [ ] Write OpenAPI/Swagger documentation
- [ ] Write "Migration from NestJS" guide (NestJS pattern → Nivasa equivalent)
- [ ] Write "Comparison with other Rust frameworks" page
- [ ] Generate `rustdoc` for all public APIs (`cargo doc --workspace --no-deps`)
- [ ] Set up documentation website (mdBook or similar)
- [ ] Add search to documentation site

---

## Phase 10: Quality, Performance & Release

### 10.1 — Testing
- [ ] Achieve >90% code coverage across all crates
- [x] Add in-process request lifecycle integration coverage (middleware → guard → interceptor → handler → Done)
- [ ] Write integration tests: full request lifecycle (middleware → guard → interceptor → pipe → handler → filter)
- [ ] Write integration tests: module composition (nested modules, imports/exports)
- [x] Write integration tests: error handling pipeline (exception → filter → response)
- [ ] Write integration tests: authentication flow (login → JWT → protected route)
- [ ] Write integration tests: validation flow (invalid DTO → ValidationPipe → 400 response)
- [ ] Write integration tests: WebSocket lifecycle
- [ ] **SCXML compliance tests:** verify every state in every statechart is reachable by integration tests
- [ ] **SCXML compliance tests:** verify every error transition is exercised (guard denied, validation error, handler error, etc.)
- [ ] **SCXML compliance tests:** verify StatechartTracer log exactly matches expected transition sequence for each test scenario
- [ ] Set up mutation testing (cargo-mutants) for critical paths
- [ ] Run `cargo clippy` with all warnings as errors
- [ ] Run `cargo deny check` for license/vulnerability issues
- [ ] Run `cargo audit` for security advisories

### 10.2 — Benchmarking
- [ ] Set up benchmark harness (criterion or divan)
- [ ] Benchmark hello-world GET (JSON response) vs Actix Web
- [ ] Benchmark hello-world GET (JSON response) vs Axum
- [ ] Benchmark DI container resolution overhead (1, 10, 100 providers)
- [ ] Benchmark routing performance (10, 100, 1000 routes)
- [ ] Benchmark full middleware + guard + interceptor pipeline overhead
- [ ] Benchmark startup time with many modules
- [ ] Document benchmark results in `BENCHMARKS.md`
- [ ] Set up CI benchmark regression detection

### 10.3 — Release Preparation
- [ ] Final API review: ensure public APIs are consistent and well-named
- [ ] Ensure all public types/functions have rustdoc with examples
- [ ] Write `CHANGELOG.md` following Keep a Changelog format
- [ ] Write `CONTRIBUTING.md` with contribution guidelines
- [ ] Set up crate publishing order (dependencies first):
  1. [ ] Publish `nivasa-common`
  2. [ ] Publish `nivasa-core`
  3. [ ] Publish `nivasa-macros`
  4. [ ] Publish `nivasa-http`
  5. [ ] Publish `nivasa-routing`
  6. [ ] Publish `nivasa-guards`
  7. [ ] Publish `nivasa-interceptors`
  8. [ ] Publish `nivasa-pipes`
  9. [ ] Publish `nivasa-filters`
  10. [ ] Publish `nivasa-validation`
  11. [ ] Publish `nivasa-config`
  12. [ ] Publish `nivasa-websocket`
  13. [ ] Publish `nivasa` (umbrella crate)
  14. [ ] Publish `nivasa-cli`
- [ ] Create GitHub release with tag `v0.1.0` and changelog
- [ ] Announce v0.1.0 release (Reddit r/rust, Hacker News, Twitter/X)

---

> **Total estimated items: ~440+ granular tasks across 11 phases**
>
> **Critical path:** Phase 0 → **Phase 0.5 (SCXML engine)** → Phase 1 → Phase 2 → Phase 8 (minimal working framework)
> **The rule:** Every subsequent phase starts with authoring/updating the SCXML statechart, then implementing.
> **Parallelizable:** Phase 3, 4, 5 can be developed in parallel once Phase 2 is done
> **Deferred safely:** Phase 7 (advanced features) can ship post-v0.1.0
>
> **SCXML enforcement — four layers, zero escape hatches:**
> 1. `build.rs` codegen — SCXML → Rust enums, transition tables, handler traits (compiler enforces)
> 2. Proc macros — `#[scxml_handler]` validates state references at compile time
> 3. Runtime engine — `StatechartEngine::send_event()` is the only way to transition (no `set_state()`)
> 4. CI pipeline — `nivasa statechart validate` + `nivasa statechart parity` block PRs with drift
