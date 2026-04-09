//! # Nivasa
//!
//! A modular, SCXML-driven Rust web framework with NestJS pattern compliance.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use nivasa::prelude::*;
//! ```
//!
//! ## Architecture
//!
//! Every lifecycle in Nivasa is modeled as a W3C SCXML statechart.
//! State transitions are code-generated from `.scxml` files and enforced
//! at compile time and runtime.

pub mod application;

/// The prelude — import everything you need with `use nivasa::prelude::*`.
pub mod prelude {
    pub use crate::application::{
        App, AppBootstrapConfig, AppBuildError, AppRoute, NestApplication, ServerOptions,
        ServerOptionsBuilder, VersioningOptions, VersioningOptionsBuilder, VersioningStrategy,
    };
    pub use nivasa_common::{HttpException, HttpStatus};
    #[cfg(feature = "config")]
    pub use nivasa_config as config;
    pub use nivasa_core::di::provider::Injectable;
    pub use nivasa_core::di::{
        FactoryProvider, Lazy, ProviderMetadata, ProviderRegistry, ValueProvider,
    };
    pub use nivasa_core::module::{
        ConfigurableModule, ControllerRouteRegistration, DynamicModule,
        ModuleControllerRegistration, ModuleHookSet, ModuleLifecycleError, ModuleOrchestrator,
        ModuleOrchestratorError, ModuleRuntime,
    };
    pub use nivasa_core::{
        DependencyContainer, DiError, Module, ModuleEntry, ModuleMetadata, ModuleRegistry,
        ModuleRegistryError, OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy,
        OnModuleInit, Provider, ProviderScope, Reflector,
    };
    pub use nivasa_filters as filters;
    pub use nivasa_filters::{
        ArgumentsHost, ExceptionFilter, ExceptionFilterFuture, HttpArgumentsHost,
        WsArgumentsHost,
    };
    pub use nivasa_guards::{ExecutionContext as GuardExecutionContext, Guard, GuardFuture};
    pub use nivasa_http::upload::MultipartLimits;
    pub use nivasa_http::HttpExceptionFilter;
    pub use nivasa_http::NivasaMiddleware as Middleware;
    pub use nivasa_http::NivasaMiddlewareLayer;
    pub use nivasa_http::TowerServiceMiddleware;
    pub use nivasa_http::{
        upload, Body, ControllerResponse, Download, FromRequest, GuardExecutionOutcome, HeaderMap,
        Html, IntoResponse, Json, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
        NivasaServer, NivasaServerBuilder, Query, Redirect, RequestExtractError, RequestPipeline,
        Sse, SseEvent, StreamBody, Text, UploadedFile,
    };
    pub use nivasa_interceptors::{
        CallHandler, ClassSerializerInterceptor, ExecutionContext, Interceptor, InterceptorFuture,
        InterceptorResult, TimeoutInterceptor,
    };
    pub use nivasa_macros::{
        all, body, controller, custom_param, delete, file, files, get, head, header, headers,
        http_code, impl_controller, injectable, ip, module, options, param, patch, post, put,
        query, req, res, scxml_handler, session, use_filters,
    };
    pub use nivasa_pipes::{ArgumentMetadata, Pipe};
    pub use nivasa_pipes as pipes;
    pub use nivasa_routing::Controller;
    pub use nivasa_statechart::{
        NivasaApplicationEvent, NivasaApplicationState, NivasaApplicationStatechart,
        NivasaModuleEvent, NivasaModuleState, NivasaModuleStatechart, NivasaProviderEvent,
        NivasaProviderState, NivasaProviderStatechart, NivasaRequestEvent, NivasaRequestState,
        NivasaRequestStatechart, StatechartEngine, StatechartSpec, GENERATED_STATECHARTS,
    };
    #[cfg(feature = "validation")]
    pub use nivasa_validation as validation;
    #[cfg(feature = "websocket")]
    pub use nivasa_websocket as websocket;
}

pub use application::{
    App, AppBootstrapConfig, AppBuildError, AppRoute, NestApplication, ServerOptions,
    ServerOptionsBuilder, VersioningOptions, VersioningOptionsBuilder, VersioningStrategy,
};
pub use nivasa_common::{self, HttpException, HttpStatus};
#[cfg(feature = "config")]
pub use nivasa_config as config;
pub use nivasa_core::di::provider::Injectable;
pub use nivasa_core::di::{
    FactoryProvider, Lazy, ProviderMetadata, ProviderRegistry, ValueProvider,
};
pub use nivasa_core::module::{
    ConfigurableModule, ControllerRouteRegistration, DynamicModule, ModuleControllerRegistration,
    ModuleHookSet, ModuleLifecycleError, ModuleOrchestrator, ModuleOrchestratorError,
    ModuleRuntime,
};
pub use nivasa_core::{
    self, DependencyContainer, DiError, Module, ModuleEntry, ModuleMetadata, ModuleRegistry,
    ModuleRegistryError, OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy,
    OnModuleInit, Provider, ProviderScope, Reflector,
};
pub use nivasa_filters as filters;
pub use nivasa_filters::{
    ArgumentsHost, ExceptionFilter, ExceptionFilterFuture, HttpArgumentsHost, WsArgumentsHost,
};
pub use nivasa_guards::{self, ExecutionContext as GuardExecutionContext, Guard, GuardFuture};
pub use nivasa_http::upload::MultipartLimits;
pub use nivasa_http::HttpExceptionFilter;
pub use nivasa_http::NivasaMiddleware as Middleware;
pub use nivasa_http::NivasaMiddlewareLayer;
pub use nivasa_http::TowerServiceMiddleware;
pub use nivasa_http::{
    self, upload, Body, ControllerResponse, Download, FromRequest, GuardExecutionOutcome,
    HeaderMap, Html, IntoResponse, Json, NextMiddleware, NivasaMiddleware, NivasaRequest,
    NivasaResponse, NivasaServer, NivasaServerBuilder, Query, Redirect, RequestExtractError,
    RequestPipeline, Sse, SseEvent, StreamBody, Text, UploadedFile,
};
pub use nivasa_interceptors::{
    self, CallHandler, ClassSerializerInterceptor, ExecutionContext, Interceptor,
    InterceptorFuture, InterceptorResult, TimeoutInterceptor,
};
pub use nivasa_macros::{
    self, all, body, controller, custom_param, delete, file, files, get, head, header, headers,
    http_code, impl_controller, injectable, ip, module, options, param, patch, post, put, query,
    req, res, scxml_handler, session, use_filters,
};
pub use nivasa_pipes::{self, ArgumentMetadata, Pipe};
pub use nivasa_pipes as pipes;
pub use nivasa_routing::Controller;
pub use nivasa_statechart::{
    self, NivasaApplicationEvent, NivasaApplicationState, NivasaApplicationStatechart,
    NivasaModuleEvent, NivasaModuleState, NivasaModuleStatechart, NivasaProviderEvent,
    NivasaProviderState, NivasaProviderStatechart, NivasaRequestEvent, NivasaRequestState,
    NivasaRequestStatechart, StatechartEngine, StatechartSpec, GENERATED_STATECHARTS,
};
#[cfg(feature = "validation")]
pub use nivasa_validation as validation;
#[cfg(feature = "websocket")]
pub use nivasa_websocket as websocket;
