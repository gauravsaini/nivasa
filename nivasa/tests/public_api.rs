use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
#[allow(unused_imports)]
use nivasa::filters as filters_crate;
#[allow(unused_imports)]
use nivasa::pipes as pipes_crate;
use nivasa::prelude::*;
#[allow(unused_imports)]
use nivasa::prelude::{
    all, body, controller, custom_param, delete, file, files, get, head, header, headers,
    http_code, impl_controller, injectable, ip, module, options, param, patch, post, put, query,
    req, res, scxml_handler, session, App, AppBuildError, AppRoute, ArgumentMetadata,
    ArgumentsHost, EmptyMutation, EmptySubscription, ExceptionFilter, ExceptionFilterFuture,
    GraphQLCoreModule, GraphQLError, GraphQLModule, GraphQLRequest, GraphQLResponse, GraphQLSchema,
    HttpArgumentsHost, InvalidHttpStatus, Middleware, NivasaMiddlewareLayer, Pipe, Reflector,
    RequestContext, TestClient, TestResponse, WsArgumentsHost,
};
use std::any::TypeId;
use std::error::Error;
use std::future::Future;
use std::net::TcpListener as StdTcpListener;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::time::{sleep, Duration};

#[test]
fn crate_root_reexports_app_config_builders() {
    let versioning = nivasa::VersioningOptions::builder(nivasa::VersioningStrategy::Header)
        .default_version(" 1 ")
        .build();
    let server = nivasa::ServerOptions::builder()
        .host("0.0.0.0")
        .port(8080)
        .enable_cors()
        .global_prefix("api")
        .versioning(versioning.clone())
        .build();

    assert_eq!(server.host, "0.0.0.0");
    assert_eq!(server.port, 8080);
    assert!(server.cors);
    assert_eq!(server.global_prefix.as_deref(), Some("/api"));
    assert_eq!(
        server.versioning.as_ref().map(|options| options.strategy),
        Some(nivasa::VersioningStrategy::Header)
    );
    assert_eq!(
        server
            .versioning
            .as_ref()
            .and_then(|options| options.default_version.as_deref()),
        Some("v1")
    );
    assert_eq!(versioning.default_version.as_deref(), Some("v1"));
}

#[test]
fn prelude_reexports_app_config_types_for_downstream_use() {
    let server = ServerOptions::builder()
        .versioning(
            VersioningOptions::builder(VersioningStrategy::MediaType)
                .default_version("/v2/")
                .build(),
        )
        .build();

    assert_eq!(server.host, "127.0.0.1");
    assert_eq!(server.port, 3000);
    assert_eq!(
        server.versioning.as_ref().map(|options| options.strategy),
        Some(VersioningStrategy::MediaType)
    );
    assert_eq!(
        server
            .versioning
            .as_ref()
            .and_then(|options| options.default_version.as_deref()),
        Some("v2")
    );
}

#[test]
fn prelude_reexports_common_request_and_status_types() {
    let mut context = RequestContext::new();
    context.insert_request_data(String::from("req-123"));

    let invalid_status = InvalidHttpStatus(599);
    let root_context = nivasa::RequestContext::new();
    let root_invalid_status = nivasa::InvalidHttpStatus(599);

    assert_eq!(
        context.request_data::<String>().map(String::as_str),
        Some("req-123")
    );
    assert_eq!(invalid_status.0, 599);
    assert!(root_context.request_data::<String>().is_none());
    assert_eq!(root_invalid_status.0, 599);
}

#[test]
fn builder_defaults_match_the_existing_config_surface() {
    let server = ServerOptions::builder().build();

    assert_eq!(server.host, "127.0.0.1");
    assert_eq!(server.port, 3000);
    assert!(!server.cors);
    assert_eq!(server.global_prefix, None);
    assert_eq!(server.versioning, None);
}

#[test]
fn crate_root_reexports_bootstrap_config_as_pure_data() {
    let server = ServerOptions::builder()
        .host("0.0.0.0")
        .port(8080)
        .versioning(
            VersioningOptions::builder(VersioningStrategy::Uri)
                .default_version(" v1 ")
                .build(),
        )
        .build();
    let bootstrap = nivasa::AppBootstrapConfig::from(server.clone());

    assert_eq!(bootstrap.server, server);
    assert_eq!(
        bootstrap.versioning().map(|options| options.strategy),
        Some(VersioningStrategy::Uri)
    );
    assert_eq!(
        bootstrap
            .versioning()
            .and_then(|options| options.default_version.as_deref()),
        Some("v1")
    );
    assert_eq!(
        nivasa::AppBootstrapConfig::default().server,
        ServerOptions::default()
    );
    assert_eq!(nivasa::AppBootstrapConfig::default().versioning(), None);
}

