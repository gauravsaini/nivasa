//! # nivasa-guards
//!
//! Nivasa framework guard primitives.

use std::{
    any::Any,
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::Arc,
    time::Duration,
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

/// Skeleton authentication guard.
///
/// This is intentionally shallow: it only checks for a bearer token that
/// looks JWT-shaped (`Bearer <header.payload.signature>`). Real JWT parsing,
/// signature verification, and claims validation remain future work.
#[derive(Debug, Default, Clone, Copy)]
pub struct AuthGuard;

impl AuthGuard {
    /// Create a new auth guard.
    pub fn new() -> Self {
        Self
    }

    fn bearer_token_from_context(context: &ExecutionContext) -> Option<String> {
        if let Some(request_context) = context.request_context() {
            if let Some(value) = request_context.custom_data("authorization") {
                if let Some(token) = value.as_str() {
                    return Some(token.to_string());
                }
            }
        }

        if let Some(value) = context.custom_data_map().get("authorization") {
            let value = value.as_ref();

            if let Some(token) = value.downcast_ref::<String>() {
                return Some(token.clone());
            }

            if let Some(token) = value.downcast_ref::<&'static str>() {
                return Some((*token).to_string());
            }
        }

        None
    }

    fn looks_like_jwt_bearer(token: &str) -> bool {
        let Some(token) = token.trim().strip_prefix("Bearer ") else {
            return false;
        };

        let mut segments = token.split('.');
        let has_three_segments = segments.next().is_some()
            && segments.next().is_some()
            && segments.next().is_some()
            && segments.next().is_none();

        has_three_segments && token.split('.').all(|segment| !segment.trim().is_empty())
    }
}

impl Guard for AuthGuard {
    fn can_activate<'a>(&'a self, context: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move {
            Ok(Self::bearer_token_from_context(context)
                .as_deref()
                .is_some_and(Self::looks_like_jwt_bearer))
        })
    }
}

/// Skeleton throttling guard.
///
/// This keeps only the guard shape and the configured rate-limit metadata.
/// Cross-request counters, storage backends, and true rate enforcement remain
/// future work in the throttling module slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrottlerGuard {
    limit: u32,
    ttl: Duration,
}

impl ThrottlerGuard {
    /// Create a new throttling guard skeleton.
    pub fn new(limit: u32, ttl: Duration) -> Self {
        Self { limit, ttl }
    }

    /// Number of requests allowed in the configured window.
    pub fn limit(&self) -> u32 {
        self.limit
    }

    /// Duration of the configured window.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    fn has_minimal_valid_configuration(&self) -> bool {
        self.limit > 0 && !self.ttl.is_zero()
    }
}

impl Guard for ThrottlerGuard {
    fn can_activate<'a>(&'a self, _context: &'a ExecutionContext) -> GuardFuture<'a> {
        Box::pin(async move { Ok(self.has_minimal_valid_configuration()) })
    }
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
                return roles
                    .iter()
                    .map(|role| role.as_str().map(|role| role.to_string()))
                    .collect::<Option<Vec<_>>>();
            }

            if let Some(values) = request_context.class_metadata("roles") {
                let roles = values.as_array()?;
                return roles
                    .iter()
                    .map(|role| role.as_str().map(|role| role.to_string()))
                    .collect::<Option<Vec<_>>>();
            }
        }

        Self::roles_from_context_values(context.handler_metadata_map())
            .or_else(|| Self::roles_from_context_values(context.class_metadata_map()))
    }

    fn principal_roles(context: &ExecutionContext) -> Option<Vec<String>> {
        if let Some(request_context) = context.request_context() {
            if let Some(values) = request_context.custom_data("roles") {
                let roles = values.as_array()?;
                return roles
                    .iter()
                    .map(|role| role.as_str().map(|role| role.to_string()))
                    .collect::<Option<Vec<_>>>();
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
    use super::{AuthGuard, ExecutionContext, Guard, RolesGuard, ThrottlerGuard};
    use nivasa_common::{HttpException, RequestContext};
    use std::{
        future::Future,
        pin::Pin,
        time::Duration,
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
        assert!(result.unwrap());
    }

    #[test]
    fn auth_guard_accepts_jwt_shaped_bearer_tokens() {
        let mut request_context = RequestContext::new();
        request_context.set_custom_data("authorization", "Bearer header.payload.signature");

        let context = ExecutionContext::new(()).with_request_context(request_context);
        let guard = AuthGuard::new();

        let result = run_ready(guard.can_activate(&context));

        assert!(result.unwrap());
    }

    #[test]
    fn auth_guard_rejects_missing_or_malformed_bearer_tokens() {
        let mut missing_context = RequestContext::new();
        missing_context.set_custom_data("authorization", "token-without-bearer-prefix");

        let missing = ExecutionContext::new(()).with_request_context(missing_context);
        let guard = AuthGuard::new();

        assert!(!run_ready(guard.can_activate(&missing)).unwrap());

        let mut malformed_context = RequestContext::new();
        malformed_context.set_custom_data("authorization", "Bearer not.jwt-shaped");

        let malformed = ExecutionContext::new(()).with_request_context(malformed_context);

        assert!(!run_ready(guard.can_activate(&malformed)).unwrap());
    }

    #[test]
    fn throttler_guard_exposes_rate_limit_configuration_without_storage_backends() {
        let guard = ThrottlerGuard::new(10, Duration::from_secs(60));

        assert_eq!(guard.limit(), 10);
        assert_eq!(guard.ttl(), Duration::from_secs(60));
        assert!(run_ready(guard.can_activate(&ExecutionContext::new(()))).unwrap());
    }

    #[test]
    fn throttler_guard_rejects_unconfigured_windows() {
        let zero_limit = ThrottlerGuard::new(0, Duration::from_secs(60));
        let zero_ttl = ThrottlerGuard::new(10, Duration::from_secs(0));
        let context = ExecutionContext::new(());

        assert!(!run_ready(zero_limit.can_activate(&context)).unwrap());
        assert!(!run_ready(zero_ttl.can_activate(&context)).unwrap());
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

        assert!(result.unwrap());
    }

    #[test]
    fn roles_guard_denies_when_roles_do_not_overlap() {
        let mut request_context = RequestContext::new();
        request_context.set_class_metadata("roles", ["admin"]);
        request_context.set_custom_data("roles", ["guest"]);

        let context = ExecutionContext::new(()).with_request_context(request_context);

        let guard = RolesGuard::new();
        let result = run_ready(guard.can_activate(&context));

        assert!(!result.unwrap());
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
