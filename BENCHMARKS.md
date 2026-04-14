# Benchmark Notes

The benchmark harness currently covers two internal workloads plus two comparison targets:

- `di_resolution/resolve_cached_singleton` measures cached `DependencyContainer::resolve::<BenchTarget>()` across 1, 10, and 100 registered providers.
- `routing_dispatch/dispatch_static_route` measures `RouteDispatchRegistry::dispatch()` across 10, 100, and 1000 registered routes.

Run them locally with:

```bash
cargo bench -p nivasa-benchmarks --bench di_resolution -- --quick --noplot
cargo bench -p nivasa-benchmarks --bench routing -- --quick --noplot
```

Current baseline:

- Harness: Criterion 0.5
- Container: `nivasa_core::DependencyContainer`
- Workload: cached singleton resolution with 1, 10, and 100 registered providers
- Registry: `nivasa_routing::RouteDispatchRegistry`
- Workload: static route dispatch with 10, 100, and 1000 registered routes
- Comparison targets: Actix Web and Axum are already included in the harness and proven in `todo.md`

More benchmark rows are still open for middleware pipeline overhead and startup time.

Routing benchmark baseline:

- `routing_dispatch/dispatch_static_route/10`: to be collected
- `routing_dispatch/dispatch_static_route/100`: to be collected
- `routing_dispatch/dispatch_static_route/1000`: to be collected

- `di_resolution/resolve_cached_singleton/1`: `194.75 ns` to `195.45 ns`
- `di_resolution/resolve_cached_singleton/10`: `183.14 ns` to `191.01 ns`
- `di_resolution/resolve_cached_singleton/100`: `194.45 ns` to `204.39 ns`

CI now runs a coarse budget gate for the DI resolution benchmark in addition to the benchmark target. The gate is intentionally loose and meant to catch obvious regressions rather than replace a full historical Criterion baseline service.