#[test]
fn crate_root_reexports_nest_application_factory_as_data_only_shell() {
    let app = nivasa::NestApplication::create(DemoModule);

    assert_eq!(
        app.app_module().metadata(),
        ModuleMetadata::default().with_controllers(vec![TypeId::of::<DemoController>()])
    );
    assert_eq!(app.bootstrap(), &nivasa::AppBootstrapConfig::default());
}

#[test]
fn crate_root_reexports_nest_application_build_as_runtime_shell() {
    fn _assert_app_type_is_in_scope(_: Option<App<DemoModule>>) {}
    fn _assert_app_build_error_is_in_scope(_: Option<AppBuildError>) {}
    fn _assert_app_route_type_is_in_scope(_: Option<AppRoute>) {}

    let app = nivasa::NestApplication::create(DemoModule)
        .build()
        .expect("build should assemble the root module shell");

    assert_eq!(
        app.module_metadata(),
        &ModuleMetadata::default().with_controllers(vec![TypeId::of::<DemoController>()])
    );
    assert_eq!(app.bootstrap(), &nivasa::AppBootstrapConfig::default());
    assert_eq!(app.controller_registrations().len(), 1);
    assert_eq!(app.routes().len(), 1);
    assert_eq!(app.routes()[0].method, nivasa_routing::RouteMethod::Get);
    assert_eq!(app.routes()[0].path, "/health");
    assert_eq!(app.routes()[0].handler, "health");
}

#[test]
fn nest_application_reports_startup_banner_routes_and_listen_address() {
    let app = nivasa::NestApplication::create(DemoModule)
        .build()
        .expect("build should assemble the root module shell");
    let report = app.startup_report();

    assert!(report.banner.contains("Nivasa v"));
    assert!(report.banner.contains(env!("CARGO_PKG_VERSION")));
    assert!(report.root_module.contains("DemoModule"));
    assert_eq!(report.routes_registered, 1);
    assert_eq!(report.listen_address, "127.0.0.1:3000");
    assert_eq!(app.startup_lines().len(), 4);
}

#[test]
fn nest_application_can_bridge_built_routes_into_test_client() {
    let app = nivasa::NestApplication::create(DemoModule)
        .build()
        .expect("build should assemble the root module shell");

    let server = app
        .to_server(|route| match route.handler {
            "health" => {
                let route_name = route.handler;
                Some(Arc::new(move |request: &NivasaRequest| {
                    let request_id = request
                        .header("x-request-id")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("missing");

                    NivasaResponse::text(format!("ok:{request_id}"))
                        .with_header("x-app-route", route_name)
                }))
            }
            _ => None,
        })
        .expect("app routes should bridge into a server");

    let response = TestClient::new(server)
        .get("/health")
        .header("x-request-id", "bridge-1")
        .send_blocking();

    assert_eq!(response.status(), 200);
    assert_eq!(response.text(), "ok:bridge-1");
    assert_eq!(response.header("x-app-route"), Some(String::from("health")));
}

