use nivasa_macros::{injectable, module};

struct ImportedModule;
struct AppController;
struct LoggingMiddleware;

#[injectable]
struct Service;

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
}
