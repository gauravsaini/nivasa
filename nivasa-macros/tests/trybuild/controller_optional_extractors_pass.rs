use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(
        #[nivasa_macros::body] raw_body: String,
        #[nivasa_macros::body("payload")] named_body: String,
        #[nivasa_macros::headers] all_headers: String,
        #[nivasa_macros::headers("selected")] named_headers: String,
        #[nivasa_macros::req] request: String,
        #[nivasa_macros::req("request")] named_request: String,
        #[nivasa_macros::res] response: String,
        #[nivasa_macros::res("response")] named_response: String,
    ) {
        let _ = (
            raw_body,
            named_body,
            all_headers,
            named_headers,
            request,
            named_request,
            response,
            named_response,
        );
    }
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_parameter_metadata(),
        vec![(
            "create",
            vec![
                ("body", None),
                ("body", Some("payload")),
                ("headers", None),
                ("headers", Some("selected")),
                ("req", None),
                ("req", Some("request")),
                ("res", None),
                ("res", Some("response")),
            ],
        )],
    );
}
