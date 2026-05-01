use nivasa_macros::{controller, impl_controller};

#[controller("/")]
struct RootController;

#[impl_controller]
impl RootController {
    #[nivasa_macros::get("/")]
    fn root(&self) {}

    #[nivasa_macros::get("/health")]
    fn health(&self) {}

    #[nivasa_macros::get("metrics")]
    fn metrics(&self) {}
}

fn main() {
    assert_eq!(
        RootController::__nivasa_controller_routes(),
        vec![
            ("GET", "/".to_string(), "root"),
            ("GET", "/health".to_string(), "health"),
            ("GET", "/metrics".to_string(), "metrics"),
        ],
    );
}