#[test]
fn nest_application_preflight_fails_before_module_configure() {
    let configure_calls = Arc::new(AtomicUsize::new(0));
    let module = PreflightModule {
        configure_calls: Arc::clone(&configure_calls),
    };

    let result = nivasa::NestApplication::create(module)
        .with_preflight(|module, _bootstrap| {
            assert_eq!(module.metadata(), ModuleMetadata::default());
            Err(AppBuildError::PreflightValidation {
                message: "missing HOST and PORT".to_string(),
            })
        })
        .build();

    match result {
        Err(error) => match error {
            AppBuildError::PreflightValidation { message } => {
                assert_eq!(message, "missing HOST and PORT");
            }
            other => panic!("unexpected error: {other}"),
        },
        Ok(_) => panic!("preflight should stop build early"),
    }

    assert_eq!(configure_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn nest_application_close_invokes_module_shutdown_hook() {
    let shutdown_calls = Arc::new(AtomicUsize::new(0));
    let module = ShutdownModule {
        shutdown_calls: Arc::clone(&shutdown_calls),
    };

    let app = nivasa::NestApplication::create(module)
        .build()
        .expect("build should assemble the root module shell");

    assert_eq!(shutdown_calls.load(Ordering::SeqCst), 0);

    app.close()
        .expect("close should run the module shutdown hook");

    assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn nest_application_sync_build_and_close_work_inside_current_thread_runtime() {
    let shutdown_calls = Arc::new(AtomicUsize::new(0));
    let module = ShutdownModule {
        shutdown_calls: Arc::clone(&shutdown_calls),
    };

    let app = nivasa::NestApplication::create(module)
        .build()
        .expect("build should complete inside a current-thread runtime");

    app.close()
        .expect("close should complete inside a current-thread runtime");

    assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);
}

#[cfg(feature = "config")]
#[test]
fn nest_application_preflight_can_validate_required_config_keys() {
    use std::collections::BTreeMap;

    use nivasa::config::{ConfigModule, ConfigSchema, ConfigValidationIssue};

    struct StartupSchema;

    impl ConfigSchema for StartupSchema {
        fn required_keys() -> &'static [&'static str] {
            &["HOST", "PORT", "API_KEY"]
        }

        fn defaults() -> &'static [(&'static str, &'static str)] {
            &[("SCHEME", "http")]
        }

        fn validate(loaded: &BTreeMap<String, String>) -> Vec<ConfigValidationIssue> {
            loaded
                .get("PORT")
                .and_then(|port| {
                    port.parse::<u16>()
                        .err()
                        .map(|_| ConfigValidationIssue::InvalidValue {
                            key: "PORT".to_string(),
                            value: port.to_string(),
                            expected: "unsigned 16-bit integer".to_string(),
                        })
                })
                .into_iter()
                .collect()
        }
    }

    let loaded = BTreeMap::from([
        ("HOST".to_string(), "127.0.0.1".to_string()),
        ("PORT".to_string(), "abc".to_string()),
    ]);
    let configure_calls = Arc::new(AtomicUsize::new(0));
    let module = PreflightModule {
        configure_calls: Arc::clone(&configure_calls),
    };

    let result = nivasa::NestApplication::create(module)
        .with_preflight(move |_module, _bootstrap| {
            ConfigModule::validate_schema::<StartupSchema>(&loaded)
                .map(|_| ())
                .map_err(|error| AppBuildError::PreflightValidation {
                    message: error.to_string(),
                })
        })
        .build();

    match result {
        Err(error) => match error {
            AppBuildError::PreflightValidation { message } => {
                assert!(message.contains("missing required config key"));
                assert!(message.contains("API_KEY"));
                assert!(message.contains("invalid config value for PORT"));
                assert!(message.contains("unsigned 16-bit integer"));
            }
            other => panic!("unexpected error: {other}"),
        },
        Ok(_) => panic!("schema validation should fail fast"),
    }

    assert_eq!(configure_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn crate_root_reexports_global_pipe_bootstrap_surface() {
    let builder =
        nivasa::AppBootstrapConfig::default().use_global_pipe(pipes_crate::TrimPipe::new());

    fn _assert_builder_is_in_scope(_: Option<NivasaServerBuilder>) {}
    let _ = builder;
}

#[test]
fn prelude_reexports_core_traits_macros_and_http_types() {
    fn _assert_request_type_is_in_scope(_: Option<NivasaRequest>) {}
    fn _assert_response_type_is_in_scope(_: Option<NivasaResponse>) {}
    fn _assert_guard_context_is_in_scope(_: Option<GuardExecutionContext>) {}
    fn _assert_guard_outcome_is_in_scope(_: Option<GuardExecutionOutcome>) {}
    fn _assert_reflector_is_in_scope(_: Option<Reflector>) {}
    fn _assert_exception_filter_trait_is_in_scope<T: ExceptionFilter<(), HttpException>>() {}
    fn _assert_exception_filter_future_is_in_scope(
        _: Option<ExceptionFilterFuture<'static, HttpException>>,
    ) {
    }
    fn _assert_arguments_host_is_in_scope(_: Option<ArgumentsHost>) {}
    fn _assert_http_arguments_host_is_in_scope(_: Option<HttpArgumentsHost>) {}
    fn _assert_ws_arguments_host_is_in_scope(_: Option<WsArgumentsHost>) {}
    fn _assert_interceptor_context_is_in_scope(_: Option<ExecutionContext>) {}
    fn _assert_interceptor_call_handler_is_in_scope(_: Option<CallHandler<NivasaResponse>>) {}
    fn _assert_interceptor_result_is_in_scope(_: Option<InterceptorResult<NivasaResponse>>) {}
    fn _assert_query_type_is_in_scope(
        _: Option<Query<std::collections::BTreeMap<String, String>>>,
    ) {
    }
    fn _assert_next_middleware_type_is_in_scope(_: Option<NextMiddleware>) {}
    fn _assert_middleware_trait_name_is_in_scope<T: Middleware>() {}
    fn _assert_pipeline_type_is_in_scope(_: Option<RequestPipeline>) {}
    fn _assert_server_builder_is_in_scope(_: Option<NivasaServerBuilder>) {}
    fn _assert_test_client_is_in_scope(_: Option<TestClient>) {}
    fn _assert_test_response_is_in_scope(_: Option<TestResponse>) {}
    fn _assert_runtime_module_type_is_in_scope(_: Option<ModuleRuntime<DemoModule>>) {}

    fn _asserts_controller_trait_name_is_in_scope<T: Controller>() {}
    fn _asserts_guard_trait_name_is_in_scope<T: Guard>() {}
    fn _asserts_middleware_trait_name_is_in_scope<T: NivasaMiddleware>() {}
    fn _asserts_module_trait_name_is_in_scope<T: Module>() {}
    fn _asserts_injectable_trait_name_is_in_scope<T: Injectable>() {}

    let _container = DependencyContainer::new();
    let _body = Body::empty();
    let _limits = MultipartLimits::new();
    let _response = NivasaResponse::builder()
        .status(HttpStatus::Ok.into())
        .build();
    let _ = NivasaServer::builder();
    let _ = UploadedFile::new("avatar.png", Some("image/png".to_string()), vec![1, 2, 3]);
    let _ = DynamicModule::new(ModuleMetadata::default());
    let _ = ProviderScope::Singleton;
    let _ = HttpStatus::Ok;
    let _ = HttpException::bad_request("boom");
    let _ = TimeoutInterceptor::<NivasaResponse>::new(std::time::Duration::from_millis(1));
    let _ = InterceptorResult::<NivasaResponse>::Ok(NivasaResponse::new(
        HttpStatus::NoContent.into(),
        Body::empty(),
    ));
    let _ = GuardExecutionOutcome::Passed;
}

#[test]
#[allow(unused_imports)]
fn crate_root_reexports_pipe_surface_as_placeholder_crate() {
    use nivasa::{
        pipes as pipes_crate, ArgumentMetadata as RootArgumentMetadata, Pipe as RootPipe,
    };

    fn _assert_pipes_namespace_is_in_scope(_: Option<pipes_crate::ArgumentMetadata>) {}
    fn _assert_pipes_namespace_pipe_is_in_scope<T: pipes_crate::Pipe>() {}
    fn _assert_root_pipe_trait_is_in_scope<T: RootPipe>() {}
    fn _assert_root_argument_metadata_is_in_scope(_: Option<RootArgumentMetadata>) {}
    fn _assert_prelude_pipe_trait_is_in_scope<T: Pipe>() {}
    fn _assert_prelude_argument_metadata_is_in_scope(_: Option<ArgumentMetadata>) {}
}

#[test]
#[allow(unused_imports)]
fn crate_root_reexports_filter_surface_as_placeholder_crate() {
    use nivasa::filters as filters_crate;
}

#[test]
#[allow(unused_imports)]
fn crate_root_reexports_dependency_crates_under_short_aliases() {
    use nivasa::{
        common as common_crate, core as core_crate, guards as guards_crate,
        interceptors as interceptors_crate, statechart as statechart_crate,
    };

    fn _assert_common_request_context_is_in_scope(_: Option<common_crate::RequestContext>) {}
    fn _assert_common_http_status_is_in_scope(_: Option<common_crate::HttpStatus>) {}
    fn _assert_core_module_registry_is_in_scope(_: Option<core_crate::ModuleRegistry>) {}
    fn _assert_guards_guard_is_in_scope<T: guards_crate::Guard>() {}
    fn _assert_interceptors_interceptor_is_in_scope<T: interceptors_crate::Interceptor>() {}
    fn _assert_statechart_engine_is_in_scope<S: statechart_crate::StatechartSpec>(
        _: Option<statechart_crate::StatechartEngine<S>>,
    ) {
    }
}

#[test]
fn crate_root_reexports_graphql_http_surface() {
    fn _assert_graphql_schema_is_in_scope(
        _: Option<GraphQLSchema<EmptyMutation, EmptyMutation, EmptySubscription>>,
    ) {
    }
    fn _assert_core_graphql_module_is_in_scope(
        _: Option<GraphQLCoreModule<EmptyMutation, EmptyMutation, EmptySubscription>>,
    ) {
    }
    fn _assert_graphql_request_is_in_scope(_: Option<GraphQLRequest>) {}
    fn _assert_graphql_response_is_in_scope(_: Option<GraphQLResponse>) {}
    fn _assert_graphql_error_is_in_scope(_: Option<GraphQLError>) {}
    fn _assert_graphql_module_is_in_scope(_: Option<GraphQLModule>) {}

    let schema = GraphQLSchema::build(EmptyMutation, EmptyMutation, EmptySubscription).finish();
    let module = GraphQLModule::from_schema(schema).title("GraphQL");

    let _ = module.endpoint_path("/graphql").playground_path("/graphql");
}

#[test]
fn crate_root_and_prelude_reexport_generated_statechart_types() {
    fn _assert_root_application_state(_: nivasa::NivasaApplicationState) {}
    fn _assert_root_application_event(_: nivasa::NivasaApplicationEvent) {}
    fn _assert_root_request_statechart(_: Option<nivasa::NivasaRequestStatechart>) {}
    fn _assert_root_guard_context(_: Option<nivasa::GuardExecutionContext>) {}
    fn _assert_root_guard_outcome(_: Option<nivasa::GuardExecutionOutcome>) {}
    fn _assert_root_reflector(_: Option<nivasa::Reflector>) {}
    fn _assert_prelude_module_state(_: NivasaModuleState) {}
    fn _assert_prelude_provider_event(_: NivasaProviderEvent) {}
    fn _assert_prelude_application_statechart(_: Option<NivasaApplicationStatechart>) {}

    let generated = nivasa::GENERATED_STATECHARTS;

    assert!(!generated.is_empty());
}

#[test]
#[allow(unused_imports)]
fn crate_root_reexports_controller_macro_and_http_surface() {
    use nivasa::{
        all, body, controller, custom_param, delete, file, files, get, head, header, headers,
        http_code, impl_controller, options, param, patch, post, put, query, req, res, session,
        Body, CallHandler, Controller, ControllerResponse, Download, ExecutionContext, Guard,
        GuardExecutionContext, GuardExecutionOutcome, Html, Interceptor, InterceptorFuture,
        InterceptorResult, Json, Middleware, MultipartLimits, NextMiddleware, NivasaMiddleware,
        NivasaRequest, NivasaResponse, NivasaServer, Reflector, RequestPipeline, Sse, Text,
        TimeoutInterceptor, UploadedFile,
    };

    fn _assert_root_interceptor_trait_name_is_in_scope<T: Interceptor>() {}
    fn _assert_root_interceptor_context_is_in_scope(_: Option<ExecutionContext>) {}
    fn _assert_root_interceptor_call_handler_is_in_scope(_: Option<CallHandler<NivasaResponse>>) {}
    fn _assert_root_interceptor_future_is_in_scope(_: Option<InterceptorFuture<NivasaResponse>>) {}
    fn _assert_root_interceptor_result_is_in_scope(_: Option<InterceptorResult<NivasaResponse>>) {}
    fn _assert_root_timeout_interceptor_is_in_scope(_: Option<TimeoutInterceptor<NivasaResponse>>) {
    }
    fn _assert_root_guard_trait_name_is_in_scope<T: Guard>() {}
    fn _assert_root_guard_context_is_in_scope(_: Option<GuardExecutionContext>) {}
    fn _assert_root_guard_outcome_is_in_scope(_: Option<GuardExecutionOutcome>) {}
    fn _assert_root_reflector_is_in_scope(_: Option<Reflector>) {}
    fn _assert_root_exception_filter_trait_is_in_scope<T: ExceptionFilter<(), HttpException>>() {}
    fn _assert_root_exception_filter_future_is_in_scope(
        _: Option<ExceptionFilterFuture<'static, HttpException>>,
    ) {
    }
    fn _assert_root_arguments_host_is_in_scope(_: Option<ArgumentsHost>) {}
    fn _assert_root_http_arguments_host_is_in_scope(_: Option<HttpArgumentsHost>) {}
    fn _assert_root_ws_arguments_host_is_in_scope(_: Option<WsArgumentsHost>) {}
    fn _assert_root_middleware_trait_name_is_in_scope<T: NivasaMiddleware>() {}
    fn _assert_root_middleware_alias_is_in_scope<T: Middleware>() {}
    fn _assert_root_middleware_layer_is_in_scope(_: Option<NivasaMiddlewareLayer<()>>) {}
    fn _assert_root_next_middleware_type_is_in_scope(_: Option<NextMiddleware>) {}

    fn _assert_root_filters_crate_is_in_scope(_: Option<filters_crate::ArgumentsHost>) {}
    fn _assert_root_filters_http_arguments_host_is_in_scope(
        _: Option<filters_crate::HttpArgumentsHost>,
    ) {
    }
    fn _assert_root_filters_ws_arguments_host_is_in_scope(
        _: Option<filters_crate::WsArgumentsHost>,
    ) {
    }
    fn _assert_root_filters_exception_filter_trait_is_in_scope<
        T: filters_crate::ExceptionFilter<(), HttpException>,
    >() {
    }
    fn _assert_root_upload_namespace_is_in_scope(_: Option<nivasa::upload::MultipartLimits>) {}
    fn _assert_prelude_upload_namespace_is_in_scope(_: Option<upload::MultipartLimits>) {}
}

struct DemoController;

impl Controller for DemoController {
    fn metadata(&self) -> nivasa_routing::ControllerMetadata {
        nivasa_routing::ControllerMetadata::new("/")
    }
}

struct DemoModule;

impl Module for DemoModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::default().with_controllers(vec![TypeId::of::<DemoController>()])
    }

    fn configure<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _container: &'life1 DependencyContainer,
    ) -> Pin<Box<dyn Future<Output = Result<(), DiError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }

    fn controller_registrations(&self) -> Vec<ModuleControllerRegistration> {
        vec![ModuleControllerRegistration::new(
            TypeId::of::<DemoController>(),
            vec![ControllerRouteRegistration::new("GET", "health", "health")],
            Vec::new(),
        )]
    }
}

