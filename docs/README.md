# Docs

Documentation root for the Nivasa project.

- [Request Lifecycle](./request-lifecycle.md) - current SCXML-gated request flow, including the typed `RequestEvent` bridge and the `StatechartEngine::send_event` transition gate.
- [nivasa-http Surface](./http-surface.md) - current request/response surface, including the SCXML-gated pipeline, `Result<HttpException>` mapping, and attachment helpers.
- [API Versioning](./api-versioning.md) - public versioning config surface, route registration, and runtime boundaries.
- [Server Core](./server-core.md)
