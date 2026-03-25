//! SCXML-safe request pipeline coordinator.
//!
//! The request lifecycle is defined in `statecharts/nivasa.request.scxml`.
//! This coordinator only advances the engine through typed
//! `NivasaRequestEvent` values and never mutates state directly.
//!
//! The full pipeline is:
//! `Received -> MiddlewareChain -> RouteMatching -> GuardChain -> InterceptorPre -> PipeTransform -> HandlerExecution -> InterceptorPost -> ErrorHandling -> SendingResponse -> Done`

use crate::NivasaRequest;
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
        }
    }
}

/// SCXML-safe request coordinator for the first request pipeline stages.
pub struct RequestPipeline {
    engine: StatechartEngine<NivasaRequestStatechart>,
    request: NivasaRequest,
}

impl RequestPipeline {
    /// Create a new request pipeline starting from the SCXML `Received` state.
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

    /// Return the current SCXML state.
    pub fn current_state(&self) -> NivasaRequestState {
        self.engine.current_state()
    }

    /// Return a serializable statechart snapshot for debug inspection.
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

    fn advance(
        &mut self,
        event: RequestEvent,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.engine.send_event(event.into())
    }
}
