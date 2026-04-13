use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;

use crate::di::{DependencyContainer, DiError, ProviderScope, ValueProvider};
use crate::module::ModuleMetadata;

/// Entry point for testing helpers.
///
/// Current slice focuses on provider overrides for a standalone test DI
/// container. It does not yet bootstrap full modules or HTTP dispatch.
///
/// ```rust
/// use nivasa_core::{ModuleMetadata, Test};
///
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// let testing = Test::create_testing_module(ModuleMetadata::new())
///     .override_provider::<String>()
///     .use_value(String::from("hello"))
///     .compile()
///     .await
///     .unwrap();
///
/// let value = testing.get::<String>().await.unwrap();
/// assert_eq!(value.as_str(), "hello");
/// # });
/// ```
pub struct Test;

impl Test {
    /// Create a testing-module builder.
    pub fn create_testing_module(metadata: ModuleMetadata) -> TestingModuleBuilder {
        TestingModuleBuilder {
            metadata,
            overrides: Vec::new(),
        }
    }
}

#[async_trait]
trait PendingOverride: Send + Sync {
    async fn apply(&self, container: &DependencyContainer);
}

struct ValueOverride<T: Send + Sync + 'static> {
    value: Arc<T>,
}

#[async_trait]
impl<T: Send + Sync + 'static> PendingOverride for ValueOverride<T> {
    async fn apply(&self, container: &DependencyContainer) {
        container
            .register::<T>(Arc::new(ValueProvider::new_from_arc(self.value.clone())))
            .await;
    }
}

struct FactoryOverride<T: Send + Sync + 'static> {
    factory: Arc<dyn Fn() -> T + Send + Sync>,
}

#[async_trait]
impl<T: Send + Sync + 'static> PendingOverride for FactoryOverride<T> {
    async fn apply(&self, container: &DependencyContainer) {
        let factory = Arc::clone(&self.factory);
        container
            .register_factory::<T, _>(ProviderScope::Singleton, vec![], move |_container| {
                let factory = Arc::clone(&factory);
                Box::pin(async move { Ok::<T, DiError>((factory)()) })
            })
            .await;
    }
}

/// Builder for a standalone testing DI container.
pub struct TestingModuleBuilder {
    metadata: ModuleMetadata,
    overrides: Vec<Box<dyn PendingOverride>>,
}

impl TestingModuleBuilder {
    /// Add or replace provider registration for `T`.
    pub fn override_provider<T: Send + Sync + 'static>(self) -> ProviderOverrideBuilder<T> {
        ProviderOverrideBuilder {
            builder: self,
            _marker: PhantomData,
        }
    }

    /// Compile testing module and initialize singleton graph.
    pub async fn compile(self) -> Result<TestingModule, DiError> {
        let container = DependencyContainer::new();

        for override_registration in self.overrides {
            override_registration.apply(&container).await;
        }

        container.initialize().await?;

        Ok(TestingModule {
            metadata: self.metadata,
            container,
        })
    }
}

/// Pending override builder for one provider type.
pub struct ProviderOverrideBuilder<T: Send + Sync + 'static> {
    builder: TestingModuleBuilder,
    _marker: PhantomData<T>,
}

impl<T: Send + Sync + 'static> ProviderOverrideBuilder<T> {
    /// Override provider with a concrete value.
    pub fn use_value(self, value: T) -> TestingModuleBuilder {
        self.use_shared_value(Arc::new(value))
    }

    /// Override provider with a shared value.
    pub fn use_shared_value(mut self, value: Arc<T>) -> TestingModuleBuilder {
        self.builder
            .overrides
            .push(Box::new(ValueOverride::<T> { value }));
        self.builder
    }

    /// Override provider with a singleton factory-backed mock.
    pub fn use_factory<F>(mut self, factory: F) -> TestingModuleBuilder
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        self.builder.overrides.push(Box::new(FactoryOverride::<T> {
            factory: Arc::new(factory),
        }));
        self.builder
    }
}

/// Compiled testing container.
pub struct TestingModule {
    metadata: ModuleMetadata,
    container: DependencyContainer,
}

impl TestingModule {
    /// Resolve provider from testing container.
    pub async fn get<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, DiError> {
        self.container.resolve::<T>().await
    }

    /// Return metadata supplied when builder was created.
    pub fn metadata(&self) -> &ModuleMetadata {
        &self.metadata
    }

    /// Access underlying container for advanced test setup.
    pub fn container(&self) -> &DependencyContainer {
        &self.container
    }
}
