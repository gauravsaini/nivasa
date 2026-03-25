use nivasa_macros::{injectable, module};

struct ImportedModule;
struct AppController;

#[injectable]
struct Service;

#[module({
    imports: [ImportedModule],
    controllers: [AppController],
    providers: [Service],
    middlwares: [AppController],
})]
struct AppModule;

fn main() {}
