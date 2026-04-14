use nivasa_macros::{controller, impl_controller};

#[controller("/throttle")]
#[throttle(limit = 5, ttl = 30)]
struct ThrottledController;

#[allow(dead_code)]
#[impl_controller]
impl ThrottledController {
    #[nivasa_macros::get("/default")]
    fn default(&self) {}

    #[nivasa_macros::throttle(limit = 1, ttl = 10)]
    #[nivasa_macros::get("/override")]
    fn override_route(&self) {}

    #[nivasa_macros::skip_throttle]
    #[nivasa_macros::get("/free")]
    fn free(&self) {}
}

#[test]
fn controller_throttle_metadata_tracks_default_override_and_skip_routes() {
    assert_eq!(
        ThrottledController::__nivasa_controller_throttle_metadata(),
        vec![
            ("default", Some((5, 30)), false),
            ("override_route", Some((1, 10)), false),
            ("free", None, true),
        ]
    );
}