#[controller("/listen")]
struct ListenController;

#[impl_controller]
impl ListenController {
    #[get("/health")]
    fn health(&self) -> NivasaResponse {
        NivasaResponse::text("listen-ready")
    }
}

struct ListenModule;

impl Module for ListenModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::default().with_controllers(vec![TypeId::of::<ListenController>()])
    }

    fn configure<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _container: &'life1 DependencyContainer,
    ) -> Pin<Box<dyn Future<Output = Result<(), DiError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }

    fn controller_registrations(&self) -> Vec<ModuleControllerRegistration> {
        vec![ModuleControllerRegistration::new(
            TypeId::of::<ListenController>(),
            ListenController::__nivasa_controller_routes()
                .into_iter()
                .map(|(method, path, handler)| {
                    ControllerRouteRegistration::new(method, path, handler)
                })
                .collect(),
            Vec::new(),
        )]
    }
}

struct PreflightModule {
    configure_calls: Arc<AtomicUsize>,
}

impl Module for PreflightModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::default()
    }

    fn configure<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _container: &'life1 DependencyContainer,
    ) -> Pin<Box<dyn Future<Output = Result<(), DiError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let configure_calls = Arc::clone(&self.configure_calls);

        Box::pin(async move {
            configure_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    fn controller_registrations(&self) -> Vec<ModuleControllerRegistration> {
        Vec::new()
    }
}

struct ShutdownModule {
    shutdown_calls: Arc<AtomicUsize>,
}

impl Module for ShutdownModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::default()
    }

    fn configure<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _container: &'life1 DependencyContainer,
    ) -> Pin<Box<dyn Future<Output = Result<(), DiError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }

    fn controller_registrations(&self) -> Vec<ModuleControllerRegistration> {
        Vec::new()
    }
}

