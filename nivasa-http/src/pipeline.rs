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
        self.advance(self.request_parsed_event())
    }

    /// Route the request into the SCXML error path from parse failure.
    pub fn fail_parse(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(self.parse_error_event())
    }

    /// Mark middleware as complete and advance to route matching.
    pub fn complete_middleware(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(self.middleware_complete_event())
    }

    /// Mark middleware as failed and enter the SCXML error path.
    pub fn fail_middleware(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(self.middleware_error_event())
    }

    /// Match the request against the routing registry and advance the engine.
    pub fn match_route<'a, T>(
        &mut self,
        routes: &'a RouteDispatchRegistry<T>,
    ) -> Result<RouteDispatchOutcome<'a, T>, InvalidTransitionError<NivasaRequestStatechart>> {
        let outcome = routes.dispatch(self.request.method().as_str(), self.request.path());

        match &outcome {
            RouteDispatchOutcome::Matched(_) => {
                self.advance(self.route_matched_event())?;
            }
            RouteDispatchOutcome::MethodNotAllowed { .. } => {
                self.advance(self.route_method_not_allowed_event())?;
            }
            RouteDispatchOutcome::NotFound => {
                self.advance(self.route_not_found_event())?;
            }
        }

        Ok(outcome)
    }

    fn advance(
        &mut self,
        event: NivasaRequestEvent,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.engine.send_event(event)
    }

    fn request_parsed_event(&self) -> NivasaRequestEvent {
        NivasaRequestEvent::RequestParsed
    }

    fn parse_error_event(&self) -> NivasaRequestEvent {
        NivasaRequestEvent::ErrorParse
    }

    fn middleware_complete_event(&self) -> NivasaRequestEvent {
        NivasaRequestEvent::MiddlewareComplete
    }

    fn middleware_error_event(&self) -> NivasaRequestEvent {
        NivasaRequestEvent::ErrorMiddleware
    }

    fn route_matched_event(&self) -> NivasaRequestEvent {
        NivasaRequestEvent::RouteMatched
    }

    fn route_not_found_event(&self) -> NivasaRequestEvent {
        NivasaRequestEvent::RouteNotFound
    }

    fn route_method_not_allowed_event(&self) -> NivasaRequestEvent {
        NivasaRequestEvent::RouteMethodNotAllowed
    }
}
