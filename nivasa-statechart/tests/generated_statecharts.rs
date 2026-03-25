use nivasa_statechart::codegen;
use nivasa_statechart::parser::ScxmlDocument;
use nivasa_statechart::{
    GENERATED_STATECHARTS, StatechartEngine, StatechartTracer,
    NivasaApplicationEvent, NivasaApplicationState, NivasaApplicationStatechart,
    NivasaModuleEvent, NivasaModuleState, NivasaModuleStatechart,
    NivasaProviderEvent, NivasaProviderState, NivasaProviderStatechart,
    NivasaRequestEvent, NivasaRequestState, NivasaRequestStatechart,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn statecharts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../statecharts")
}

fn drive<S>(engine: &mut StatechartEngine<S>, steps: &[(S::Event, S::State)])
where
    S: nivasa_statechart::StatechartSpec,
    S::State: Copy + std::fmt::Debug + PartialEq,
    S::Event: Clone + std::fmt::Debug,
{
    for (event, expected) in steps {
        let observed = engine.send_event(event.clone()).unwrap();
        assert_eq!(observed, *expected);
        assert_eq!(engine.current_state(), *expected);
    }
}

#[test]
fn generated_registry_matches_source_scxml_hashes() {
    for generated in GENERATED_STATECHARTS {
        let path = statecharts_dir().join(generated.file_name);
        let doc = ScxmlDocument::from_file(&path).unwrap();

        assert_eq!(
            doc.content_hash(),
            generated.scxml_hash,
            "hash mismatch for {}",
            generated.file_name,
        );

        let rendered = codegen::generate_rust_with_spec_path(&doc, "crate");
        assert!(
            rendered.contains(&format!("pub struct {}Statechart;", generated.name)),
            "missing generated statechart type for {}",
            generated.file_name,
        );
        assert!(
            rendered.contains(generated.scxml_hash),
            "missing embedded hash for {}",
            generated.file_name,
        );
    }
}

#[test]
fn application_bootstrap_path_reaches_running() {
    let mut engine = StatechartEngine::<NivasaApplicationStatechart>::new(
        NivasaApplicationState::ResolvingModules,
    );

    assert_eq!(
        engine.valid_events(),
        vec![
            NivasaApplicationEvent::ModulesResolved,
            NivasaApplicationEvent::ErrorModuleCircularDependency,
        ],
    );

    drive(
        &mut engine,
        &[
            (
                NivasaApplicationEvent::ModulesResolved,
                NivasaApplicationState::InitializingModules,
            ),
            (
                NivasaApplicationEvent::ModulesInitialized,
                NivasaApplicationState::RegisteringRoutes,
            ),
            (
                NivasaApplicationEvent::RoutesRegistered,
                NivasaApplicationState::ApplyingGlobalConfig,
            ),
            (
                NivasaApplicationEvent::ConfigApplied,
                NivasaApplicationState::BootstrapComplete,
            ),
            (
                NivasaApplicationEvent::AppStart,
                NivasaApplicationState::Running,
            ),
        ],
    );

    assert!(!engine.is_in_final_state());
}