impl OnApplicationShutdown for ShutdownModule {
    fn on_application_shutdown<'life0, 'async_trait>(
        &'life0 self,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        let shutdown_calls = Arc::clone(&self.shutdown_calls);

        Box::pin(async move {
            shutdown_calls.fetch_add(1, Ordering::SeqCst);
        })
    }
}

#[test]
fn bootstrap_config_exposes_a_normalized_global_prefix_for_route_setup() {
    let bootstrap =
        nivasa::AppBootstrapConfig::from(ServerOptions::builder().global_prefix(" api/ ").build());

    assert_eq!(bootstrap.global_prefix(), Some("/api"));
}

#[test]
fn bootstrap_config_exposes_a_listen_address_for_startup_reporting() {
    let bootstrap = nivasa::AppBootstrapConfig::from(
        ServerOptions::builder().host("0.0.0.0").port(8080).build(),
    );

    assert_eq!(bootstrap.listen_address(), "0.0.0.0:8080");
    assert_eq!(bootstrap.server.listen_address(), "0.0.0.0:8080");
}

#[test]
fn bootstrap_config_normalizes_docs_paths_and_ipv6_listen_addresses() {
    let bootstrap = nivasa::AppBootstrapConfig::new(
        ServerOptions::builder().host("::1").port(4100).build(),
    )
    .with_openapi_spec_path(" docs/openapi.json ")
    .with_swagger_ui_path(" docs/ui ");

    assert_eq!(bootstrap.listen_address(), "[::1]:4100");
    assert_eq!(bootstrap.openapi_spec_path(), "/docs/openapi.json");
    assert_eq!(bootstrap.swagger_ui_path(), "/docs/ui");

    let defaults = nivasa::AppBootstrapConfig::default()
        .with_openapi_spec_path("   ")
        .with_swagger_ui_path("   ");

    assert_eq!(defaults.openapi_spec_path(), "/api/docs/openapi.json");
    assert_eq!(defaults.swagger_ui_path(), "/api/docs");
}

