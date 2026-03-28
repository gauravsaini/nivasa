//! # nivasa-guards
//!
//! Nivasa framework guard primitives.

use std::{
    any::Any,
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::Arc,
};

use nivasa_common::HttpException;

type ContextValue = Arc<dyn Any + Send + Sync>;

/// Metadata/custom data shared with guards during request execution.
pub type ContextDataMap = BTreeMap<String, ContextValue>;

/// Runtime context passed into guards.
///
/// The request is intentionally stored as an opaque typed value so this crate
/// can define the surface before the HTTP integration is wired in.
#[derive(Clone)]
pub struct ExecutionContext {
    request: ContextValue,
    handler_metadata: ContextDataMap,
    class_metadata: ContextDataMap,
    custom_data: ContextDataMap,
}

impl ExecutionContext {
    /// Create a new guard execution context from an arbitrary request value.
    pub fn new<T>(request: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        Self {
            request: Arc::new(request),
            handler_metadata: BTreeMap::new(),
            class_metadata: BTreeMap::new(),
            custom_data: BTreeMap::new(),
        }
    }

    /// Return the typed request value if it matches the requested type.
    pub fn request<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.request.as_ref().downcast_ref::<T>()
    }

    /// Return the raw request value for integration layers that need to downcast manually.
    pub fn request_value(&self) -> &(dyn Any + Send + Sync) {
        self.request.as_ref()
    }

    pub fn handler_metadata<T>(&self, key: &str) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.handler_metadata
            .get(key)
            .and_then(|value| value.as_ref().downcast_ref::<T>())
    }

    pub fn class_metadata<T>(&self, key: &str) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.class_metadata
            .get(key)
            .and_then(|value| value.as_ref().downcast_ref::<T>())
    }

    pub fn custom_data<T>(&self, key: &str) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.custom_data
            .get(key)
            .and_then(|value| value.as_ref().downcast_ref::<T>())
    }

    pub fn with_handler_metadata<T>(mut self, key: impl Into<String>, value: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.insert_handler_metadata(key, value);
        self
    }

    pub fn with_class_metadata<T>(mut self, key: impl Into<String>, value: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.insert_class_metadata(key, value);
        self
    }

    pub fn with_custom_data<T>(mut self, key: impl Into<String>, value: T) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.insert_custom_data(key, value);
        self
    }

    pub fn insert_handler_metadata<T>(&mut self, key: impl Into<String>, value: T)
    where
        T: Any + Send + Sync + 'static,
    {
        self.handler_metadata.insert(key.into(), Arc::new(value));
    }

    pub fn insert_class_metadata<T>(&mut self, key: impl Into<String>, value: T)
    where
        T: Any + Send + Sync + 'static,
    {
        self.class_metadata.insert(key.into(), Arc::new(value));
    }

    pub fn insert_custom_data<T>(&mut self, key: impl Into<String>, value: T)
    where
        T: Any + Send + Sync + 'static,
    {
        self.custom_data.insert(key.into(), Arc::new(value));
    }

    pub fn handler_metadata_map(&self) -> &ContextDataMap {
        &self.handler_metadata
    }

    pub fn class_metadata_map(&self) -> &ContextDataMap {
        &self.class_metadata
    }

    pub fn custom_data_map(&self) -> &ContextDataMap {
        &self.custom_data
    }
}

/// Boxed future returned by a guard.
pub type GuardFuture<'a> = Pin<Box<dyn Future<Output = Result<bool, HttpException>> + Send + 'a>>;

/// Request guard surface.
pub trait Guard: Send + Sync {
    fn can_activate<'a>(&'a self, context: &'a ExecutionContext) -> GuardFuture<'a>;
}

#[cfg(test)]
mod tests {
    use super::{ExecutionContext, Guard};
    use nivasa_common::HttpException;
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct FakeRequest {
        path: &'static str,
    }

    struct RoleGuard;

    impl Guard for RoleGuard {
        fn can_activate<'a>(
            &'a self,
            context: &'a ExecutionContext,
        ) -> Pin<Box<dyn Future<Output = Result<bool, HttpException>> + Send + 'a>> {
            Box::pin(async move {
                Ok(context
                    .handler_metadata::<Vec<&'static str>>("roles")
                    .is_some_and(|roles| roles.contains(&"admin")))
            })
        }
    }

    #[test]
    fn execution_context_exposes_typed_request_and_metadata() {
        let context = ExecutionContext::new(FakeRequest { path: "/admin" })
            .with_handler_metadata("roles", vec!["admin", "editor"])
            .with_class_metadata("controller", "users")
            .with_custom_data("tenant", "acme");

        assert_eq!(
            context.request::<FakeRequest>(),
            Some(&FakeRequest { path: "/admin" })
        );
        assert_eq!(
            context.handler_metadata::<Vec<&'static str>>("roles"),
            Some(&vec!["admin", "editor"])
        );
        assert_eq!(context.class_metadata::<&'static str>("controller"), Some(&"users"));
        assert_eq!(context.custom_data::<&'static str>("tenant"), Some(&"acme"));
    }

    #[test]
    fn guard_trait_can_read_execution_context() {
        let context =
            ExecutionContext::new(FakeRequest { path: "/admin" }).with_handler_metadata(
                "roles",
                vec!["admin", "editor"],
            );

        let result = run_ready(RoleGuard.can_activate(&context));
        assert_eq!(result.unwrap(), true);
    }

    fn run_ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);

        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("test future unexpectedly pending"),
        }
    }

    fn noop_waker() -> Waker {
        unsafe { Waker::from_raw(noop_raw_waker()) }
    }

    fn noop_raw_waker() -> RawWaker {
        RawWaker::new(std::ptr::null(), &NOOP_RAW_WAKER_VTABLE)
    }

    unsafe fn noop_clone(_: *const ()) -> RawWaker {
        noop_raw_waker()
    }

    unsafe fn noop(_: *const ()) {}

    static NOOP_RAW_WAKER_VTABLE: RawWakerVTable =
        RawWakerVTable::new(noop_clone, noop, noop, noop);
}
