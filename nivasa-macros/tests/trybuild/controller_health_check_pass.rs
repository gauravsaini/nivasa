use nivasa_macros::{controller, impl_controller};

#[controller("/health")]
struct HealthController;

#[impl_controller]
impl HealthController {
    #[nivasa_macros::get("/")]
    #[nivasa_macros::health_check]
    fn health(&self) {}
}

fn main() {
    assert_eq!(
        HealthController::__nivasa_controller_health_check_metadata(),
        vec!["health"],
    );
}
