use nivasa::prelude::*;

struct JwtGuard;

#[controller("/auth")]
#[guard(JwtGuard)]
pub struct AuthController;

#[impl_controller]
impl AuthController {
    #[get("/profile")]
    pub fn profile(&self) -> &'static str {
        "protected profile"
    }
}

#[module({
    controllers: [AuthController],
})]
pub struct AppModule;
