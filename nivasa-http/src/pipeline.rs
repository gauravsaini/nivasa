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
        self.advance(NivasaRequestEvent::RequestParsed)
    }

    /// Mark middleware as complete and advance to route matching.
    pub fn complete_middleware(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(NivasaRequestEvent::MiddlewareComplete)
    }

    /// Mark middleware as failed and enter the SCXML error path.
    pub fn fail_middleware(
        &mut self,
    ) -> Result<NivasaRequestState, InvalidTransitionError<NivasaRequestStatechart>> {
        self.advance(NivasaRequestEvent::ErrorMiddleware)
    }

    /// Match the request against the routing registry and advance the engine.
    pub fn match_route<'a, T>(
        &mut self,
        routes: &'a RouteDispatchRegistry<T>,
    ) -> Result<RouteDispatchOutcome<'a, T>, InvalidTransitionError<NivasaRequestStatechart>> {
        let outcome = routes.dispatch(self.request.method().as_str(), self.request.path());

        match &outcome {
            RouteDispatchOutcome::Matched(_) => {
                self.advance(NivasaRequestEvent::RouteMatched)?;
            }
            RouteDispatchOutcome::MethodNotAllowed { .. } => {
                self.advance(NivasaRequestEvent::RouteMethodNotAllowed)?;
            }
            RouteDispatchOutcome::NotFound => {
                self.advance(NivasaRequestEvent::RouteNotFound)?;
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
}
