//! SCXML-safe request pipeline coordinator.
//!
//! The request lifecycle is defined in `statecharts/nivasa.request.scxml`.
//! This coordinator only advances the engine through typed
//! `NivasaRequestEvent` values and never mutates state directly.
//!
//! The full pipeline is:
//! `Received -> MiddlewareChain -> RouteMatching -> GuardChain -> InterceptorPre -> PipeTransform -> HandlerExecution -> InterceptorPost -> ErrorHandling -> SendingResponse -> Done`
//!
//! ```rust
//! use http::Method;
//! use nivasa_http::{Body, NivasaRequest, RequestPipeline};
//! use nivasa_statechart::NivasaRequestState;
//!
//! let request = NivasaRequest::new(Method::GET, "/health", Body::empty());
//! let pipeline = RequestPipeline::new(request);
//!
//! assert_eq!(pipeline.current_state(), NivasaRequestState::Received);
//! ```

use crate::NivasaRequest;
use nivasa_common::HttpException;
use nivasa_guards::{ExecutionContext, Guard};
use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry};
use nivasa_statechart::{
    InvalidTransitionError, NivasaRequestEvent, NivasaRequestState, NivasaRequestStatechart,
    StatechartEngine, StatechartSnapshot,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RequestEvent {
    RequestParsed,
    ErrorParse,
    MiddlewareComplete,
    ErrorMiddleware,
    RouteMatched,
    RouteNotFound,
    RouteMethodNotAllowed,
    GuardsPassed,
    GuardDenied,
    ErrorGuard,
    InterceptorsPreComplete,
    ErrorInterceptor,
    PipesComplete,
    ErrorValidation,
    ErrorPipe,
    HandlerComplete,
    ErrorHandler,
    InterceptorsPostComplete,
    ErrorInterceptorPost,
    FilterHandled,
    ErrorFilterUnhandled,
    ResponseSent,
    ErrorSend,
}

impl RequestEvent {
    const fn request_parsed() -> Self {
        Self::RequestParsed
    }

    const fn parse_error() -> Self {
        Self::ErrorParse
    }

    const fn middleware_complete() -> Self {
        Self::MiddlewareComplete
    }

    const fn middleware_error() -> Self {
        Self::ErrorMiddleware
    }

    const fn route_matched() -> Self {
        Self::RouteMatched
    }

    const fn route_not_found() -> Self {
        Self::RouteNotFound
    }

    const fn route_method_not_allowed() -> Self {
        Self::RouteMethodNotAllowed
    }

    const fn guards_passed() -> Self {
        Self::GuardsPassed
    }

    const fn guard_denied() -> Self {
        Self::GuardDenied
    }

    const fn guard_error() -> Self {
        Self::ErrorGuard
    }

    const fn interceptors_pre_complete() -> Self {
        Self::InterceptorsPreComplete
    }

    const fn interceptor_error() -> Self {
        Self::ErrorInterceptor
    }

    const fn pipes_complete() -> Self {
        Self::PipesComplete
    }

    const fn validation_error() -> Self {
        Self::ErrorValidation
    }

    const fn pipe_error() -> Self {
        Self::ErrorPipe
    }

    const fn handler_complete() -> Self {
        Self::HandlerComplete
    }

    const fn handler_error() -> Self {
        Self::ErrorHandler
    }

    const fn interceptors_post_complete() -> Self {
        Self::InterceptorsPostComplete
    }

    const fn interceptor_post_error() -> Self {
        Self::ErrorInterceptorPost
    }

    const fn filter_handled() -> Self {
        Self::FilterHandled
    }

    const fn filter_unhandled() -> Self {
        Self::ErrorFilterUnhandled
    }

    const fn response_sent() -> Self {
        Self::ResponseSent
    }

    const fn send_error() -> Self {
        Self::ErrorSend
    }

    fn for_route_outcome<'a, T>(outcome: &RouteDispatchOutcome<'a, T>) -> Self {
        match outcome {
            RouteDispatchOutcome::Matched(_) => Self::route_matched(),
            RouteDispatchOutcome::MethodNotAllowed { .. } => Self::route_method_not_allowed(),
            RouteDispatchOutcome::NotFound => Self::route_not_found(),
        }
    }
}

