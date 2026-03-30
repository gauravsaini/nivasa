use nivasa_macros::{controller, impl_controller, use_filters};

struct RequestScopedFilter;
struct AuditFilter;

#[use_filters(RequestScopedFilter, AuditFilter)]
#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/list")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_filters(),
        vec!["RequestScopedFilter", "AuditFilter"],
    );
}
