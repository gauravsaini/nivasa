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
        AppBootstrapConfig, ServerOptions, ServerOptionsBuilder, VersioningOptions,
        VersioningOptionsBuilder, VersioningStrategy,
    };
    pub use nivasa_common::{HttpException, HttpStatus};
    pub use nivasa_http::{
        upload, Body, ControllerResponse, FromRequest, Html, IntoResponse, Json, NivasaRequest,
        NivasaResponse, Query, Redirect,
    };
    pub use nivasa_core::di::Lazy;
    pub use nivasa_core::di::provider::Injectable;
    pub use nivasa_core::{
        DependencyContainer, DiError, Module, ModuleMetadata, OnApplicationBootstrap,
        OnApplicationShutdown, OnModuleDestroy, OnModuleInit, Provider, ProviderScope,
    };
    pub use nivasa_macros::{
        all, body, controller, custom_param, delete, file, files, get, head, header, headers,
        http_code, impl_controller, injectable, ip, module, options, param, patch, post, put,
        query, req, res, scxml_handler, session,
    };
    pub use nivasa_statechart::{StatechartEngine, StatechartSpec};
    #[cfg(feature = "config")]
    pub use nivasa_config as config;
    #[cfg(feature = "validation")]
    pub use nivasa_validation as validation;
    #[cfg(feature = "websocket")]
    pub use nivasa_websocket as websocket;
}

pub use application::{
    AppBootstrapConfig, ServerOptions, ServerOptionsBuilder, VersioningOptions,
    VersioningOptionsBuilder, VersioningStrategy,
};
pub use nivasa_common::{self, HttpException, HttpStatus};
#[cfg(feature = "config")]
pub use nivasa_config as config;
pub use nivasa_http::{
    self, upload, Body, ControllerResponse, FromRequest, Html, IntoResponse, Json, NivasaRequest,
    NivasaResponse, Query, Redirect,
};
pub use nivasa_core::di::Lazy;
pub use nivasa_core::di::provider::Injectable;
pub use nivasa_core::{
    self, DependencyContainer, DiError, Module, ModuleEntry, ModuleMetadata, ModuleRegistry,
    ModuleRegistryError, OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy,
    OnModuleInit, Provider, ProviderScope,
};
pub use nivasa_macros::{
    self, all, body, controller, custom_param, delete, file, files, get, head, header, headers,
    http_code, impl_controller, injectable, ip, module, options, param, patch, post, put, query,
    req, res, scxml_handler, session,
};
pub use nivasa_statechart::{self, StatechartEngine, StatechartSpec};
#[cfg(feature = "validation")]
pub use nivasa_validation as validation;
#[cfg(feature = "websocket")]
pub use nivasa_websocket as websocket;