#[test]
fn application_failure_path_reaches_terminated() {
    let mut engine = StatechartEngine::<NivasaApplicationStatechart>::new(
        NivasaApplicationState::ResolvingModules,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaApplicationEvent::ErrorModuleCircularDependency,
                NivasaApplicationState::BootstrapFailed,
            ),
            (
                NivasaApplicationEvent::AppAbort,
                NivasaApplicationState::Terminated,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn module_happy_path_reaches_destroyed() {
    let mut engine = StatechartEngine::<NivasaModuleStatechart>::new(
        NivasaModuleState::ResolvingImports,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaModuleEvent::ImportsResolved,
                NivasaModuleState::RegisteringProviders,
            ),
            (
                NivasaModuleEvent::ProvidersRegistered,
                NivasaModuleState::ResolvingDependencies,
            ),
            (
                NivasaModuleEvent::DependenciesResolved,
                NivasaModuleState::Loaded,
            ),
            (
                NivasaModuleEvent::ModuleInit,
                NivasaModuleState::Initialized,
            ),
            (
                NivasaModuleEvent::ModuleActivate,
                NivasaModuleState::Active,
            ),
            (
                NivasaModuleEvent::ModuleDestroy,
                NivasaModuleState::Destroying,
            ),
            (
                NivasaModuleEvent::DestroyComplete,
                NivasaModuleState::Destroyed,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn module_load_failure_short_circuits_to_failed() {
    let mut engine = StatechartEngine::<NivasaModuleStatechart>::new(
        NivasaModuleState::ResolvingImports,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaModuleEvent::ErrorImportMissing,
                NivasaModuleState::LoadFailed,
            ),
            (
                NivasaModuleEvent::ModuleAbort,
                NivasaModuleState::Failed,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn provider_happy_path_reaches_disposed() {
    let mut engine = StatechartEngine::<NivasaProviderStatechart>::new(
        NivasaProviderState::Unregistered,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaProviderEvent::ProviderRegister,
                NivasaProviderState::Registered,
            ),
            (
                NivasaProviderEvent::ProviderResolve,
                NivasaProviderState::Resolving,
            ),
            (
                NivasaProviderEvent::DependenciesReady,
                NivasaProviderState::Constructing,
            ),
            (
                NivasaProviderEvent::ProviderConstructed,
                NivasaProviderState::Resolved,
            ),
            (
                NivasaProviderEvent::ProviderDispose,
                NivasaProviderState::Disposing,
            ),
            (
                NivasaProviderEvent::ProviderDisposed,
                NivasaProviderState::Disposed,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn provider_resolution_failure_short_circuits_to_failed() {
    let mut engine = StatechartEngine::<NivasaProviderStatechart>::new(
        NivasaProviderState::Resolving,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaProviderEvent::ErrorDependencyMissing,
                NivasaProviderState::ResolutionFailed,
            ),
            (
                NivasaProviderEvent::ProviderAbort,
                NivasaProviderState::Failed,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn request_happy_path_reaches_done() {
    let mut engine = StatechartEngine::<NivasaRequestStatechart>::new(
        NivasaRequestState::Received,
    );

    assert_eq!(
        engine.valid_events(),
        vec![
            NivasaRequestEvent::RequestParsed,
            NivasaRequestEvent::ErrorParse,
        ],
    );

    drive(
        &mut engine,
        &[
            (
                NivasaRequestEvent::RequestParsed,
                NivasaRequestState::MiddlewareChain,
            ),
            (
                NivasaRequestEvent::MiddlewareComplete,
                NivasaRequestState::RouteMatching,
            ),
            (
                NivasaRequestEvent::RouteMatched,
                NivasaRequestState::GuardChain,
            ),
            (
                NivasaRequestEvent::GuardsPassed,
                NivasaRequestState::InterceptorPre,
            ),
            (
                NivasaRequestEvent::InterceptorsPreComplete,
                NivasaRequestState::PipeTransform,
            ),
            (
                NivasaRequestEvent::PipesComplete,
                NivasaRequestState::HandlerExecution,
            ),
            (
                NivasaRequestEvent::HandlerComplete,
                NivasaRequestState::InterceptorPost,
            ),
            (
                NivasaRequestEvent::InterceptorsPostComplete,
                NivasaRequestState::SendingResponse,
            ),
            (
                NivasaRequestEvent::ResponseSent,
                NivasaRequestState::Done,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn request_guard_denied_short_circuits_to_done() {
    let mut engine = StatechartEngine::<NivasaRequestStatechart>::new(
        NivasaRequestState::GuardChain,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaRequestEvent::GuardDenied,
                NivasaRequestState::ErrorHandling,
            ),
            (
                NivasaRequestEvent::FilterHandled,
                NivasaRequestState::SendingResponse,
            ),
            (
                NivasaRequestEvent::ErrorSend,
                NivasaRequestState::Done,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
#[should_panic(expected = "SCXML violation")]
fn invalid_event_in_generated_application_state_panics_in_debug() {
    let mut engine =
        StatechartEngine::<NivasaApplicationStatechart>::new(NivasaApplicationState::Created);
    let _ = engine.send_event(NivasaApplicationEvent::AppStart);
}

#[test]
fn request_validation_error_short_circuits_to_done() {
    let mut engine = StatechartEngine::<NivasaRequestStatechart>::new(
        NivasaRequestState::PipeTransform,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaRequestEvent::ErrorValidation,
                NivasaRequestState::ErrorHandling,
            ),
            (
                NivasaRequestEvent::FilterHandled,
                NivasaRequestState::SendingResponse,
            ),
            (
                NivasaRequestEvent::ErrorSend,
                NivasaRequestState::Done,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn request_handler_error_short_circuits_to_done() {
    let mut engine = StatechartEngine::<NivasaRequestStatechart>::new(
        NivasaRequestState::HandlerExecution,
    );

    drive(
        &mut engine,
        &[
            (
                NivasaRequestEvent::ErrorHandler,
                NivasaRequestState::ErrorHandling,
            ),
            (
                NivasaRequestEvent::FilterHandled,
                NivasaRequestState::SendingResponse,
            ),
            (
                NivasaRequestEvent::ErrorSend,
                NivasaRequestState::Done,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn tracer_receives_generated_request_transitions() {
    #[derive(Clone, Default)]
    struct RecordingTracer {
        events: Arc<Mutex<Vec<(String, String, String)>>>,
    }

    impl StatechartTracer for RecordingTracer {
        fn on_transition(&self, from: &str, event: &str, to: &str) {
            self.events.lock().unwrap().push((
                from.to_string(),
                event.to_string(),
                to.to_string(),
            ));
        }

        fn on_invalid_transition(&self, _from: &str, _event: &str, _valid: &[String]) {}
    }

    let tracer = RecordingTracer::default();
    let events = tracer.events.clone();
    let mut engine = StatechartEngine::<NivasaRequestStatechart>::with_tracer(
        NivasaRequestState::Received,
        Box::new(tracer),
    );

    drive(
        &mut engine,
        &[
            (
                NivasaRequestEvent::RequestParsed,
                NivasaRequestState::MiddlewareChain,
            ),
            (
                NivasaRequestEvent::MiddlewareComplete,
                NivasaRequestState::RouteMatching,
            ),
        ],
    );

    let log = events.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].0, "Received");
    assert_eq!(log[0].1, "RequestParsed");
    assert_eq!(log[0].2, "MiddlewareChain");
    assert_eq!(log[1].2, "RouteMatching");
    assert_eq!(engine.recent_transitions().len(), 2);
}
