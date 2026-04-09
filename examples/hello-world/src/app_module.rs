use nivasa::prelude::*;

#[controller("/hello")]
pub struct HelloController;

#[impl_controller]
impl HelloController {
    #[get("/")]
    pub fn greet(&self) -> &'static str {
        "hello world"
    }
}

#[module({
    controllers: [HelloController],
})]
pub struct AppModule;
