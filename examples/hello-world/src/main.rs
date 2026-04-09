mod app_module;

use app_module::AppModule;
use nivasa::prelude::*;

fn main() {
    let app = NestApplication::create(AppModule)
        .build()
        .expect("hello-world example should build");

    println!("routes: {:?}", app.routes());
}
