use nivasa::openapi::{
    build_openapi_document, OpenApiControllerMetadata, OpenApiControllerMetadataProvider,
};

struct ManualUsersController;

impl OpenApiControllerMetadataProvider for ManualUsersController {
    fn routes() -> Vec<(&'static str, String, &'static str)> {
        vec![
            ("GET", "/users/:id".to_string(), "show"),
            ("POST", "/users".to_string(), "create"),
        ]
    }

    fn api_tags() -> Vec<&'static str> {
        vec!["Users"]
    }

    fn api_operation_metadata() -> Vec<(&'static str, Option<&'static str>)> {
        vec![("show", Some("Get a user")), ("create", Some("Create a user"))]
    }

    fn api_param_metadata() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
        vec![("show", vec![("id", "User ID")]), ("create", vec![])]
    }

    fn api_response_metadata() -> Vec<(&'static str, Vec<(u16, &'static str, &'static str)>)> {
        vec![
            ("show", vec![(200, "UserDto", "Success")]),
            ("create", vec![(201, "UserDto", "Created")]),
        ]
    }

    fn api_body_metadata() -> Vec<(&'static str, &'static str)> {
        vec![("create", "CreateUserDto")]
    }

    fn api_bearer_auth_metadata() -> Vec<&'static str> {
        vec!["show"]
    }
}

#[test]
fn openapi_builder_collects_controller_route_and_api_metadata() {
    let document = build_openapi_document(
        "Users API",
        "1.0.0",
        [OpenApiControllerMetadata::from_provider::<ManualUsersController>()],
    );

    assert_eq!(document.openapi, "3.0.0");
    assert_eq!(document.info.title, "Users API");
    assert_eq!(document.info.version, "1.0.0");

    let show = &document.paths["/users/{id}"]["get"];
    assert_eq!(show.tags, vec!["Users".to_string()]);
    assert_eq!(show.summary.as_deref(), Some("Get a user"));
    assert_eq!(show.parameters.len(), 1);
    assert_eq!(show.parameters[0].name, "id");
    assert_eq!(show.parameters[0].location, "path");
    assert_eq!(show.parameters[0].description, "User ID");
    assert_eq!(show.responses["200"].description, "Success");
    assert_eq!(
        show.responses["200"].content["application/json"].schema_ref,
        "#/components/schemas/UserDto"
    );
    assert_eq!(
        show.security,
        vec![std::collections::BTreeMap::from([(
            "bearerAuth".to_string(),
            Vec::new(),
        )])]
    );

    let create = &document.paths["/users"]["post"];
    assert_eq!(create.summary.as_deref(), Some("Create a user"));
    assert!(create.parameters.is_empty());
    assert_eq!(
        create
            .request_body
            .as_ref()
            .expect("create route should expose a request body")
            .content["application/json"]
            .schema_ref,
        "#/components/schemas/CreateUserDto"
    );
    assert_eq!(create.responses["201"].description, "Created");

    assert!(document.components.schemas.contains_key("CreateUserDto"));
    assert!(document.components.schemas.contains_key("UserDto"));
    assert_eq!(
        document.components.security_schemes["bearerAuth"].scheme,
        "bearer"
    );
}

#[test]
fn openapi_spec_includes_all_routes_with_correct_methods() {
    let document = build_openapi_document(
        "Users API",
        "1.0.0",
        [OpenApiControllerMetadata::from_provider::<ManualUsersController>()],
    );

    let user_by_id = document
        .paths
        .get("/users/{id}")
        .expect("spec must include the GET /users/{id} path");
    assert_eq!(
        user_by_id.keys().cloned().collect::<Vec<_>>(),
        vec!["get".to_string()]
    );

    let users = document
        .paths
        .get("/users")
        .expect("spec must include the POST /users path");
    assert_eq!(
        users.keys().cloned().collect::<Vec<_>>(),
        vec!["post".to_string()]
    );
}

#[test]
fn openapi_spec_includes_request_and_response_schemas() {
    let document = build_openapi_document(
        "Users API",
        "1.0.0",
        [OpenApiControllerMetadata::from_provider::<ManualUsersController>()],
    );

    let create = &document.paths["/users"]["post"];
    let request_body = create
        .request_body
        .as_ref()
        .expect("create route must expose request body schema");
    assert_eq!(
        request_body.content["application/json"].schema_ref,
        "#/components/schemas/CreateUserDto"
    );

    let show = &document.paths["/users/{id}"]["get"];
    assert_eq!(
        show.responses["200"].content["application/json"].schema_ref,
        "#/components/schemas/UserDto"
    );
    assert_eq!(
        create.responses["201"].content["application/json"].schema_ref,
        "#/components/schemas/UserDto"
    );
}
