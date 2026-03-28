# Docs

Documentation root for the Nivasa project.

- [Request Lifecycle](./request-lifecycle.md) - current SCXML-gated request flow, including the typed `RequestEvent` bridge, the `StatechartEngine::send_event` transition gate, and the repo-local `pkg-config` wrapper that keeps `statechart validate --all` and `statechart parity` building cleanly on this machine.
- [nivasa-http Surface](./http-surface.md) - current request/response surface, including the SCXML-gated pipeline, controller-side runtime slices for `#[body]`, `#[req]`, `#[param]`, `#[query]`, `#[res]`, multipart `#[file]`/`#[files]` helpers, `Result<HttpException>` mapping, buffered streaming, SSE, and attachment helpers.
- [API Versioning](./api-versioning.md) - public versioning config surface, the bootstrap-time `AppBootstrapConfig::global_prefix()` route-setup helper, and runtime boundaries.
- [Server Core](./server-core.md) - transport shell, app-facing bootstrap config boundary, the bootstrap-time `AppBootstrapConfig::global_prefix()` helper, the minimal transport-side CORS bridge, and current TLS/runtime boundaries, with the SCXML request-pipeline gate kept explicit.
