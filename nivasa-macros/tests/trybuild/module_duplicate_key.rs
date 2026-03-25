use nivasa_macros::{injectable, module};

struct ImportedModule;

#[injectable]
struct Service;

#[module({
    imports: [ImportedModule],
    providers: [Service],
    providers: [Service],
})]
struct AppModule;

fn main() {}