impl From<RequestEvent> for NivasaRequestEvent {
    fn from(value: RequestEvent) -> Self {
        match value {
            RequestEvent::RequestParsed => Self::RequestParsed,
            RequestEvent::ErrorParse => Self::ErrorParse,
            RequestEvent::MiddlewareComplete => Self::MiddlewareComplete,
            RequestEvent::ErrorMiddleware => Self::ErrorMiddleware,
            RequestEvent::RouteMatched => Self::RouteMatched,
            RequestEvent::RouteNotFound => Self::RouteNotFound,
            RequestEvent::RouteMethodNotAllowed => Self::RouteMethodNotAllowed,
            RequestEvent::GuardsPassed => Self::GuardsPassed,
            RequestEvent::GuardDenied => Self::GuardDenied,
            RequestEvent::ErrorGuard => Self::ErrorGuard,
            RequestEvent::InterceptorsPreComplete => Self::InterceptorsPreComplete,
            RequestEvent::ErrorInterceptor => Self::ErrorInterceptor,
            RequestEvent::PipesComplete => Self::PipesComplete,
            RequestEvent::ErrorValidation => Self::ErrorValidation,
            RequestEvent::ErrorPipe => Self::ErrorPipe,
            RequestEvent::HandlerComplete => Self::HandlerComplete,
            RequestEvent::ErrorHandler => Self::ErrorHandler,
            RequestEvent::InterceptorsPostComplete => Self::InterceptorsPostComplete,
            RequestEvent::ErrorInterceptorPost => Self::ErrorInterceptorPost,
            RequestEvent::FilterHandled => Self::FilterHandled,
            RequestEvent::ErrorFilterUnhandled => Self::ErrorFilterUnhandled,
            RequestEvent::ResponseSent => Self::ResponseSent,
            RequestEvent::ErrorSend => Self::ErrorSend,
        }
    }
}

/// Result of evaluating a guard against the request pipeline.
#[derive(Clone, Debug)]
pub enum GuardExecutionOutcome {
    Passed,
    Denied,
    Error(HttpException),
}

/// SCXML-safe request coordinator for request pipeline stages.
///
/// The pipeline stays honest to SCXML: each public method advances the
/// underlying statechart through a typed event, and route/guard helpers only
/// mutate request metadata when the SCXML transition says to move forward.
pub struct RequestPipeline {
    engine: StatechartEngine<NivasaRequestStatechart>,
    request: NivasaRequest,
}

impl RequestPipeline {
    /// Create a new request pipeline starting from the SCXML `Received` state.
    ///
    /// ```rust
    /// use http::Method;
    /// use nivasa_http::{Body, NivasaRequest, RequestPipeline};
    /// use nivasa_statechart::NivasaRequestState;
    ///
    /// let request = NivasaRequest::new(Method::GET, "/", Body::empty());
    /// let pipeline = RequestPipeline::new(request);
    ///
    /// assert_eq!(pipeline.current_state(), NivasaRequestState::Received);
    /// ```
    pub fn new(request: NivasaRequest) -> Self {
        Self {
            engine: StatechartEngine::new(NivasaRequestState::Received),
            request,
        }
    }

    /// Borrow the underlying request.
    pub fn request(&self) -> &NivasaRequest {
        &self.request
    }

    /// Mutably borrow the underlying request.
    pub fn request_mut(&mut self) -> &mut NivasaRequest {
        &mut self.request
    }

    /// Return the current SCXML state.
    ///
    /// ```rust
    /// use http::Method;
    /// use nivasa_http::{Body, NivasaRequest, RequestPipeline};
    /// use nivasa_statechart::NivasaRequestState;
    ///
    /// let request = NivasaRequest::new(Method::GET, "/", Body::empty());
    /// let pipeline = RequestPipeline::new(request);
    ///
    /// assert_eq!(pipeline.current_state(), NivasaRequestState::Received);
    /// ```
    pub fn current_state(&self) -> NivasaRequestState {
        self.engine.current_state()
    }

    /// Return a serializable statechart snapshot for debug inspection.
    ///
    /// ```rust
    /// use http::Method;
    /// use nivasa_http::{Body, NivasaRequest, RequestPipeline};
    ///
    /// let request = NivasaRequest::new(Method::GET, "/", Body::empty());
    /// let pipeline = RequestPipeline::new(request);
    /// let snapshot = pipeline.snapshot();
    ///
    /// assert_eq!(snapshot.current_state, "Received");
    /// assert!(snapshot.recent_transitions.is_empty());
    /// ```
    pub fn snapshot(&self) -> StatechartSnapshot {
        self.engine.snapshot(None)
    }

