struct ImportedModule;
struct BillingGuard;
struct AuditInterceptor;
struct BillingRole;
struct LoggingMiddleware;

#[nivasa_macros::injectable]
struct Service;

#[nivasa_macros::controller("/app")]
struct AppController;

#[nivasa_macros::impl_controller]
impl AppController {
    #[nivasa_macros::get("/health")]
    fn health(&self) {}
}

/// ordinary docs should be ignored by module marker parsing
#[allow(dead_code)]
/// __NIVASA_ROLES__ BillingRole
#[nivasa_macros::module({
    imports: [ImportedModule],
    controllers: [AppController],
    providers: [Service],
    exports: [Service],
    middlewares: [LoggingMiddleware],
})]
#[nivasa_macros::guard(BillingGuard)]
#[nivasa_macros::interceptor(AuditInterceptor)]
#[nivasa_macros::set_metadata(key = "scope", value = "billing")]
struct BillingModule;

fn main() {
    assert_eq!(BillingModule::__nivasa_module_imports().len(), 1);
    assert_eq!(BillingModule::__nivasa_module_controllers().len(), 1);
    assert_eq!(BillingModule::__nivasa_module_providers().len(), 1);
    assert_eq!(BillingModule::__nivasa_module_exports().len(), 1);
    assert_eq!(BillingModule::__nivasa_module_middlewares().len(), 1);
    assert_eq!(BillingModule::__nivasa_module_guards(), vec!["BillingGuard"]);
    assert_eq!(
        BillingModule::__nivasa_module_interceptors(),
        vec!["AuditInterceptor"]
    );
    assert_eq!(BillingModule::__nivasa_module_roles(), vec!["BillingRole"]);
    assert_eq!(
        BillingModule::__nivasa_module_set_metadata(),
        vec![("scope", "billing")]
    );
    assert_eq!(BillingModule::__nivasa_module_controller_registrations().len(), 1);
}
