use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nivasa_core::DependencyContainer;
use std::hint::black_box;
use tokio::runtime::Runtime;

#[derive(Debug)]
struct BenchTarget;

macro_rules! define_dummy_values {
    ($($name:ident),+ $(,)?) => {
        $(#[derive(Debug)] struct $name;)+
    };
}

define_dummy_values!(
    Dummy0, Dummy1, Dummy2, Dummy3, Dummy4, Dummy5, Dummy6, Dummy7, Dummy8, Dummy9, Dummy10,
    Dummy11, Dummy12, Dummy13, Dummy14, Dummy15, Dummy16, Dummy17, Dummy18, Dummy19, Dummy20,
    Dummy21, Dummy22, Dummy23, Dummy24, Dummy25, Dummy26, Dummy27, Dummy28, Dummy29, Dummy30,
    Dummy31, Dummy32, Dummy33, Dummy34, Dummy35, Dummy36, Dummy37, Dummy38, Dummy39, Dummy40,
    Dummy41, Dummy42, Dummy43, Dummy44, Dummy45, Dummy46, Dummy47, Dummy48, Dummy49, Dummy50,
    Dummy51, Dummy52, Dummy53, Dummy54, Dummy55, Dummy56, Dummy57, Dummy58, Dummy59, Dummy60,
    Dummy61, Dummy62, Dummy63, Dummy64, Dummy65, Dummy66, Dummy67, Dummy68, Dummy69, Dummy70,
    Dummy71, Dummy72, Dummy73, Dummy74, Dummy75, Dummy76, Dummy77, Dummy78, Dummy79, Dummy80,
    Dummy81, Dummy82, Dummy83, Dummy84, Dummy85, Dummy86, Dummy87, Dummy88, Dummy89, Dummy90,
    Dummy91, Dummy92, Dummy93, Dummy94, Dummy95, Dummy96, Dummy97, Dummy98, Dummy99
);

macro_rules! register_dummy_values {
    ($container:expr, $count:expr, $($name:ident),+ $(,)?) => {{
        let mut index = 0usize;
        $(
            if $count > index {
                $container.register_value::<$name>($name).await;
            }
            index += 1;
        )+
        let _ = index;
    }};
}

async fn setup_container(provider_count: usize) -> DependencyContainer {
    let container = DependencyContainer::new();
    container.register_value::<BenchTarget>(BenchTarget).await;
    register_dummy_values!(
        container,
        provider_count.saturating_sub(1),
        Dummy0,
        Dummy1,
        Dummy2,
        Dummy3,
        Dummy4,
        Dummy5,
        Dummy6,
        Dummy7,
        Dummy8,
        Dummy9,
        Dummy10,
        Dummy11,
        Dummy12,
        Dummy13,
        Dummy14,
        Dummy15,
        Dummy16,
        Dummy17,
        Dummy18,
        Dummy19,
        Dummy20,
        Dummy21,
        Dummy22,
        Dummy23,
        Dummy24,
        Dummy25,
        Dummy26,
        Dummy27,
        Dummy28,
        Dummy29,
        Dummy30,
        Dummy31,
        Dummy32,
        Dummy33,
        Dummy34,
        Dummy35,
        Dummy36,
        Dummy37,
        Dummy38,
        Dummy39,
        Dummy40,
        Dummy41,
        Dummy42,
        Dummy43,
        Dummy44,
        Dummy45,
        Dummy46,
        Dummy47,
        Dummy48,
        Dummy49,
        Dummy50,
        Dummy51,
        Dummy52,
        Dummy53,
        Dummy54,
        Dummy55,
        Dummy56,
        Dummy57,
        Dummy58,
        Dummy59,
        Dummy60,
        Dummy61,
        Dummy62,
        Dummy63,
        Dummy64,
        Dummy65,
        Dummy66,
        Dummy67,
        Dummy68,
        Dummy69,
        Dummy70,
        Dummy71,
        Dummy72,
        Dummy73,
        Dummy74,
        Dummy75,
        Dummy76,
        Dummy77,
        Dummy78,
        Dummy79,
        Dummy80,
        Dummy81,
        Dummy82,
        Dummy83,
        Dummy84,
        Dummy85,
        Dummy86,
        Dummy87,
        Dummy88,
        Dummy89,
        Dummy90,
        Dummy91,
        Dummy92,
        Dummy93,
        Dummy94,
        Dummy95,
        Dummy96,
        Dummy97,
        Dummy98,
        Dummy99
    );
    container
        .initialize()
        .await
        .expect("benchmark DI graph must initialize");
    container
}

fn bench_di_resolution(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime must build");
    let mut group = c.benchmark_group("di_resolution");

    for provider_count in [1usize, 10, 100] {
        let container = runtime.block_on(setup_container(provider_count));
        group.bench_with_input(
            BenchmarkId::new("resolve_cached_singleton", provider_count),
            &provider_count,
            |bench, _| {
                bench.iter(|| {
                    runtime.block_on(async {
                        let resolved = container
                            .resolve::<BenchTarget>()
                            .await
                            .expect("benchmark DI resolution must succeed");
                        black_box(resolved);
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_di_resolution);
criterion_main!(benches);