    /// Mark the request as parsed and advance the SCXML engine.
    pub fn parse_request(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::request_parsed())
    }

    /// Route the request into the SCXML error path from parse failure.
    pub fn fail_parse(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::parse_error())
    }

    /// Mark middleware as complete and advance to route matching.
    pub fn complete_middleware(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::middleware_complete())
    }

    /// Mark middleware as failed and enter the SCXML error path.
    pub fn fail_middleware(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::middleware_error())
    }

    /// Match the request against the routing registry and advance the engine.
    ///
    /// When route lookup succeeds, captured path params are copied into the
    /// request before the pipeline advances to the route-matching outcome.
    ///
    /// ```rust
    /// use http::Method;
    /// use nivasa_http::{Body, NivasaRequest, RequestPipeline};
    /// use nivasa_routing::RouteDispatchRegistry;
    ///
    /// let request = NivasaRequest::new(Method::GET, "/health", Body::empty());
    /// let mut pipeline = RequestPipeline::new(request);
    /// pipeline.parse_request().unwrap();
    /// pipeline.complete_middleware().unwrap();
    /// let routes = RouteDispatchRegistry::<()>::new();
    ///
    /// let outcome = pipeline.match_route(&routes).unwrap();
    /// assert!(matches!(outcome, nivasa_routing::RouteDispatchOutcome::NotFound));
    /// ```
    pub fn match_route<'a, T>(
        &mut self,
        routes: &'a RouteDispatchRegistry<T>,
    ) -> Result<RouteDispatchOutcome<'a, T>, InvalidTransitionError<NivasaRequestStatechart>> {
        let outcome = routes.dispatch(self.request.method().as_str(), self.request.path());

        match &outcome {
            RouteDispatchOutcome::Matched(_) => {
                if let Some(matched) =
                    routes.resolve_match(self.request.method().as_str(), self.request.path())
                {
                    self.request.set_path_params(matched.captures);
                } else {
                    self.request.clear_path_params();
                }
            }
            RouteDispatchOutcome::MethodNotAllowed { .. } => {
                self.request.clear_path_params();
            }
            RouteDispatchOutcome::NotFound => {
                self.request.clear_path_params();
            }
        }

        self.advance(RequestEvent::for_route_outcome(&outcome))?;
        Ok(outcome)
    }

    /// Mark guard evaluation as successful and continue the SCXML lifecycle.
    pub fn pass_guards(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::guards_passed())
    }

    /// Enter the SCXML error path because guard evaluation denied the request.
    pub fn deny_guards(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::guard_denied())
    }

    /// Enter the SCXML error path because guard evaluation failed.
    pub fn fail_guards(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::guard_error())
    }

    /// Evaluate a guard chain from the SCXML `GuardChain` state.
    ///
    /// Every guard must pass before the pipeline advances to `InterceptorPre`.
    /// The first deny or error short-circuits the chain and routes the pipeline
    /// through the existing SCXML error transitions.
    ///
    /// ```rust,no_run
    /// use nivasa_guards::ExecutionContext;
    /// use nivasa_http::{Body, GuardExecutionOutcome, NivasaRequest, RequestPipeline};
    /// use http::Method;
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let request = NivasaRequest::new(Method::GET, "/", Body::empty());
    /// let mut pipeline = RequestPipeline::new(request);
    /// let context = ExecutionContext::new(());
    ///
    /// let outcome = pipeline.evaluate_guard_chain(&[], &context).await.unwrap();
    /// assert!(matches!(outcome, GuardExecutionOutcome::Passed));
    /// # });
    /// ```
    pub async fn evaluate_guard_chain(
        &mut self,
        guards: &[&dyn Guard],
        context: &ExecutionContext,
    ) -> Result<GuardExecutionOutcome, InvalidTransitionError<NivasaRequestStatechart>> {
        for guard in guards {
            match guard.can_activate(context).await {
                Ok(true) => continue,
                Ok(false) => {
                    self.deny_guards()?;
                    return Ok(GuardExecutionOutcome::Denied);
                }
                Err(error) => {
                    self.fail_guards()?;
                    return Ok(GuardExecutionOutcome::Error(error));
                }
            }
        }

        self.pass_guards()?;
        Ok(GuardExecutionOutcome::Passed)
    }

    /// Evaluate a guard from the SCXML `GuardChain` state and advance along the
    /// pass, deny, or error transition that the guard result dictates.
    pub async fn evaluate_guard<G: Guard>(
        &mut self,
        guard: &G,
        context: &ExecutionContext,
    ) -> Result<GuardExecutionOutcome, InvalidTransitionError<NivasaRequestStatechart>> {
        let guard: &dyn Guard = guard;
        self.evaluate_guard_chain(&[guard], context).await
    }

    /// Mark interceptor pre-processing as complete.
    pub fn complete_interceptors_pre(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::interceptors_pre_complete())
    }

    /// Enter the SCXML error path because a pre-handler interceptor failed.
    pub fn fail_interceptors_pre(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::interceptor_error())
    }

    /// Mark pipe execution as complete.
    pub fn complete_pipes(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::pipes_complete())
    }

    /// Enter the SCXML error path because validation failed.
    pub fn fail_validation(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::validation_error())
    }

    /// Enter the SCXML error path because pipe execution failed.
    pub fn fail_pipes(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::pipe_error())
    }

    /// Mark handler execution as complete.
    pub fn complete_handler(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::handler_complete())
    }

    /// Enter the SCXML error path because handler execution failed.
    pub fn fail_handler(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::handler_error())
    }

    /// Mark interceptor post-processing as complete.
    pub fn complete_interceptors_post(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::interceptors_post_complete())
    }

    /// Enter the SCXML error path because a post-handler interceptor failed.
    pub fn fail_interceptors_post(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::interceptor_post_error())
    }

    /// Mark error filters as having produced a response.
    pub fn handle_filter(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::filter_handled())
    }

    /// Advance from error handling when no filter handled the failure.
    pub fn fail_filter_unhandled(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::filter_unhandled())
    }

    /// Mark response sending as complete.
    pub fn complete_response(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::response_sent())
    }

    /// Advance through the SCXML send-error branch.
    pub fn fail_send(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(RequestEvent::send_error())
    }

    fn advance(
        &mut self,
        event: RequestEvent,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.engine.send_event(event.into())
    }
}
