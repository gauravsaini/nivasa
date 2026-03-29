use nivasa_macros::{controller, impl_controller};

struct RequestScopedFilter;
struct AuditFilter;

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::use_filters(RequestScopedFilter, AuditFilter)]
    #[nivasa_macros::get("/list")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_filter_metadata(),
        vec![("list", vec!["RequestScopedFilter", "AuditFilter"])],
    );
}
