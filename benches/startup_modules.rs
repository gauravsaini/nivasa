use async_trait::async_trait;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nivasa_core::{
    module::{Module, ModuleHookSet, ModuleMetadata, ModuleOrchestrator},
    DependencyContainer, DiError,
};
use std::any::TypeId;
use std::hint::black_box;
use tokio::runtime::Runtime;

macro_rules! define_startup_module {
    ($module:ident, $value:ident) => {
        #[derive(Debug)]
        struct $value;

        #[derive(Debug)]
        struct $module;

        #[async_trait]
        impl Module for $module {
            fn metadata(&self) -> ModuleMetadata {
                ModuleMetadata::new()
                    .with_providers(vec![TypeId::of::<$value>()])
                    .with_exports(vec![TypeId::of::<$value>()])
            }

            async fn configure(&self, container: &DependencyContainer) -> Result<(), DiError> {
                container.register_value::<$value>($value).await;
                Ok(())
            }
        }
    };

    ($module:ident, $value:ident, $import:ident) => {
        #[derive(Debug)]
        struct $value;

        #[derive(Debug)]
        struct $module;

        #[async_trait]
        impl Module for $module {
            fn metadata(&self) -> ModuleMetadata {
                ModuleMetadata::new()
                    .with_imports(vec![TypeId::of::<$import>()])
                    .with_providers(vec![TypeId::of::<$value>()])
                    .with_exports(vec![TypeId::of::<$value>()])
            }

            async fn configure(&self, container: &DependencyContainer) -> Result<(), DiError> {
                container.register_value::<$value>($value).await;
                Ok(())
            }
        }
    };
}

macro_rules! register_startup_modules {
    ($orchestrator:expr, $count:expr, $($module:ident),+ $(,)?) => {{
        let mut index = 0usize;
        $(
            if $count > index {
                $orchestrator.register_with_hooks($module, ModuleHookSet::none());
            }
            index += 1;
        )+
        let _ = index;
    }};
}

define_startup_module!(StartupModule0, StartupValue0);
define_startup_module!(StartupModule1, StartupValue1, StartupModule0);
define_startup_module!(StartupModule2, StartupValue2, StartupModule1);
define_startup_module!(StartupModule3, StartupValue3, StartupModule2);
define_startup_module!(StartupModule4, StartupValue4, StartupModule3);
define_startup_module!(StartupModule5, StartupValue5, StartupModule4);
define_startup_module!(StartupModule6, StartupValue6, StartupModule5);
define_startup_module!(StartupModule7, StartupValue7, StartupModule6);
define_startup_module!(StartupModule8, StartupValue8, StartupModule7);
define_startup_module!(StartupModule9, StartupValue9, StartupModule8);
define_startup_module!(StartupModule10, StartupValue10, StartupModule9);
define_startup_module!(StartupModule11, StartupValue11, StartupModule10);
define_startup_module!(StartupModule12, StartupValue12, StartupModule11);
define_startup_module!(StartupModule13, StartupValue13, StartupModule12);
define_startup_module!(StartupModule14, StartupValue14, StartupModule13);
define_startup_module!(StartupModule15, StartupValue15, StartupModule14);
define_startup_module!(StartupModule16, StartupValue16, StartupModule15);
define_startup_module!(StartupModule17, StartupValue17, StartupModule16);
define_startup_module!(StartupModule18, StartupValue18, StartupModule17);
define_startup_module!(StartupModule19, StartupValue19, StartupModule18);
define_startup_module!(StartupModule20, StartupValue20, StartupModule19);
define_startup_module!(StartupModule21, StartupValue21, StartupModule20);
define_startup_module!(StartupModule22, StartupValue22, StartupModule21);
define_startup_module!(StartupModule23, StartupValue23, StartupModule22);
define_startup_module!(StartupModule24, StartupValue24, StartupModule23);

fn populate_orchestrator(module_count: usize) -> ModuleOrchestrator {
    let mut orchestrator = ModuleOrchestrator::new();
    register_startup_modules!(
        orchestrator,
        module_count,
        StartupModule0,
        StartupModule1,
        StartupModule2,
        StartupModule3,
        StartupModule4,
        StartupModule5,
        StartupModule6,
        StartupModule7,
        StartupModule8,
        StartupModule9,
        StartupModule10,
        StartupModule11,
        StartupModule12,
        StartupModule13,
        StartupModule14,
        StartupModule15,
        StartupModule16,
        StartupModule17,
        StartupModule18,
        StartupModule19,
        StartupModule20,
        StartupModule21,
        StartupModule22,
        StartupModule23,
        StartupModule24
    );
    orchestrator
}

fn bench_startup_many_modules(c: &mut Criterion) {
    let runtime = Runtime::new().expect("benchmark runtime must build");
    let mut group = c.benchmark_group("startup_modules");

    for module_count in [1usize, 10, 25] {
        group.bench_with_input(
            BenchmarkId::new("bootstrap", module_count),
            &module_count,
            |bench, &module_count| {
                bench.iter(|| {
                    runtime.block_on(async {
                        let mut orchestrator = populate_orchestrator(module_count);
                        let order = orchestrator
                            .bootstrap()
                            .await
                            .expect("benchmark startup must bootstrap");
                        black_box(order.len());
                    });
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_startup_many_modules);
criterion_main!(benches);
