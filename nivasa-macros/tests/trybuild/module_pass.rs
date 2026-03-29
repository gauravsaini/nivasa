use nivasa_macros::{controller, impl_controller, injectable, module};

struct ImportedModule;
struct LoggingMiddleware;

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
struct AppModule;

fn main() {
    let _ = AppModule::__nivasa_module_middlewares();
    let _ = AppModule::__nivasa_module_controller_registrations();
}
