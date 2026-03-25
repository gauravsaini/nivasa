use std::any::TypeId;

use nivasa_core::module::Module;
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

#[test]
fn module_macro_exposes_registration_metadata_helpers() {
    let module = AppModule;
    let metadata = module.metadata();

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
    assert_eq!(AppModule::__nivasa_module_metadata(), metadata);
    assert!(!AppModule::__nivasa_module_metadata().is_global);
}
