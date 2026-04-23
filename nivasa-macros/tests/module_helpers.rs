use std::any::TypeId;

use nivasa_core::module::Module;
use nivasa_macros::{
    controller, guard, impl_controller, injectable, interceptor, module, set_metadata,
};

struct ImportedModule;
struct OwnerGuard;
struct AuditGuard;
struct OwnerRole;
struct AuditorRole;
struct LoggingMiddleware;
struct AuditInterceptor;
struct TraceInterceptor;
struct DocGuard;
struct DocInterceptorA;
struct DocInterceptorB;
struct BillingRole;
struct AuditRole;

#[injectable]
struct Service;

#[controller("/app")]
#[nivasa_macros::throttle(limit = 5, ttl = 30)]
struct AppController;

#[impl_controller]
impl AppController {
    #[allow(dead_code)]
    #[nivasa_macros::get("/health")]
    fn health(&self) {}

    #[allow(dead_code)]
    #[nivasa_macros::throttle(limit = 1, ttl = 10)]
    #[nivasa_macros::get("/limited")]
    fn limited(&self) {}

    #[allow(dead_code)]
    #[nivasa_macros::skip_throttle]
    #[nivasa_macros::get("/free")]
    fn free(&self) {}
}

#[module({
    imports: [ImportedModule],
    controllers: [AppController],
    providers: [Service],
    exports: [Service],
    middlewares: [LoggingMiddleware],
})]
#[guard(OwnerGuard, AuditGuard)]
#[interceptor(AuditInterceptor, TraceInterceptor)]
#[set_metadata(key = "tenant", value = "billing")]
#[set_metadata(key = "region", value = "ap-southeast-2")]
struct AppModule;

#[module({})]
struct EmptyModule;

/// __NIVASA_GUARD__ DocGuard
/// __NIVASA_INTERCEPTOR__ DocInterceptorA, DocInterceptorB
/// __NIVASA_ROLES__ BillingRole, AuditRole
/// nivasa-set-metadata: scope=billing
#[module({})]
struct DocMarkerModule;

#[test]
fn module_macro_exposes_registration_metadata_helpers() {
    let _ = (
        OwnerGuard,
        AuditGuard,
        OwnerRole,
        AuditorRole,
        AuditInterceptor,
        TraceInterceptor,
    );
    let module = AppModule;
    let _controller = AppController;
    let metadata = module.metadata();
    let controller_registrations = module.controller_registrations();

    assert_eq!(
        AppModule::__nivasa_module_imports(),
        vec![TypeId::of::<ImportedModule>()]
    );
    assert_eq!(
        AppModule::__nivasa_module_controllers(),
        vec![TypeId::of::<AppController>()]
    );
    assert_eq!(
        AppModule::__nivasa_module_providers(),
        vec![TypeId::of::<Service>()]
    );
    assert_eq!(
        AppModule::__nivasa_module_exports(),
        vec![TypeId::of::<Service>()]
    );
    assert_eq!(
        AppModule::__nivasa_module_metadata().middlewares,
        vec![TypeId::of::<LoggingMiddleware>()]
    );
    assert_eq!(
        AppModule::__nivasa_module_middlewares(),
        vec![TypeId::of::<LoggingMiddleware>()]
    );
    assert_eq!(
        AppModule::__nivasa_module_guards(),
        vec!["OwnerGuard", "AuditGuard"]
    );
    assert_eq!(
        AppModule::__nivasa_module_interceptors(),
        vec!["AuditInterceptor", "TraceInterceptor"]
    );
    assert_eq!(
        AppModule::__nivasa_module_set_metadata(),
        vec![("tenant", "billing"), ("region", "ap-southeast-2")]
    );
    assert_eq!(
        AppModule::__nivasa_module_controller_registrations(),
        controller_registrations,
    );
    assert_eq!(AppModule::__nivasa_module_metadata(), metadata);
    assert!(!AppModule::__nivasa_module_metadata().is_global);
    assert_eq!(controller_registrations.len(), 1);
    let registration = &controller_registrations[0];
    assert_eq!(registration.controller, TypeId::of::<AppController>());
    assert_eq!(registration.routes.len(), 3);
    assert_eq!(registration.routes[0].method, "GET");
    assert_eq!(registration.routes[0].path, "/app/health");
    assert_eq!(registration.routes[0].handler, "health");
    assert_eq!(
        registration.routes[0]
            .throttle
            .as_ref()
            .map(|throttle| (throttle.limit, throttle.ttl_secs,)),
        Some((5, 30))
    );
    assert!(!registration.routes[0].skip_throttle);
    assert_eq!(registration.routes[1].path, "/app/limited");
    assert_eq!(
        registration.routes[1]
            .throttle
            .as_ref()
            .map(|throttle| (throttle.limit, throttle.ttl_secs,)),
        Some((1, 10))
    );
    assert!(!registration.routes[1].skip_throttle);
    assert_eq!(registration.routes[2].path, "/app/free");
    assert!(registration.routes[2].throttle.is_none());
    assert!(registration.routes[2].skip_throttle);
    assert_eq!(
        registration.middlewares,
        vec![TypeId::of::<LoggingMiddleware>()]
    );
    let _ = _controller;
}

#[test]
fn module_macro_defaults_optional_helpers_to_empty() {
    let module = EmptyModule;

    assert!(EmptyModule::__nivasa_module_imports().is_empty());
    assert!(EmptyModule::__nivasa_module_controllers().is_empty());
    assert!(EmptyModule::__nivasa_module_providers().is_empty());
    assert!(EmptyModule::__nivasa_module_exports().is_empty());
    assert!(EmptyModule::__nivasa_module_middlewares().is_empty());
    assert!(EmptyModule::__nivasa_module_guards().is_empty());
    assert!(EmptyModule::__nivasa_module_interceptors().is_empty());
    assert!(EmptyModule::__nivasa_module_roles().is_empty());
    assert!(EmptyModule::__nivasa_module_set_metadata().is_empty());
    assert!(EmptyModule::__nivasa_module_controller_registrations().is_empty());
    assert!(module.metadata().imports.is_empty());
    assert!(module.controller_registrations().is_empty());
}

#[test]
fn module_macro_parses_doc_marker_helpers() {
    let _ = (DocGuard, DocInterceptorA, DocInterceptorB, BillingRole, AuditRole);

    assert_eq!(DocMarkerModule::__nivasa_module_guards(), vec!["DocGuard"]);
    assert_eq!(
        DocMarkerModule::__nivasa_module_interceptors(),
        vec!["DocInterceptorA", "DocInterceptorB"]
    );
    assert_eq!(
        DocMarkerModule::__nivasa_module_roles(),
        vec!["BillingRole", "AuditRole"]
    );
    assert_eq!(
        DocMarkerModule::__nivasa_module_set_metadata(),
        vec![("scope", "billing")]
    );
}
