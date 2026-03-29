//! # nivasa-interceptors
//!
//! Nivasa framework interceptor foundations.

use std::collections::BTreeMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nivasa_common::{HttpException, HttpStatus, RequestContext};

/// Standard result type returned by interceptor handlers.
pub type InterceptorResult<T> = Result<T, HttpException>;

/// Boxed future used by [`CallHandler`] and [`Interceptor`].
pub type InterceptorFuture<T> =
    Pin<Box<dyn Future<Output = InterceptorResult<T>> + Send + 'static>>;

/// Minimal request execution context shared with interceptors.
///
/// This keeps the first interceptor slice independent from the HTTP runtime
/// while still carrying the request, handler, class, and metadata shape that
/// later phases will need.
#[derive(Clone, Default)]
pub struct ExecutionContext {
    request_method: Option<String>,
    request_path: Option<String>,
    handler_name: Option<String>,
    class_name: Option<String>,
    handler_metadata: BTreeMap<String, String>,
    class_metadata: BTreeMap<String, String>,
    custom_data: BTreeMap<String, String>,
    request_context: Option<Arc<RequestContext>>,
}

impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("request_method", &self.request_method)
            .field("request_path", &self.request_path)
            .field("handler_name", &self.handler_name)
            .field("class_name", &self.class_name)
            .field("handler_metadata", &self.handler_metadata)
            .field("class_metadata", &self.class_metadata)
            .field("custom_data", &self.custom_data)
            .field("has_request_context", &self.request_context.is_some())
            .finish()
    }
}

impl ExecutionContext {
    /// Create an empty execution context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach request method + path information.
    pub fn with_request(mut self, method: impl Into<String>, path: impl Into<String>) -> Self {
        self.request_method = Some(method.into());
        self.request_path = Some(path.into());
        self
    }

    /// Attach a handler name.
    pub fn with_handler_name(mut self, handler_name: impl Into<String>) -> Self {
        self.handler_name = Some(handler_name.into());
        self
    }

    /// Attach a controller/class name.
    pub fn with_class_name(mut self, class_name: impl Into<String>) -> Self {
        self.class_name = Some(class_name.into());
        self
    }

    /// Attach the canonical shared request context.
    pub fn with_request_context(mut self, request_context: RequestContext) -> Self {
        self.request_context = Some(Arc::new(request_context));
        self
    }

