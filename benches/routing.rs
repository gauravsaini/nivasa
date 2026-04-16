use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod};
use std::hint::black_box;

fn build_registry(route_count: usize) -> RouteDispatchRegistry<usize> {
    let mut registry = RouteDispatchRegistry::new();

    for index in 0..route_count {
        registry
            .register_static(RouteMethod::Get, format!("/routes/{index}"), index)
            .expect("benchmark route registration must succeed");
    }

    registry
}

fn bench_routing_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_dispatch");

    for route_count in [10usize, 100, 1000] {
        let registry = build_registry(route_count);
        let target_path = format!("/routes/{}", route_count - 1);

        group.bench_with_input(
            BenchmarkId::new("dispatch_static_route", route_count),
            &target_path,
            |bench, path| {
                bench.iter(|| {
                    let outcome = registry.dispatch("GET", black_box(path.as_str()));
                    match outcome {
                        RouteDispatchOutcome::Matched(entry) => black_box(entry.value),
                        _ => panic!("benchmark route dispatch must match"),
                    };
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_routing_dispatch);
criterion_main!(benches);
