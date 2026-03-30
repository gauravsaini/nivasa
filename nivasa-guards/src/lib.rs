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

use nivasa_common::{HttpException, RequestContext};

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
    request_context: Option<Arc<RequestContext>>,
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
            request_context: None,
            handler_metadata: BTreeMap::new(),
            class_metadata: BTreeMap::new(),
            custom_data: BTreeMap::new(),
        }
    }

    /// Attach the shared canonical request context without replacing the
    /// existing guard-local request value or metadata surface.
    pub fn with_request_context(mut self, request_context: RequestContext) -> Self {
        self.request_context = Some(Arc::new(request_context));
        self
    }

    pub fn request_context(&self) -> Option<&RequestContext> {
        self.request_context.as_deref()
    }

    /// Return the typed request value if it matches the requested type.
    pub fn request<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.request
            .as_ref()
            .downcast_ref::<T>()
            .or_else(|| self.request_context.as_ref()?.request_data::<T>())
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

/// Guard that authorizes requests by comparing required `roles` metadata from
/// the request context against the roles attached to the current request.
#[derive(Debug, Default, Clone, Copy)]
pub struct RolesGuard;

impl RolesGuard {
    /// Create a new roles guard.
    pub fn new() -> Self {
        Self
    }

    fn roles_from_context_values(values: &ContextDataMap) -> Option<Vec<String>> {
        let value = values.get("roles")?.as_ref();

        if let Some(roles) = value.downcast_ref::<Vec<String>>() {
            return Some(roles.clone());
        }

        value
            .downcast_ref::<Vec<&'static str>>()
            .map(|roles| roles.iter().map(|role| (*role).to_string()).collect())
    }

    fn required_roles(context: &ExecutionContext) -> Option<Vec<String>> {
        if let Some(request_context) = context.request_context() {
            if let Some(values) = request_context.handler_metadata("roles") {
                let roles = values.as_array()?;
                return Some(
                    roles
                        .iter()
                        .map(|role| role.as_str().map(|role| role.to_string()))
                        .collect::<Option<Vec<_>>>()?,
                );
            }

            if let Some(values) = request_context.class_metadata("roles") {
                let roles = values.as_array()?;
                return Some(
                    roles
                        .iter()
                        .map(|role| role.as_str().map(|role| role.to_string()))
                        .collect::<Option<Vec<_>>>()?,
                );
            }
        }

        Self::roles_from_context_values(context.handler_metadata_map())
            .or_else(|| Self::roles_from_context_values(context.class_metadata_map()))
    }

    fn principal_roles(context: &ExecutionContext) -> Option<Vec<String>> {
        if let Some(request_context) = context.request_context() {
            if let Some(values) = request_context.custom_data("roles") {
                let roles = values.as_array()?;
                return Some(
                    roles
                        .iter()
                        .map(|role| role.as_str().map(|role| role.to_string()))
                        .collect::<Option<Vec<_>>>()?,
                );
            }
        }

        Self::roles_from_context_values(context.custom_data_map())
    }

    fn roles_match(required: &[String], principal: &[String]) -> bool {
        required
            .iter()
            .any(|role| principal.iter().any(|candidate| candidate == role))
    }
}

impl Guard for RolesGuard {
    fn can_activate<'a>(&'a self, context: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move {
            let required_roles = match Self::required_roles(context) {
                Some(roles) => roles,
                None => return Ok(true),
            };

            let principal_roles = Self::principal_roles(context).unwrap_or_default();
            Ok(Self::roles_match(&required_roles, &principal_roles))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ExecutionContext, Guard, RolesGuard};
    use nivasa_common::{HttpException, RequestContext};
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
    fn execution_context_can_attach_shared_request_context() {
        let mut request_context = RequestContext::new();
        request_context.insert_request_data(FakeRequest { path: "/shared" });
        request_context.set_handler_metadata("roles", ["admin"]);

        let context = ExecutionContext::new(())
            .with_request_context(request_context);

        assert_eq!(
            context.request::<FakeRequest>(),
            Some(&FakeRequest { path: "/shared" })
        );
        assert!(context.request_context().is_some());
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

    #[test]
    fn roles_guard_matches_roles_from_context_metadata() {
        let mut request_context = RequestContext::new();
        request_context.set_handler_metadata("roles", ["editor"]);
        request_context.set_custom_data("roles", ["editor"]);

        let context = ExecutionContext::new(())
            .with_request_context(request_context)
            .with_class_metadata("roles", vec!["admin"]);

        let guard = RolesGuard::new();
        let result = run_ready(guard.can_activate(&context));

        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn roles_guard_denies_when_roles_do_not_overlap() {
        let mut request_context = RequestContext::new();
        request_context.set_class_metadata("roles", ["admin"]);
        request_context.set_custom_data("roles", ["guest"]);

        let context = ExecutionContext::new(()).with_request_context(request_context);

        let guard = RolesGuard::new();
        let result = run_ready(guard.can_activate(&context));

        assert_eq!(result.unwrap(), false);
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