    /// Record handler metadata for later interceptor lookups.
    pub fn insert_handler_metadata(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<String> {
        self.handler_metadata.insert(key.into(), value.into())
    }

    /// Record class metadata for later interceptor lookups.
    pub fn insert_class_metadata(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<String> {
        self.class_metadata.insert(key.into(), value.into())
    }

    /// Record custom per-request data.
    pub fn insert_custom_data(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<String> {
        self.custom_data.insert(key.into(), value.into())
    }

    pub fn request_method(&self) -> Option<&str> {
        self.request_method.as_deref()
    }

    pub fn request_path(&self) -> Option<&str> {
        self.request_path.as_deref()
    }

    pub fn handler_name(&self) -> Option<&str> {
        self.handler_name.as_deref()
    }

    pub fn class_name(&self) -> Option<&str> {
        self.class_name.as_deref()
    }

    pub fn handler_metadata(&self) -> &BTreeMap<String, String> {
        &self.handler_metadata
    }

    pub fn class_metadata(&self) -> &BTreeMap<String, String> {
        &self.class_metadata
    }

    pub fn custom_data(&self) -> &BTreeMap<String, String> {
        &self.custom_data
    }

    /// Access the shared canonical request context, when present.
    pub fn request_context(&self) -> Option<&RequestContext> {
        self.request_context.as_deref()
    }

    /// Look up typed request data from the shared request context.
    pub fn request_data<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.request_context()
            .and_then(|request_context| request_context.request_data::<T>())
    }
}

/// Deferred handler invocation passed into an interceptor.
pub struct CallHandler<T> {
    inner: Option<Box<dyn FnOnce() -> InterceptorFuture<T> + Send + 'static>>,
}

impl<T> CallHandler<T>
where
    T: Send + 'static,
{
    /// Create a call handler from an async function/closure.
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = InterceptorResult<T>> + Send + 'static,
    {
        Self {
            inner: Some(Box::new(move || Box::pin(f()))),
        }
    }

    /// Execute the deferred handler.
    pub async fn handle(mut self) -> InterceptorResult<T> {
        let handler = self
            .inner
            .take()
            .expect("CallHandler::handle() may only be called once");
        handler().await
    }
}

/// Trait implemented by interceptor types.
pub trait Interceptor: Send + Sync {
    type Response: Send + 'static;

    fn intercept(
        &self,
        context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response>;
}

/// Interceptor that turns slow handlers into a request timeout error.
///
/// This stays deliberately small and dependency-free. It measures the elapsed
/// wall-clock time around the next handler in the existing interceptor chain
/// and returns a `408 Request Timeout` response if the handler takes too long.
#[derive(Clone, Debug)]
pub struct TimeoutInterceptor<T = ()> {
    timeout: Duration,
    _marker: PhantomData<fn() -> T>,
}

impl<T> TimeoutInterceptor<T> {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            _marker: PhantomData,
        }
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl<T> Interceptor for TimeoutInterceptor<T>
where
    T: Send + 'static,
{
    type Response = T;

    fn intercept(
        &self,
        _context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let timeout = self.timeout;
        Box::pin(async move {
            let started = Instant::now();
            let result = next.handle().await;

            if started.elapsed() > timeout {
                Err(HttpException::from_status(
                    HttpStatus::RequestTimeout,
                    "request timed out",
                ))
            } else {
                result
            }
        })
    }
}

type LogSink = Arc<dyn Fn(String) + Send + Sync + 'static>;
type StatusFormatter<T> = Arc<dyn Fn(&T) -> String + Send + Sync + 'static>;
type CacheKeyResolver = Arc<dyn Fn(&ExecutionContext) -> String + Send + Sync + 'static>;
type CacheStore<T> = Arc<Mutex<BTreeMap<String, T>>>;

/// Interceptor that records request metadata, response status, and duration.
///
/// The slice stays deliberately small and testable by accepting a log sink and
/// a response-status formatter. That keeps the helper independent from any
/// concrete HTTP response type while still letting the HTTP test harness
/// assert on the emitted log line.
#[derive(Clone)]
pub struct LoggingInterceptor<T = ()> {
    sink: LogSink,
    status_formatter: StatusFormatter<T>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> LoggingInterceptor<T> {
    /// Create a logging interceptor with a caller-provided sink and response formatter.
    pub fn new<S, F>(sink: S, status_formatter: F) -> Self
    where
        S: Fn(String) + Send + Sync + 'static,
        F: Fn(&T) -> String + Send + Sync + 'static,
    {
        Self {
            sink: Arc::new(sink),
            status_formatter: Arc::new(status_formatter),
            _marker: PhantomData,
        }
    }
}

impl<T> Interceptor for LoggingInterceptor<T>
where
    T: Send + 'static,
{
    type Response = T;

    fn intercept(
        &self,
        context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let sink = Arc::clone(&self.sink);
        let status_formatter = Arc::clone(&self.status_formatter);
        let method = context.request_method().unwrap_or("unknown").to_string();
        let path = context.request_path().unwrap_or("unknown").to_string();
        let handler_name = context.handler_name().unwrap_or("unknown").to_string();
        let class_name = context.class_name().unwrap_or("unknown").to_string();

        Box::pin(async move {
            let started = Instant::now();
            let result = next.handle().await;
            let elapsed = started.elapsed();
            let status = match result.as_ref() {
                Ok(response) => status_formatter.as_ref()(response),
                Err(error) => error.status_code.to_string(),
            };

            sink.as_ref()(format!(
                "method={method} path={path} handler={handler_name} class={class_name} status={status} duration_ns={}",
                elapsed.as_nanos()
            ));

            result
        })
    }
}

/// Interceptor that memoizes successful responses in a small in-memory store.
///
/// The helper is intentionally simple: it derives a cache key from the request
/// shape already present on [`ExecutionContext`], stores cloned successful
/// responses in a process-local map, and skips the next handler when a matching
/// entry is present.
#[derive(Clone)]
pub struct CacheInterceptor<T = ()> {
    store: CacheStore<T>,
    key_resolver: CacheKeyResolver,
    _marker: PhantomData<fn() -> T>,
}

impl<T> CacheInterceptor<T> {
    /// Create a cache interceptor using the default request-shape key.
    pub fn new() -> Self {
        Self::with_key_resolver(default_cache_key)
    }

    /// Create a cache interceptor with a caller-provided cache key resolver.
    pub fn with_key_resolver<F>(key_resolver: F) -> Self
    where
        F: Fn(&ExecutionContext) -> String + Send + Sync + 'static,
    {
        Self {
            store: Arc::new(Mutex::new(BTreeMap::new())),
            key_resolver: Arc::new(key_resolver),
            _marker: PhantomData,
        }
    }
}

impl<T> Default for CacheInterceptor<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Interceptor for CacheInterceptor<T>
where
    T: Clone + Send + 'static,
{
    type Response = T;

    fn intercept(
        &self,
        context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let store = Arc::clone(&self.store);
        let key_resolver = Arc::clone(&self.key_resolver);
        let cache_key = key_resolver.as_ref()(context);

        Box::pin(async move {
            if let Some(response) = store.lock().unwrap().get(&cache_key).cloned() {
                return Ok(response);
            }

            let result = next.handle().await;
            if let Ok(response) = &result {
                store.lock().unwrap().insert(cache_key, response.clone());
            }

            result
        })
    }
}

fn default_cache_key(context: &ExecutionContext) -> String {
    format!(
        "method={};path={};handler={};class={}",
        context.request_method().unwrap_or("unknown"),
        context.request_path().unwrap_or("unknown"),
        context.handler_name().unwrap_or("unknown"),
        context.class_name().unwrap_or("unknown")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn block_on<F: Future>(future: F) -> F::Output {
        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn wake(_: *const ()) {}
            fn wake_by_ref(_: *const ()) {}
            fn drop(_: *const ()) {}

            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
            )
        }

        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut future = Box::pin(future);
        let mut context = Context::from_waker(&waker);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    struct PrefixInterceptor;

    impl Interceptor for PrefixInterceptor {
        type Response = String;

        fn intercept(
            &self,
            context: &ExecutionContext,
            next: CallHandler<Self::Response>,
        ) -> InterceptorFuture<Self::Response> {
            let handler_name = context.handler_name().unwrap_or("unknown").to_string();
            Box::pin(async move {
                let value = next.handle().await?;
                Ok(format!("{handler_name}:{value}"))
            })
        }
    }

    #[test]
    fn execution_context_tracks_request_and_metadata_shape() {
        let mut context = ExecutionContext::new()
            .with_request("POST", "/users")
            .with_handler_name("create_user")
            .with_class_name("UsersController");
        context.insert_handler_metadata("role", "admin");
        context.insert_class_metadata("version", "1");
        context.insert_custom_data("request_id", "req-123");

        assert_eq!(context.request_method(), Some("POST"));
        assert_eq!(context.request_path(), Some("/users"));
        assert_eq!(context.handler_name(), Some("create_user"));
        assert_eq!(context.class_name(), Some("UsersController"));
        assert_eq!(
            context.handler_metadata().get("role").map(String::as_str),
            Some("admin")
        );
        assert_eq!(
            context.class_metadata().get("version").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            context.custom_data().get("request_id").map(String::as_str),
            Some("req-123")
        );
    }

    #[test]
    fn call_handler_runs_the_deferred_handler_once() {
        let calls = Arc::new(AtomicUsize::new(0));
        let recorded = Arc::clone(&calls);
        let handler = CallHandler::new(move || {
            let recorded = Arc::clone(&recorded);
            async move {
                recorded.fetch_add(1, Ordering::SeqCst);
                Ok::<_, HttpException>("ok")
            }
        });

        let result = block_on(handler.handle()).unwrap();

        assert_eq!(result, "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn interceptors_can_wrap_the_next_handler() {
        let interceptor = PrefixInterceptor;
        let context = ExecutionContext::new().with_handler_name("list_users");
        let next = CallHandler::new(|| async { Ok::<_, HttpException>("done".to_string()) });

        let result = block_on(interceptor.intercept(&context, next)).unwrap();

        assert_eq!(result, "list_users:done");
    }

    #[test]
    fn interceptors_can_propagate_handler_errors() {
        let interceptor = PrefixInterceptor;
        let context = ExecutionContext::new().with_handler_name("list_users");
        let next = CallHandler::new(|| async {
            Err::<String, _>(HttpException::internal_server_error("boom"))
        });

        let error = block_on(interceptor.intercept(&context, next)).unwrap_err();

        assert_eq!(error.status_code, 500);
        assert_eq!(error.message, "boom");
    }

    #[test]
    fn timeout_interceptor_turns_slow_handlers_into_request_timeout_errors() {
        let interceptor = TimeoutInterceptor::<String>::new(Duration::from_millis(1));
        let context = ExecutionContext::new().with_handler_name("list_users");
        let next = CallHandler::new(|| async {
            std::thread::sleep(Duration::from_millis(5));
            Ok::<_, HttpException>("done".to_string())
        });

        let error = block_on(interceptor.intercept(&context, next)).unwrap_err();

        assert_eq!(error.status_code, 408);
        assert_eq!(error.message, "request timed out");
    }

    #[test]
    fn logging_interceptor_records_status_and_duration() {
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = {
            let log = Arc::clone(&log);
            move |entry| log.lock().unwrap().push(entry)
        };
        let interceptor = LoggingInterceptor::new(sink, |response: &String| response.clone());
        let context = ExecutionContext::new()
            .with_request("GET", "/logging")
            .with_handler_name("list_users")
            .with_class_name("UsersController");
        let next = CallHandler::new(|| async { Ok::<_, HttpException>("200".to_string()) });

        let result = block_on(interceptor.intercept(&context, next)).unwrap();

        assert_eq!(result, "200");
        let entry = log.lock().unwrap().pop().expect("log entry must exist");
        assert!(entry.contains("method=GET"));
        assert!(entry.contains("path=/logging"));
        assert!(entry.contains("handler=list_users"));
        assert!(entry.contains("class=UsersController"));
        assert!(entry.contains("status=200"));
        assert!(entry.contains("duration_ns="));
    }

    #[derive(Debug, PartialEq)]
    struct TestRequest {
        method: &'static str,
        path: &'static str,
    }

    #[test]
    fn execution_context_can_carry_the_shared_request_context() {
        let mut request_context = RequestContext::new();
        request_context.insert_request_data(TestRequest {
            method: "GET",
            path: "/users/42",
        });

        let context = ExecutionContext::new().with_request_context(request_context);

        assert_eq!(
            context.request_data::<TestRequest>(),
            Some(&TestRequest {
                method: "GET",
                path: "/users/42",
            })
        );
        assert!(context.request_context().is_some());
    }
}
