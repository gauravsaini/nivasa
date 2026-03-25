use http::Method;
use nivasa_http::{Body, FromRequest, Json, NivasaRequest, Query};
use nivasa_macros::{controller, impl_controller};
use nivasa_routing::{RouteDispatchRegistry, RouteMethod};
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct CreateUser {
    name: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct UserSearch {
    page: u32,
    active: bool,
}

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(&self) {}
}

#[test]
fn post_route_registration_supports_json_and_query_extraction() {
    let mut routes = RouteDispatchRegistry::new();
    for (method, path, handler) in UsersController::__nivasa_controller_routes() {
        routes
            .register_static(RouteMethod::from(method), path, handler)
            .expect("controller route must register");
    }

    let controller = UsersController;
    controller.create();

    let request = NivasaRequest::new(
        Method::POST,
        "/users/create?page=2&active=true",
        Body::json(serde_json::json!({ "name": "Ada" })),
    );

    assert!(routes.resolve_match("POST", request.path()).is_some());
    assert_eq!(request.query("page"), Some("2"));
    assert_eq!(request.query("active"), Some("true"));

    let body = Json::<CreateUser>::from_request(&request).unwrap();
    assert_eq!(
        body.into_inner(),
        CreateUser {
            name: "Ada".to_string(),
        }
    );

    let query = Query::<UserSearch>::from_request(&request).unwrap();
    assert_eq!(
        query.into_inner(),
        UserSearch {
            page: 2,
            active: true,
        }
    );

    let body_json = request.extract::<Json<CreateUser>>().unwrap();
    assert_eq!(
        body_json.into_inner(),
        CreateUser {
            name: "Ada".to_string(),
        }
    );
}
