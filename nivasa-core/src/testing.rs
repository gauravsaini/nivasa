use std::collections::VecDeque;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::di::{DependencyContainer, DiError, ProviderScope, ValueProvider};
use crate::module::ModuleMetadata;

/// Shared mock provider for tests.
///
/// The mock records every call and returns queued responses in FIFO order.
///
/// ```rust
/// use nivasa_core::MockProvider;
///
/// let mock = MockProvider::with_response("ok");
/// let value = mock.call("request");
/// assert_eq!(value, "ok");
/// mock.assert_call_count(1);
/// ```
#[derive(Clone)]
pub struct MockProvider<Args, Output> {
    state: Arc<Mutex<MockState<Args, Output>>>,
}

struct MockState<Args, Output> {
    calls: Vec<Args>,
    responses: VecDeque<Output>,
}

impl<Args, Output> Default for MockProvider<Args, Output>
where
    Args: Clone + PartialEq + Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Args, Output> MockProvider<Args, Output>
where
    Args: Clone + PartialEq + Debug,
{
    /// Create a mock with no queued responses.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                calls: Vec::new(),
                responses: VecDeque::new(),
            })),
        }
    }

    /// Create a mock with one queued response.
    pub fn with_response(response: Output) -> Self {
        let mock = Self::new();
        mock.enqueue_response(response);
        mock
    }

    /// Queue a response for the next call.
    pub fn enqueue_response(&self, response: Output) {
        self.state
            .lock()
            .expect("mock provider lock poisoned")
            .responses
            .push_back(response);
    }

    /// Record call arguments and return the next queued response.
    pub fn call(&self, args: Args) -> Output {
        let mut state = self.state.lock().expect("mock provider lock poisoned");
        state.calls.push(args);
        state
            .responses
            .pop_front()
            .expect("mock provider has no queued response")
    }

    /// Return the number of calls recorded so far.
    pub fn call_count(&self) -> usize {
        self.state
            .lock()
            .expect("mock provider lock poisoned")
            .calls
            .len()
    }

    /// Return a copy of the recorded calls.
    pub fn calls(&self) -> Vec<Args> {
        self.state
            .lock()
            .expect("mock provider lock poisoned")
            .calls
            .clone()
    }

    /// Assert the call count matches the expected value.
    pub fn assert_call_count(&self, expected: usize) {
        assert_eq!(self.call_count(), expected);
    }

    /// Assert the recorded calls match the expected sequence.
    pub fn assert_called_with(&self, expected: &[Args]) {
        let calls = self.calls();
        assert_eq!(calls, expected);
    }
}

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
