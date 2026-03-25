use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/list")]
    fn list(&self) {}

    #[nivasa_macros::post("create")]
    fn create(&self) {}

    #[nivasa_macros::put("/profile")]
    fn update_profile(&self) {}

    #[nivasa_macros::delete("/archive")]
    fn archive(&self) {}

    #[nivasa_macros::patch("/rename")]
    fn rename(&self) {}

    #[nivasa_macros::head("/health")]
    fn health(&self) {}

    #[nivasa_macros::options("/capabilities")]
    fn capabilities(&self) {}

    #[nivasa_macros::all("/catch-all")]
    fn catch_all(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_routes(),
        vec![
            ("GET", "/users/list".to_string(), "list"),
            ("POST", "/users/create".to_string(), "create"),
            ("PUT", "/users/profile".to_string(), "update_profile"),
            ("DELETE", "/users/archive".to_string(), "archive"),
            ("PATCH", "/users/rename".to_string(), "rename"),
            ("HEAD", "/users/health".to_string(), "health"),
            ("OPTIONS", "/users/capabilities".to_string(), "capabilities"),
            ("ALL", "/users/catch-all".to_string(), "catch_all"),
        ],
    );
}
