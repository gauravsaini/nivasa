mod app_module;

use app_module::AppModule;
use nivasa::prelude::*;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let app = NestApplication::create(AppModule)
        .build()
        .expect("auth-jwt example should build");

    let server = app
        .to_server(|route| app_module::resolve_route_handler(route.handler))
        .expect("auth-jwt example should resolve the known route handlers");

    let report = app.startup_report();
    for line in report.lines() {
        println!("{line}");
    }

    println!("routes:");
    for route in app.routes() {
        println!(
            "  {} {} -> {}",
            route.method.as_str(),
            route.path,
            route.handler
        );
    }

    println!("auth flow:");
    println!("  POST /auth/login -> {}", app_module::SESSION_TOKEN);
    println!("  GET /auth/profile -> Authorization: {}", app_module::SESSION_TOKEN);
    println!("  profile handler is wired through the same route surface as the test-backed flow");

    let _server = server;
    Ok(())
}
