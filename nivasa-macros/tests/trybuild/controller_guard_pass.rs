use nivasa_macros::{controller, impl_controller};

struct ControllerGuard;
struct AuthGuard;
struct AuditGuard;

#[nivasa_macros::guard(ControllerGuard)]
#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::guard(AuthGuard, AuditGuard)]
    #[nivasa_macros::get("/list")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_guards(),
        vec!["ControllerGuard"],
    );
    assert_eq!(
        UsersController::__nivasa_controller_guard_metadata(),
        vec![("list", vec!["AuthGuard", "AuditGuard"])],
    );
}
