use nivasa_http::NivasaRequest;
use nivasa_macros::{controller, impl_controller};

#[controller("/events")]
struct EventController;

#[impl_controller]
impl EventController {
    #[nivasa_macros::get("/")]
    fn list(&self) -> &'static str {
        "listed"
    }

    #[nivasa_macros::get("/request-aware")]
    fn request_aware(&self, request: &NivasaRequest) -> String {
        request.path().to_string()
    }

    #[nivasa_macros::get("/unsupported")]
    fn unsupported(&self, id: u64) -> u64 {
        id
    }
}

fn main() {
    assert_eq!(
        EventController::__nivasa_controller_routes(),
        vec![
            ("GET", "/events".to_string(), "list"),
            ("GET", "/events/request-aware".to_string(), "request_aware"),
            ("GET", "/events/unsupported".to_string(), "unsupported"),
        ],
    );

    assert!(nivasa_http::resolve_controller_route_handler("/events", "list").is_some());
    assert!(
        nivasa_http::resolve_controller_route_handler("/events/request-aware", "request_aware")
            .is_some()
    );
    assert!(
        nivasa_http::resolve_controller_route_handler("/events/unsupported", "unsupported")
            .is_none()
    );
}
