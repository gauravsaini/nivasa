#[allow(unused_imports)]
use nivasa::filters as filters_crate;
#[allow(unused_imports)]
use nivasa::pipes as pipes_crate;
use nivasa::prelude::*;
#[allow(unused_imports)]
use nivasa::prelude::{
    all, body, controller, custom_param, delete, file, files, get, head, header, headers,
    http_code, impl_controller, injectable, ip, module, options, param, patch, post, put, query,
    req, res, scxml_handler, session, ArgumentMetadata, ArgumentsHost, ExceptionFilter,
    ExceptionFilterFuture, HttpArgumentsHost, Middleware, NivasaMiddlewareLayer, Pipe, Reflector,
};
use std::future::Future;
use std::pin::Pin;

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
    use nivasa::{pipes as pipes_crate, ArgumentMetadata as RootArgumentMetadata, Pipe as RootPipe};

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
    fn _assert_root_middleware_trait_name_is_in_scope<T: NivasaMiddleware>() {}
    fn _assert_root_middleware_alias_is_in_scope<T: Middleware>() {}
    fn _assert_root_middleware_layer_is_in_scope(_: Option<NivasaMiddlewareLayer<()>>) {}
    fn _assert_root_next_middleware_type_is_in_scope(_: Option<NextMiddleware>) {}

    fn _assert_root_filters_crate_is_in_scope(_: Option<filters_crate::ArgumentsHost>) {}
    fn _assert_root_filters_http_arguments_host_is_in_scope(
        _: Option<filters_crate::HttpArgumentsHost>,
    ) {
    }
    fn _assert_root_filters_exception_filter_trait_is_in_scope<
        T: filters_crate::ExceptionFilter<(), HttpException>,
    >() {
    }
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
}

#[test]
fn bootstrap_config_exposes_a_normalized_global_prefix_for_route_setup() {
    let bootstrap =
        nivasa::AppBootstrapConfig::from(ServerOptions::builder().global_prefix(" api/ ").build());

    assert_eq!(bootstrap.global_prefix(), Some("/api"));
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
        bootstrap.versioning().and_then(|options| options.default_version.as_deref()),
        Some("v2")
    );
    assert_eq!(bootstrap.server.versioning, Some(versioning));
}

#[cfg(feature = "config")]
#[test]
#[allow(unused_imports)]
fn optional_crate_features_reexport_placeholder_crates_when_enabled() {
    use nivasa::config as config_crate;
    use nivasa::validation as validation_crate;
    use nivasa::websocket as websocket_crate;
}