#[test]
fn bootstrap_config_can_compose_prefixed_route_paths_without_runtime_wiring() {
    let bootstrap =
        nivasa::AppBootstrapConfig::from(ServerOptions::builder().global_prefix("api").build());

    assert_eq!(bootstrap.prefixed_route_path("users"), "/api/users");
    assert_eq!(bootstrap.prefixed_route_path("/"), "/api");
    assert_eq!(
        nivasa::AppBootstrapConfig::default().prefixed_route_path("users"),
        "/users"
    );
}

#[test]
fn bootstrap_config_applies_global_prefix_to_unversioned_route_registration() {
    let bootstrap =
        nivasa::AppBootstrapConfig::from(ServerOptions::builder().global_prefix(" api/ ").build());

    let builder = bootstrap
        .route(nivasa_routing::RouteMethod::Get, "health", |_| {
            NivasaResponse::text("ok")
        })
        .expect("prefixed route registration should succeed");

    assert_eq!(bootstrap.prefixed_route_path("health"), "/api/health");
    let _ = builder;
}

#[test]
fn app_to_server_reports_missing_route_handlers_by_name() {
    let app = nivasa::NestApplication::create(DemoModule)
        .build()
        .expect("build should assemble the root module shell");

    let error = match app.to_server(|_| None) {
        Ok(_) => panic!("missing handler should fail server bridge"),
        Err(error) => error,
    };

    match error {
        AppBuildError::MissingRouteHandler { handler } => {
            assert_eq!(handler, "health");
            assert_eq!(
                AppBuildError::MissingRouteHandler {
                    handler: handler.clone(),
                }
                .to_string(),
                "missing route handler `health` while building app server"
            );
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn bootstrap_config_can_forward_global_middleware_into_the_server_builder() {
    fn assert_builder(_: NivasaServerBuilder) {}

    let builder = nivasa::AppBootstrapConfig::default()
        .use_middleware(|request: NivasaRequest, next: NextMiddleware| async move {
            next.run(request).await
        })
        .route(nivasa_routing::RouteMethod::Get, "/health", |_| {
            NivasaResponse::text("ok")
        })
        .expect("route registration should succeed");

    assert_builder(builder);
}

fn free_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("must bind an ephemeral port")
        .local_addr()
        .expect("must inspect ephemeral addr")
        .port()
}

async fn wait_for_server(port: u16) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return;
        }

        sleep(Duration::from_millis(20)).await;
    }

    panic!("server did not become ready");
}

