use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(
        #[nivasa_macros::body("body")] body: String,
        #[nivasa_macros::param("id")] id: String,
        #[nivasa_macros::query("query")] query: String,
        #[nivasa_macros::query("search")] search: String,
        #[nivasa_macros::headers("headers")] headers: String,
        #[nivasa_macros::header("x-request-id")] request_id: String,
        #[nivasa_macros::req("request")] request: String,
        #[nivasa_macros::res("response")] response: String,
        #[nivasa_macros::custom_param(MyExtractor)] extractor: String,
    ) {
        let _ = (
            body,
            id,
            query,
            search,
            headers,
            request_id,
            request,
            response,
            extractor,
        );
    }
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_parameter_metadata(),
        vec![(
            "create",
            vec![
                ("body", Some("body")),
                ("param", Some("id")),
                ("query", Some("query")),
                ("query", Some("search")),
                ("headers", Some("headers")),
                ("header", Some("x-request-id")),
                ("req", Some("request")),
                ("res", Some("response")),
                ("custom_param", Some("MyExtractor")),
            ],
        )],
    );
}
