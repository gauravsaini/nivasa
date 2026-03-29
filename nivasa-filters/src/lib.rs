//! # nivasa-filters
//!
//! Nivasa framework — filters.
//!
//! This crate intentionally stays small for now: it defines the filter
//! surface and the transport-agnostic host types that later phases will wire
//! into the HTTP runtime.

use std::{fmt, future::Future, pin::Pin, sync::Arc};

use nivasa_common::{HttpException, RequestContext};

/// Boxed future returned by exception filters.
pub type ExceptionFilterFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Transport-agnostic filter host backed by the shared request context.
#[derive(Clone, Default)]
pub struct ArgumentsHost {
    request_context: Option<Arc<RequestContext>>,
}

impl fmt::Debug for ArgumentsHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArgumentsHost")
            .field("has_request_context", &self.request_context.is_some())
            .finish()
    }
}

impl ArgumentsHost {
    /// Create an empty host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the shared request context to the host.
    pub fn with_request_context(mut self, request_context: RequestContext) -> Self {
        self.request_context = Some(Arc::new(request_context));
        self
    }

    /// Access the attached request context, when present.
    pub fn request_context(&self) -> Option<&RequestContext> {
        self.request_context.as_deref()
    }

    /// Look up typed request data from the shared request context.
    pub fn request<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.request_context()
            .and_then(|request_context| request_context.request_data::<T>())
    }
}

/// HTTP-specific alias for the default arguments host.
pub type HttpArgumentsHost = ArgumentsHost;

/// Runtime metadata used to order and match exception filters.
pub trait ExceptionFilterMetadata {
    /// Return the concrete exception type this filter targets, when known.
    fn exception_type(&self) -> Option<&'static str> {
        None
    }

    /// Whether this filter is an explicit catch-all fallback.
    fn is_catch_all(&self) -> bool {
        false
    }
}

/// Transport-neutral summary of an HTTP exception.
///
/// This keeps the filters crate free of any response type coupling while still
/// providing a stable default shape that HTTP adapters can turn into a response
/// body later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpExceptionSummary {
    pub status_code: u16,
    pub error: String,
    pub message: String,
}

impl From<&HttpException> for HttpExceptionSummary {
    fn from(exception: &HttpException) -> Self {
        Self {
            status_code: exception.status_code,
            error: exception.error.clone(),
            message: exception.message.clone(),
        }
    }
}

impl fmt::Display for HttpExceptionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.status_code, self.error, self.message)
    }
}

/// Build the default HTTP exception summary used by adapters and filters.
pub fn http_exception_summary(exception: &HttpException) -> HttpExceptionSummary {
    HttpExceptionSummary::from(exception)
}

/// Request exception filter surface.
///
/// The runtime hook is intentionally lightweight for now so the umbrella crate
/// can expose the API surface without coupling the filters crate to the HTTP
/// response type yet.
pub trait ExceptionFilter<E, R = HttpException>: Send + Sync {
    fn catch<'a>(&'a self, exception: E, host: HttpArgumentsHost) -> ExceptionFilterFuture<'a, R>;
}