#[tokio::test]
async fn nest_application_listen_starts_http_server_from_registered_controller_handlers(
) -> Result<(), Box<dyn Error>> {
    let port = free_port();
    let app = nivasa::NestApplication::create(ListenModule);
    let server_options = ServerOptions::builder()
        .host("127.0.0.1")
        .port(port)
        .build();

    let server_task = tokio::spawn(async move { app.listen(server_options).await });
    wait_for_server(port).await;

    let client: Client<HttpConnector, Empty<Bytes>> =
        Client::builder(TokioExecutor::new()).build_http();
    let uri = format!("http://127.0.0.1:{port}/listen/health").parse()?;
    let response = client.get(uri).await?;

    assert_eq!(response.status().as_u16(), 200);
    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body, Bytes::from_static(b"listen-ready"));

    server_task.abort();
    let _ = server_task.await;

    Ok(())
}

#[test]
fn bootstrap_config_can_forward_global_interceptors_into_the_server_builder() {
    struct DemoInterceptor;

    impl Interceptor for DemoInterceptor {
        type Response = NivasaResponse;

        fn intercept(
            &self,
            _context: &ExecutionContext,
            next: CallHandler<Self::Response>,
        ) -> InterceptorFuture<Self::Response> {
            Box::pin(async move { next.handle().await })
        }
    }

    fn assert_builder(_: NivasaServerBuilder) {}

    let builder = nivasa::AppBootstrapConfig::default()
        .use_interceptor(DemoInterceptor)
        .route(nivasa_routing::RouteMethod::Get, "/health", |_| {
            NivasaResponse::text("ok")
        })
        .expect("route registration should succeed");

    assert_builder(builder);
}

