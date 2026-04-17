use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::time::Duration;

use nivasa_common::{HttpException, RequestContext};
use nivasa_interceptors::{
    class_serialize, CallHandler, ClassSerializerInterceptor, ExecutionContext, Interceptor,
    InterceptorFuture, TimeoutInterceptor,
};
use serde::Serialize;

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
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
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[derive(Debug, PartialEq)]
struct AuditRequest {
    id: &'static str,
}

#[derive(Debug)]
struct MetadataPrefixInterceptor;

#[derive(Debug, Clone, Serialize, PartialEq)]
struct SerializableProfile {
    id: u32,
    email: String,
    password: String,
    display_name: String,
}

impl Interceptor for MetadataPrefixInterceptor {
    type Response = String;

    fn intercept(
        &self,
        context: &ExecutionContext,
        next: CallHandler<Self::Response>,
    ) -> InterceptorFuture<Self::Response> {
        let class_name = context.class_name().unwrap_or("unknown").to_string();
        let role = context
            .request_context()
            .and_then(|request_context| request_context.handler_metadata("role"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();

        Box::pin(async move {
            let value = next.handle().await?;
            Ok(format!("{class_name}:{role}:{value}"))
        })
    }
}

#[test]
fn execution_context_propagates_request_context_metadata() {
    let mut request_context = RequestContext::new();
    request_context.insert_request_data(AuditRequest { id: "req-42" });
    request_context.set_handler_metadata("role", "admin");
    request_context.set_class_metadata("version", "v1");
    request_context.set_custom_data("request_id", "trace-123");

    let context = ExecutionContext::new()
        .with_request("GET", "/audit")
        .with_handler_name("show_audit")
        .with_class_name("AuditController")
        .with_request_context(request_context);

    assert_eq!(context.request_method(), Some("GET"));
    assert_eq!(context.request_path(), Some("/audit"));
    assert_eq!(context.handler_name(), Some("show_audit"));
    assert_eq!(context.class_name(), Some("AuditController"));
    assert_eq!(
        context.request_data::<AuditRequest>(),
        Some(&AuditRequest { id: "req-42" })
    );

    let shared = context
        .request_context()
        .expect("request context must exist");
    assert_eq!(
        shared
            .handler_metadata("role")
            .and_then(|value| value.as_str()),
        Some("admin")
    );
    assert_eq!(
        shared
            .class_metadata("version")
            .and_then(|value| value.as_str()),
        Some("v1")
    );
    assert_eq!(
        shared
            .custom_data("request_id")
            .and_then(|value| value.as_str()),
        Some("trace-123")
    );
}

#[test]
fn interceptor_and_call_handler_chain_preserves_metadata_and_runs_once() {
    let interceptor = MetadataPrefixInterceptor;
    let calls = Arc::new(AtomicUsize::new(0));
    let recorded = Arc::clone(&calls);

    let mut request_context = RequestContext::new();
    request_context.set_handler_metadata("role", "admin");

    let context = ExecutionContext::new()
        .with_class_name("AuditController")
        .with_request_context(request_context);

    let next = CallHandler::new(move || {
        let recorded = Arc::clone(&recorded);
        async move {
            recorded.fetch_add(1, Ordering::SeqCst);
            Ok::<_, HttpException>("ok".to_string())
        }
    });

    let result = block_on(interceptor.intercept(&context, next)).expect("interceptor should pass");

    assert_eq!(result, "AuditController:admin:ok");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn execution_context_returns_none_without_shared_request_context() {
    let mut context = ExecutionContext::new().with_class_name("AuditController");

    assert!(context.request_context().is_none());
    assert!(context.request_data::<AuditRequest>().is_none());

    assert_eq!(context.insert_handler_metadata("role", "admin"), None);
    assert_eq!(
        context.insert_handler_metadata("role", "owner"),
        Some("admin".to_string())
    );
    assert_eq!(
        context.handler_metadata().get("role").map(String::as_str),
        Some("owner")
    );
}

#[test]
fn logging_interceptor_records_error_status_and_unknown_request_fields() {
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = {
        let log = Arc::clone(&log);
        move |entry| log.lock().unwrap().push(entry)
    };
    let interceptor =
        nivasa_interceptors::LoggingInterceptor::new(sink, |response: &String| response.clone());
    let context = ExecutionContext::new();
    let next = CallHandler::new(|| async {
        Err::<String, _>(HttpException::internal_server_error("boom"))
    });

    let error = block_on(interceptor.intercept(&context, next)).expect_err("should fail");

    assert_eq!(error.status_code, 500);
    assert_eq!(error.message, "boom");

    let entry = log.lock().unwrap().pop().expect("log entry must exist");
    assert!(entry.contains("method=unknown"));
    assert!(entry.contains("path=unknown"));
    assert!(entry.contains("handler=unknown"));
    assert!(entry.contains("class=unknown"));
    assert!(entry.contains("status=500"));
}

#[test]
fn cache_interceptor_reuses_cached_values_without_invoking_next() {
    let interceptor = nivasa_interceptors::CacheInterceptor::<String>::with_key_resolver(|_| {
        "fixed-key".to_string()
    });
    let context = ExecutionContext::new()
        .with_request("GET", "/cached")
        .with_handler_name("list_cached");
    let calls = Arc::new(AtomicUsize::new(0));

    let first = {
        let calls = Arc::clone(&calls);
        let next = CallHandler::new(move || {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, HttpException>("fresh".to_string())
            }
        });

        block_on(interceptor.intercept(&context, next)).unwrap()
    };

    let second = {
        let calls = Arc::clone(&calls);
        let next = CallHandler::new(move || {
            let calls = Arc::clone(&calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, HttpException>("should-not-run".to_string())
            }
        });

        block_on(interceptor.intercept(&context, next)).unwrap()
    };

    assert_eq!(first, "fresh");
    assert_eq!(second, "fresh");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn class_serializer_interceptor_applies_exposure_then_exclusion() {
    let interceptor = ClassSerializerInterceptor::new()
        .with_exposed_fields(["id", "email", "display_name"])
        .with_excluded_fields(["email"]);
    let context = ExecutionContext::new().with_class_name("ProfileView");
    let next = CallHandler::new(|| async {
        Ok::<_, HttpException>(class_serialize(&SerializableProfile {
            id: 21,
            email: "hidden@example.com".to_string(),
            password: "secret".to_string(),
            display_name: "Reader".to_string(),
        }))
    });

    let result = block_on(interceptor.intercept(&context, next)).unwrap();

    assert_eq!(
        result,
        serde_json::json!({
            "id": 21,
            "display_name": "Reader"
        })
    );
}

#[test]
fn timeout_interceptor_passes_fast_handlers_through() {
    let interceptor = TimeoutInterceptor::<String>::new(Duration::from_millis(50));
    let context = ExecutionContext::new().with_handler_name("list_users");
    let next = CallHandler::new(|| async { Ok::<_, HttpException>("done".to_string()) });

    let result = block_on(interceptor.intercept(&context, next)).unwrap();

    assert_eq!(result, "done");
}
