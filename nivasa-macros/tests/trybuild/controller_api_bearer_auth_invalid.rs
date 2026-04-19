use nivasa_macros::{controller, impl_controller};

#[controller("/auth")]
struct AuthController;

#[impl_controller]
impl AuthController {
    #[nivasa_macros::get("/session")]
    #[nivasa_macros::api_bearer_auth(true)]
    fn session(&self) {}
}

fn main() {}