#[test]
fn bootstrap_config_can_forward_global_interceptors_via_alias_into_the_server_builder() {
    struct DemoInterceptor;

    impl Interceptor for DemoInterceptor {
        type Response = NivasaResponse;

        fn intercept(
            &self,
            _context: &ExecutionContext,
            next: CallHandler<Self::Response>,
        ) -> InterceptorFuture<Self::Response> {
            Box::pin(async move { next.handle().await })
        }
    }

    fn assert_builder(_: NivasaServerBuilder) {}

    let builder = nivasa::AppBootstrapConfig::default()
        .use_global_interceptor(DemoInterceptor)
        .route(nivasa_routing::RouteMethod::Get, "/health", |_| {
            NivasaResponse::text("ok")
        })
        .expect("route registration should succeed");

    assert_builder(builder);
}

#[test]
fn bootstrap_config_can_forward_global_guards_into_the_server_builder() {
    struct DemoGuard;

    impl Guard for DemoGuard {
        fn can_activate<'a>(&'a self, context: &'a GuardExecutionContext) -> GuardFuture<'a> {
            let _request = context
                .request::<NivasaRequest>()
                .expect("guard context must include the request");

            Box::pin(async move { Ok(true) })
        }
    }

    fn assert_builder(_: NivasaServerBuilder) {}

    let builder = nivasa::AppBootstrapConfig::default()
        .use_global_guard(DemoGuard)
        .route(nivasa_routing::RouteMethod::Get, "/health", |_| {
            NivasaResponse::text("ok")
        })
        .expect("route registration should succeed");

    assert_builder(builder);
}

#[test]
fn bootstrap_config_can_forward_global_filters_into_the_server_builder() {
    struct DemoFilter;

    impl ExceptionFilter<HttpException, NivasaResponse> for DemoFilter {
        fn catch<'a>(
            &'a self,
            exception: HttpException,
            _host: HttpArgumentsHost,
        ) -> ExceptionFilterFuture<'a, NivasaResponse> {
            let _ = exception;
            Box::pin(async move { NivasaResponse::text("handled") })
        }
    }

    impl filters_crate::ExceptionFilterMetadata for DemoFilter {
        fn is_catch_all(&self) -> bool {
            true
        }
    }

    fn assert_builder(_: NivasaServerBuilder) {}

    let builder = nivasa::AppBootstrapConfig::default().use_global_filter(DemoFilter);

    assert_builder(builder);
}

#[test]
fn bootstrap_config_can_enable_versioning_without_runtime_wiring() {
    let versioning = VersioningOptions::builder(VersioningStrategy::MediaType)
        .default_version(" /v2/ ")
        .build();

    let bootstrap = nivasa::AppBootstrapConfig::default().enable_versioning(versioning.clone());

    assert_eq!(bootstrap.versioning(), Some(&versioning));
    assert_eq!(
        bootstrap
            .versioning()
            .and_then(|options| options.default_version.as_deref()),
        Some("v2")
    );
    assert_eq!(bootstrap.server.versioning, Some(versioning));
}

#[cfg(feature = "config")]
#[test]
#[allow(unused_imports)]
fn optional_crate_features_reexport_placeholder_crates_when_enabled() {
    use nivasa::config as config_crate;
    #[cfg(feature = "validation")]
    use nivasa::validation as validation_crate;
    #[cfg(feature = "websocket")]
    use nivasa::websocket as websocket_crate;
}
