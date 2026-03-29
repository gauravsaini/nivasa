use std::any::TypeId;

use nivasa_core::module::Module;
use nivasa_macros::{controller, guard, impl_controller, injectable, interceptor, module};

struct ImportedModule;
struct OwnerGuard;
struct AuditGuard;
struct LoggingMiddleware;
struct AuditInterceptor;
struct TraceInterceptor;

#[injectable]
struct Service;

#[controller("/app")]
struct AppController;

#[impl_controller]
impl AppController {
    #[nivasa_macros::get("/health")]
    fn health(&self) {}
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
struct AppModule;

#[test]
fn module_macro_exposes_registration_metadata_helpers() {
    let module = AppModule;
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
        AppModule::__nivasa_module_controller_registrations(),
        controller_registrations,
    );
    assert_eq!(AppModule::__nivasa_module_metadata(), metadata);
    assert!(!AppModule::__nivasa_module_metadata().is_global);
    assert_eq!(controller_registrations.len(), 1);
    assert_eq!(controller_registrations[0].controller, TypeId::of::<AppController>());
    assert_eq!(controller_registrations[0].routes.len(), 1);
    assert_eq!(controller_registrations[0].routes[0].method, "GET");
    assert_eq!(controller_registrations[0].routes[0].path, "/app/health");
    assert_eq!(controller_registrations[0].routes[0].handler, "health");
}
