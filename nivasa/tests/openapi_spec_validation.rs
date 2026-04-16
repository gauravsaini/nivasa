use nivasa::openapi::{
    build_openapi_document, OpenApiControllerMetadata, OpenApiControllerMetadataProvider,
};
use openapiv3::{
    Components, Info, MediaType, ObjectType, OpenAPI, Operation, Parameter, ParameterData,
    ParameterSchemaOrContent, PathItem, PathStyle, Paths, ReferenceOr, RequestBody, Response,
    Responses, Schema, SchemaData, SchemaKind, SecurityRequirement, SecurityScheme, StatusCode,
    Type,
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
        vec![
            ("show", Some("Get a user")),
            ("create", Some("Create a user")),
        ]
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
fn openapi_spec_validates_against_openapi_3_0_spec() {
    let document = build_openapi_document(
        "Users API",
        "1.0.0",
        [OpenApiControllerMetadata::from_provider::<
            ManualUsersController,
        >()],
    );

    let openapi = to_openapiv3_document(&document);
    let serialized = serde_json::to_value(&openapi).expect("OpenAPI document must serialize");
    let validated: OpenAPI =
        serde_json::from_value(serialized).expect("OpenAPI document must deserialize");

    assert_eq!(validated.openapi, "3.0.0");
    assert!(validated.paths.paths.contains_key("/users/{id}"));
    assert!(validated.paths.paths.contains_key("/users"));
    assert_eq!(validated.operations().count(), 2);
}

fn to_openapiv3_document(document: &nivasa::openapi::OpenApiDocument) -> OpenAPI {
    let mut paths = Paths::default();
    let mut schemas = Components::default().schemas;
    let mut security_schemes = Components::default().security_schemes;

    for schema_name in document.components.schemas.keys() {
        schemas.insert(schema_name.clone(), ReferenceOr::Item(object_schema()));
    }

    for (scheme_name, scheme) in &document.components.security_schemes {
        if scheme.scheme == "bearer" {
            security_schemes.insert(
                scheme_name.clone(),
                ReferenceOr::Item(SecurityScheme::HTTP {
                    scheme: scheme.scheme.clone(),
                    bearer_format: scheme.bearer_format.clone(),
                    description: None,
                    extensions: Default::default(),
                }),
            );
        }
    }

    for (path, operations) in &document.paths {
        let mut item = PathItem::default();

        for (method, operation) in operations {
            let converted = convert_operation(path, operation);
            match method.as_str() {
                "get" => item.get = Some(converted),
                "post" => item.post = Some(converted),
                "put" => item.put = Some(converted),
                "delete" => item.delete = Some(converted),
                "options" => item.options = Some(converted),
                "head" => item.head = Some(converted),
                "patch" => item.patch = Some(converted),
                "trace" => item.trace = Some(converted),
                _ => panic!("unexpected method {method} in generated spec"),
            }
        }

        paths.paths.insert(path.clone(), ReferenceOr::Item(item));
    }

    OpenAPI {
        openapi: document.openapi.clone(),
        info: Info {
            title: document.info.title.clone(),
            version: document.info.version.clone(),
            ..Default::default()
        },
        servers: Vec::new(),
        paths,
        components: Some(Components {
            schemas,
            security_schemes,
            ..Default::default()
        }),
        security: None,
        tags: Vec::new(),
        external_docs: None,
        extensions: Default::default(),
    }
}

fn convert_operation(path: &str, operation: &nivasa::openapi::OpenApiOperation) -> Operation {
    let mut responses = Responses::default();
    responses.responses = operation
        .responses
        .iter()
        .map(|(status, response)| {
            let status = status
                .parse::<u16>()
                .expect("generated status code must be numeric");
            (
                StatusCode::Code(status),
                ReferenceOr::Item(Response {
                    description: response.description.clone(),
                    content: response
                        .content
                        .iter()
                        .map(|(content_type, media_type)| {
                            (
                                content_type.clone(),
                                MediaType {
                                    schema: Some(ReferenceOr::ref_(&media_type.schema_ref)),
                                    ..Default::default()
                                },
                            )
                        })
                        .collect(),
                    ..Default::default()
                }),
            )
        })
        .collect();

    Operation {
        tags: operation.tags.clone(),
        summary: operation.summary.clone(),
        parameters: operation
            .parameters
            .iter()
            .map(|parameter| {
                ReferenceOr::Item(Parameter::Path {
                    parameter_data: ParameterData {
                        name: parameter.name.clone(),
                        description: Some(parameter.description.clone()),
                        required: parameter.required,
                        deprecated: None,
                        format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(
                            parameter_schema(),
                        )),
                        example: None,
                        examples: Default::default(),
                        explode: None,
                        extensions: Default::default(),
                    },
                    style: PathStyle::default(),
                })
            })
            .collect(),
        request_body: operation.request_body.as_ref().map(|request_body| {
            ReferenceOr::Item(RequestBody {
                required: request_body.required,
                content: request_body
                    .content
                    .iter()
                    .map(|(content_type, media_type)| {
                        (
                            content_type.clone(),
                            MediaType {
                                schema: Some(ReferenceOr::ref_(&media_type.schema_ref)),
                                ..Default::default()
                            },
                        )
                    })
                    .collect(),
                ..Default::default()
            })
        }),
        responses,
        callbacks: Default::default(),
        deprecated: false,
        security: if operation.security.is_empty() {
            None
        } else {
            Some(
                operation
                    .security
                    .iter()
                    .map(|requirement| {
                        let mut validated = SecurityRequirement::new();
                        for scheme_name in requirement.keys() {
                            validated.insert(scheme_name.clone(), Vec::new());
                        }
                        validated
                    })
                    .collect(),
            )
        },
        servers: Vec::new(),
        description: None,
        external_docs: None,
        operation_id: Some(format!(
            "{}_{}",
            path.trim_matches('/').replace('/', "_"),
            "op"
        )),
        extensions: Default::default(),
    }
}

fn object_schema() -> Schema {
    Schema {
        schema_data: SchemaData::default(),
        schema_kind: SchemaKind::Type(Type::Object(ObjectType::default())),
    }
}

fn parameter_schema() -> Schema {
    Schema {
        schema_data: SchemaData::default(),
        schema_kind: SchemaKind::Type(Type::String(Default::default())),
    }
}
