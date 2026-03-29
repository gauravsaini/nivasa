use nivasa_macros::{controller, impl_controller, injectable, interceptor, module};

struct ImportedModule;
struct LoggingMiddleware;
struct AuditInterceptor;

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
#[interceptor(AuditInterceptor)]
struct AppModule;

fn main() {
    let _ = AppModule::__nivasa_module_middlewares();
    let _ = AppModule::__nivasa_module_interceptors();
    let _ = AppModule::__nivasa_module_controller_registrations();
}
