use nivasa_statechart::codegen;
use nivasa_statechart::parser::ScxmlDocument;
use nivasa_statechart::{
    NivasaApplicationEvent, NivasaApplicationState, NivasaApplicationStatechart, NivasaModuleEvent,
    NivasaModuleState, NivasaModuleStatechart, NivasaProviderEvent, NivasaProviderState,
    NivasaProviderStatechart, NivasaRequestEvent, NivasaRequestState, NivasaRequestStatechart,
    StatechartEngine, StatechartTracer, TransitionKind, GENERATED_STATECHARTS,
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

type ValidTrace = (String, String, String);
type InvalidTrace = (String, String, Vec<String>);

#[derive(Clone, Default)]
struct RecordingTracer {
    transitions: Arc<Mutex<Vec<ValidTrace>>>,
    invalid_transitions: Arc<Mutex<Vec<InvalidTrace>>>,
}

impl StatechartTracer for RecordingTracer {
    fn on_transition(&self, from: &str, event: &str, to: &str) {
        self.transitions.lock().unwrap().push((
            from.to_string(),
            event.to_string(),
            to.to_string(),
        ));
    }

    fn on_invalid_transition(&self, from: &str, event: &str, valid: &[String]) {
        self.invalid_transitions.lock().unwrap().push((
            from.to_string(),
            event.to_string(),
            valid.to_vec(),
        ));
    }
}

fn traced_engine<S>(initial_state: S::State) -> (StatechartEngine<S>, RecordingTracer)
where
    S: nivasa_statechart::StatechartSpec,
{
    let tracer = RecordingTracer::default();
    let engine = StatechartEngine::<S>::with_tracer(initial_state, Box::new(tracer.clone()));
    (engine, tracer)
}

fn assert_valid_trace<S>(
    engine: &StatechartEngine<S>,
    tracer: &RecordingTracer,
    expected: &[ValidTrace],
) where
    S: nivasa_statechart::StatechartSpec,
{
    assert_eq!(*tracer.transitions.lock().unwrap(), expected);
    assert!(tracer.invalid_transitions.lock().unwrap().is_empty());

    let recent = engine.recent_transitions();
    assert_eq!(recent.len(), expected.len());
    assert!(recent
        .iter()
        .all(|record| record.kind == TransitionKind::Valid));

    let recent = recent
        .into_iter()
        .map(|record| {
            (
                record.from,
                record.event,
                record.to.expect("valid transition must record a target"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(recent, expected);
}

fn assert_invalid_trace<S>(
    engine: &StatechartEngine<S>,
    tracer: &RecordingTracer,
    expected: &[InvalidTrace],
) where
    S: nivasa_statechart::StatechartSpec,
{
    assert!(tracer.transitions.lock().unwrap().is_empty());
    assert_eq!(*tracer.invalid_transitions.lock().unwrap(), expected);

    let recent = engine.recent_transitions();
    assert_eq!(recent.len(), expected.len());
    assert!(recent
        .iter()
        .all(|record| record.kind == TransitionKind::Invalid));

    let recent = recent
        .into_iter()
        .map(|record| (record.from, record.event, record.valid_events))
        .collect::<Vec<_>>();
    assert_eq!(recent, expected);
}

fn drive_and_assert_trace<S>(
    engine: &mut StatechartEngine<S>,
    tracer: &RecordingTracer,
    steps: &[(S::Event, S::State)],
) where
    S: nivasa_statechart::StatechartSpec,
    S::State: Copy + std::fmt::Debug + PartialEq,
    S::Event: Clone + std::fmt::Debug,
{
    let mut from = format!("{:?}", engine.current_state());
    let expected = steps
        .iter()
        .map(|(event, to)| {
            let trace = (from.clone(), format!("{:?}", event), format!("{:?}", to));
            from = format!("{:?}", to);
            trace
        })
        .collect::<Vec<_>>();

    drive(engine, steps);
    assert_valid_trace(engine, tracer, &expected);
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }

    if let Some(message) = payload.downcast_ref::<&'static str>() {
        return (*message).to_string();
    }

    "<non-string panic payload>".to_string()
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
    let (mut engine, tracer) =
        traced_engine::<NivasaApplicationStatechart>(NivasaApplicationState::ResolvingModules);

    assert_eq!(
        engine.valid_events(),
        vec![
            NivasaApplicationEvent::ModulesResolved,
            NivasaApplicationEvent::ErrorModuleCircularDependency,
        ],
    );

    drive_and_assert_trace(
        &mut engine,
        &tracer,
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
                NivasaApplicationState::Listening,
            ),
        ],
    );

    assert!(!engine.is_in_final_state());
}

#[test]
fn application_container_state_enters_its_initial_child() {
    let (engine, tracer) =
        traced_engine::<NivasaApplicationStatechart>(NivasaApplicationState::Running);

    assert_eq!(engine.current_state(), NivasaApplicationState::Listening);
    assert_eq!(
        engine.valid_events(),
        vec![NivasaApplicationEvent::AppShutdown],
    );
    assert_valid_trace(&engine, &tracer, &[]);
}

#[test]
fn application_container_states_enter_their_initial_children() {
    let (engine, tracer) =
        traced_engine::<NivasaApplicationStatechart>(NivasaApplicationState::Bootstrapping);
    assert_eq!(
        engine.current_state(),
        NivasaApplicationState::ResolvingModules
    );
    assert_eq!(
        engine.valid_events(),
        vec![
            NivasaApplicationEvent::ModulesResolved,
            NivasaApplicationEvent::ErrorModuleCircularDependency,
        ],
    );
    assert_valid_trace(&engine, &tracer, &[]);

    let (engine, tracer) =
        traced_engine::<NivasaApplicationStatechart>(NivasaApplicationState::ShuttingDown);
    assert_eq!(
        engine.current_state(),
        NivasaApplicationState::DestroyingModules
    );
    assert_eq!(
        engine.valid_events(),
        vec![NivasaApplicationEvent::ModulesDestroyed],
    );
    assert_valid_trace(&engine, &tracer, &[]);
}

#[test]
fn application_failure_path_reaches_terminated() {
    let (mut engine, tracer) =
        traced_engine::<NivasaApplicationStatechart>(NivasaApplicationState::ResolvingModules);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
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
fn application_shutdown_path_reaches_shutdown_complete() {
    let (mut engine, tracer) =
        traced_engine::<NivasaApplicationStatechart>(NivasaApplicationState::Listening);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
        &[
            (
                NivasaApplicationEvent::AppShutdown,
                NivasaApplicationState::Draining,
            ),
            (
                NivasaApplicationEvent::AppDrainComplete,
                NivasaApplicationState::DestroyingModules,
            ),
            (
                NivasaApplicationEvent::ModulesDestroyed,
                NivasaApplicationState::Cleanup,
            ),
            (
                NivasaApplicationEvent::CleanupDone,
                NivasaApplicationState::ShutdownComplete,
            ),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn application_error_transitions_short_circuit_to_bootstrap_failed() {
    for (initial, event, from) in [
        (
            NivasaApplicationState::ResolvingModules,
            NivasaApplicationEvent::ErrorModuleCircularDependency,
            "ResolvingModules",
        ),
        (
            NivasaApplicationState::InitializingModules,
            NivasaApplicationEvent::ErrorModuleInitFailed,
            "InitializingModules",
        ),
        (
            NivasaApplicationState::RegisteringRoutes,
            NivasaApplicationEvent::ErrorRouteConflict,
            "RegisteringRoutes",
        ),
    ] {
        let (mut engine, tracer) = traced_engine::<NivasaApplicationStatechart>(initial);
        let event_name = format!("{event:?}");

        drive_and_assert_trace(
            &mut engine,
            &tracer,
            &[(event, NivasaApplicationState::BootstrapFailed)],
        );

        assert_eq!(
            engine.current_state(),
            NivasaApplicationState::BootstrapFailed
        );
        assert_eq!(
            *tracer.transitions.lock().unwrap(),
            vec![(from.to_string(), event_name, "BootstrapFailed".to_string(),)],
        );
    }
}

#[test]
fn module_happy_path_reaches_destroyed() {
    let (mut engine, tracer) = traced_engine::<NivasaModuleStatechart>(NivasaModuleState::Unloaded);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
        &[
            (
                NivasaModuleEvent::ModuleLoad,
                NivasaModuleState::ResolvingImports,
            ),
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
            (NivasaModuleEvent::ModuleActivate, NivasaModuleState::Active),
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
fn module_error_transitions_short_circuit_to_load_failed() {
    for (initial, event, from) in [
        (
            NivasaModuleState::ResolvingImports,
            NivasaModuleEvent::ErrorImportMissing,
            "ResolvingImports",
        ),
        (
            NivasaModuleState::ResolvingDependencies,
            NivasaModuleEvent::ErrorDiCircular,
            "ResolvingDependencies",
        ),
        (
            NivasaModuleState::ResolvingDependencies,
            NivasaModuleEvent::ErrorDiMissingProvider,
            "ResolvingDependencies",
        ),
    ] {
        let (mut engine, tracer) = traced_engine::<NivasaModuleStatechart>(initial);
        let event_name = format!("{event:?}");

        drive_and_assert_trace(
            &mut engine,
            &tracer,
            &[(event, NivasaModuleState::LoadFailed)],
        );

        assert_eq!(engine.current_state(), NivasaModuleState::LoadFailed);
        assert_eq!(
            *tracer.transitions.lock().unwrap(),
            vec![(from.to_string(), event_name, "LoadFailed".to_string(),)],
        );
    }
}

#[test]
fn module_load_failure_short_circuits_to_failed() {
    let (mut engine, tracer) = traced_engine::<NivasaModuleStatechart>(NivasaModuleState::Unloaded);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
        &[
            (
                NivasaModuleEvent::ModuleLoad,
                NivasaModuleState::ResolvingImports,
            ),
            (
                NivasaModuleEvent::ErrorImportMissing,
                NivasaModuleState::LoadFailed,
            ),
            (NivasaModuleEvent::ModuleAbort, NivasaModuleState::Failed),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn module_container_state_enters_its_initial_child() {
    let (engine, tracer) = traced_engine::<NivasaModuleStatechart>(NivasaModuleState::Loading);

    assert_eq!(engine.current_state(), NivasaModuleState::ResolvingImports);
    assert_eq!(
        engine.valid_events(),
        vec![
            NivasaModuleEvent::ImportsResolved,
            NivasaModuleEvent::ErrorImportMissing
        ]
    );
    assert_valid_trace(&engine, &tracer, &[]);
}

#[test]
fn provider_error_transitions_short_circuit_to_resolution_failed() {
    for (initial, event, from) in [
        (
            NivasaProviderState::Resolving,
            NivasaProviderEvent::ErrorDependencyMissing,
            "Resolving",
        ),
        (
            NivasaProviderState::Resolving,
            NivasaProviderEvent::ErrorDependencyCircular,
            "Resolving",
        ),
        (
            NivasaProviderState::Constructing,
            NivasaProviderEvent::ErrorConstruction,
            "Constructing",
        ),
    ] {
        let (mut engine, tracer) = traced_engine::<NivasaProviderStatechart>(initial);
        let event_name = format!("{event:?}");

        drive_and_assert_trace(
            &mut engine,
            &tracer,
            &[(event, NivasaProviderState::ResolutionFailed)],
        );

        assert_eq!(
            engine.current_state(),
            NivasaProviderState::ResolutionFailed
        );
        assert_eq!(
            *tracer.transitions.lock().unwrap(),
            vec![(from.to_string(), event_name, "ResolutionFailed".to_string(),)],
        );
    }
}

#[test]
fn provider_happy_path_reaches_disposed() {
    let (mut engine, tracer) =
        traced_engine::<NivasaProviderStatechart>(NivasaProviderState::Unregistered);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
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
    let (mut engine, tracer) =
        traced_engine::<NivasaProviderStatechart>(NivasaProviderState::Resolving);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
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
    let (mut engine, tracer) =
        traced_engine::<NivasaRequestStatechart>(NivasaRequestState::Received);

    assert_eq!(
        engine.valid_events(),
        vec![
            NivasaRequestEvent::RequestParsed,
            NivasaRequestEvent::ErrorParse,
        ],
    );

    drive_and_assert_trace(
        &mut engine,
        &tracer,
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
            (NivasaRequestEvent::ResponseSent, NivasaRequestState::Done),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn request_guard_denied_short_circuits_to_done() {
    let (mut engine, tracer) =
        traced_engine::<NivasaRequestStatechart>(NivasaRequestState::GuardChain);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
        &[
            (
                NivasaRequestEvent::GuardDenied,
                NivasaRequestState::ErrorHandling,
            ),
            (
                NivasaRequestEvent::FilterHandled,
                NivasaRequestState::SendingResponse,
            ),
            (NivasaRequestEvent::ErrorSend, NivasaRequestState::Done),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn invalid_event_in_generated_application_state_panics_in_debug() {
    let (mut engine, tracer) =
        traced_engine::<NivasaApplicationStatechart>(NivasaApplicationState::Created);
    let expected_valid_events = engine
        .valid_events()
        .into_iter()
        .map(|event| format!("{:?}", event))
        .collect::<Vec<_>>();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = engine.send_event(NivasaApplicationEvent::AppStart);
    }))
    .expect_err("invalid generated transition must panic in debug");

    assert!(panic_message(panic).contains("SCXML violation"));
    assert_invalid_trace(
        &engine,
        &tracer,
        &[(
            "Created".to_string(),
            "AppStart".to_string(),
            expected_valid_events,
        )],
    );
}

#[test]
fn invalid_event_in_generated_request_state_records_invalid_trace() {
    let (mut engine, tracer) =
        traced_engine::<NivasaRequestStatechart>(NivasaRequestState::Received);
    let expected_valid_events = engine
        .valid_events()
        .into_iter()
        .map(|event| format!("{:?}", event))
        .collect::<Vec<_>>();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = engine.send_event(NivasaRequestEvent::ResponseSent);
    }))
    .expect_err("invalid generated transition must panic in debug");

    assert!(panic_message(panic).contains("SCXML violation"));
    assert_invalid_trace(
        &engine,
        &tracer,
        &[(
            "Received".to_string(),
            "ResponseSent".to_string(),
            expected_valid_events.clone(),
        )],
    );

    let recent = engine.recent_transitions();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].kind, TransitionKind::Invalid);
    assert_eq!(recent[0].from, "Received");
    assert_eq!(recent[0].event, "ResponseSent");
    assert_eq!(recent[0].valid_events, expected_valid_events);
}

#[test]
fn request_validation_error_short_circuits_to_done() {
    let (mut engine, tracer) =
        traced_engine::<NivasaRequestStatechart>(NivasaRequestState::PipeTransform);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
        &[
            (
                NivasaRequestEvent::ErrorValidation,
                NivasaRequestState::ErrorHandling,
            ),
            (
                NivasaRequestEvent::FilterHandled,
                NivasaRequestState::SendingResponse,
            ),
            (NivasaRequestEvent::ErrorSend, NivasaRequestState::Done),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn request_handler_error_short_circuits_to_done() {
    let (mut engine, tracer) =
        traced_engine::<NivasaRequestStatechart>(NivasaRequestState::HandlerExecution);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
        &[
            (
                NivasaRequestEvent::ErrorHandler,
                NivasaRequestState::ErrorHandling,
            ),
            (
                NivasaRequestEvent::FilterHandled,
                NivasaRequestState::SendingResponse,
            ),
            (NivasaRequestEvent::ErrorSend, NivasaRequestState::Done),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn request_handler_stage_interceptor_error_short_circuits_to_done() {
    let (mut engine, tracer) =
        traced_engine::<NivasaRequestStatechart>(NivasaRequestState::HandlerExecution);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
        &[
            (
                NivasaRequestEvent::ErrorInterceptor,
                NivasaRequestState::ErrorHandling,
            ),
            (
                NivasaRequestEvent::FilterHandled,
                NivasaRequestState::SendingResponse,
            ),
            (NivasaRequestEvent::ErrorSend, NivasaRequestState::Done),
        ],
    );

    assert!(engine.is_in_final_state());
}

#[test]
fn tracer_receives_generated_request_transitions() {
    let (mut engine, tracer) =
        traced_engine::<NivasaRequestStatechart>(NivasaRequestState::Received);

    drive_and_assert_trace(
        &mut engine,
        &tracer,
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
    assert_eq!(engine.recent_transitions().len(), 2);
}
