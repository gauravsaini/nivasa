# Benchmark Notes

The benchmark harness currently covers four internal workloads plus two comparison targets:

- `di_resolution/resolve_cached_singleton` measures cached `DependencyContainer::resolve::<BenchTarget>()` across 1, 10, and 100 registered providers.
- `routing_dispatch/dispatch_static_route` measures `RouteDispatchRegistry::dispatch()` across 10, 100, and 1000 registered routes.
- `pipeline_overhead/route_only_roundtrip` measures the baseline route path with no global middleware, module middleware, route middleware, guard, or interceptor.
- `pipeline_overhead/full_stack_roundtrip` measures the normal SCXML-backed request path with global middleware, module middleware, route middleware, a guard, and an interceptor.
- `startup_modules/bootstrap` measures module bootstrap across 1, 10, and 25 modules.

Run them locally with:

```bash
cargo bench -p nivasa-benchmarks --bench di_resolution -- --quick --noplot
cargo bench -p nivasa-benchmarks --bench routing -- --quick --noplot
cargo bench -p nivasa-benchmarks --bench pipeline_overhead -- --quick --noplot
cargo bench -p nivasa-benchmarks --bench startup_modules -- --quick --noplot
```

Current baseline:

- Harness: Criterion 0.5
- Container: `nivasa_core::DependencyContainer`
- Workload: cached singleton resolution with 1, 10, and 100 registered providers
- Registry: `nivasa_routing::RouteDispatchRegistry`
- Workload: static route dispatch with 10, 100, and 1000 registered routes
- Comparison targets: Actix Web and Axum are already included in the harness and proven in `todo.md`
- Server path: `nivasa_http::NivasaServer` with middleware, guard, and interceptor on the normal request flow
- Orchestrator: `nivasa_core::module::ModuleOrchestrator` bootstrapping module stacks with imports and exports

Pipeline overhead row is now complete. Full stack path still wraps handler response in `{"data": ...}` when interceptor is active.

Routing benchmark baseline:

- `routing_dispatch/dispatch_static_route/10`: to be collected
- `routing_dispatch/dispatch_static_route/100`: to be collected
- `routing_dispatch/dispatch_static_route/1000`: to be collected
- `pipeline_overhead/route_only_roundtrip/baseline`: `78.572 µs` to `266.30 µs`
- `pipeline_overhead/full_stack_roundtrip/middleware_guard_interceptor`: `209.95 µs` to `329.65 µs`
- `startup_modules/bootstrap/1`: `4.2147 µs` to `5.7422 µs`
- `startup_modules/bootstrap/10`: `76.755 µs` to `90.848 µs`
- `startup_modules/bootstrap/25`: `235.25 µs` to `236.73 µs`

- `di_resolution/resolve_cached_singleton/1`: `194.75 ns` to `195.45 ns`
- `di_resolution/resolve_cached_singleton/10`: `183.14 ns` to `191.01 ns`
- `di_resolution/resolve_cached_singleton/100`: `194.45 ns` to `204.39 ns`

CI now runs a coarse budget gate for the DI resolution benchmark in addition to the benchmark target. The gate is intentionally loose and meant to catch obvious regressions rather than replace a full historical Criterion baseline service.

The startup benchmark implementation is wired in `benches/startup_modules.rs` and now runs under `cargo bench`.

Pipeline overhead is wired and measured. Latest quick run showed `route_only_roundtrip/baseline` at `78.572 µs` to `266.30 µs`, and `full_stack_roundtrip/middleware_guard_interceptor` at `209.95 µs` to `329.65 µs` with Criterion reporting a regression versus the previous sample.
