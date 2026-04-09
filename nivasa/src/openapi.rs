use std::collections::{BTreeMap, BTreeSet};

/// Minimal OpenAPI 3.0 document assembled from controller metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiDocument {
    pub openapi: String,
    pub info: OpenApiInfo,
    pub paths: BTreeMap<String, BTreeMap<String, OpenApiOperation>>,
    pub components: OpenApiComponents,
}

/// Top-level OpenAPI info block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiInfo {
    pub title: String,
    pub version: String,
}

/// Minimal components block used by the first pure builder slice.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenApiComponents {
    pub schemas: BTreeMap<String, OpenApiSchema>,
    pub security_schemes: BTreeMap<String, OpenApiSecurityScheme>,
}

/// Placeholder schema entry keyed by DTO type name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiSchema {
    pub schema_type: String,
}

/// Minimal security scheme entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiSecurityScheme {
    pub scheme_type: String,
    pub scheme: String,
    pub bearer_format: Option<String>,
}

/// One path operation in the OpenAPI document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenApiOperation {
    pub tags: Vec<String>,
    pub summary: Option<String>,
    pub parameters: Vec<OpenApiParameter>,
    pub request_body: Option<OpenApiRequestBody>,
    pub responses: BTreeMap<String, OpenApiResponse>,
    pub security: Vec<OpenApiSecurityRequirement>,
}

/// One parameter entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiParameter {
    pub name: String,
    pub location: String,
    pub description: String,
    pub required: bool,
    pub schema: OpenApiInlineSchema,
}

/// Small inline schema for scalar parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiInlineSchema {
    pub schema_type: String,
}

/// Minimal request body entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiRequestBody {
    pub required: bool,
    pub content: BTreeMap<String, OpenApiMediaType>,
}

/// Minimal response entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiResponse {
    pub description: String,
    pub content: BTreeMap<String, OpenApiMediaType>,
}

/// Media type entry backed by a schema ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenApiMediaType {
    pub schema_ref: String,
}

/// Simple security requirement map.
pub type OpenApiSecurityRequirement = BTreeMap<String, Vec<String>>;

/// Bridge trait for controllers that already expose generated metadata helpers.
pub trait OpenApiControllerMetadataProvider {
    fn routes() -> Vec<(&'static str, String, &'static str)>;

    fn api_tags() -> Vec<&'static str> {
        Vec::new()
    }

    fn api_operation_metadata() -> Vec<(&'static str, Option<&'static str>)> {
        Vec::new()
    }

    fn api_param_metadata() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
        Vec::new()
    }

    fn api_response_metadata() -> Vec<(&'static str, Vec<(u16, &'static str, &'static str)>)> {
        Vec::new()
    }

    fn api_body_metadata() -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }

    fn api_bearer_auth_metadata() -> Vec<&'static str> {
        Vec::new()
    }
}

/// Owned controller metadata bundle consumed by the pure OpenAPI builder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenApiControllerMetadata {
    pub routes: Vec<(&'static str, String, &'static str)>,
    pub api_tags: Vec<&'static str>,
    pub api_operations: Vec<(&'static str, Option<&'static str>)>,
    pub api_params: Vec<(&'static str, Vec<(&'static str, &'static str)>)>,
    pub api_responses: Vec<(&'static str, Vec<(u16, &'static str, &'static str)>)>,
    pub api_bodies: Vec<(&'static str, &'static str)>,
    pub api_bearer_auth: Vec<&'static str>,
}

impl OpenApiControllerMetadata {
    pub fn from_provider<T: OpenApiControllerMetadataProvider>() -> Self {
        Self {
            routes: T::routes(),
            api_tags: T::api_tags(),
            api_operations: T::api_operation_metadata(),
            api_params: T::api_param_metadata(),
            api_responses: T::api_response_metadata(),
            api_bodies: T::api_body_metadata(),
            api_bearer_auth: T::api_bearer_auth_metadata(),
        }
    }
}

/// Build a minimal OpenAPI 3.0 document from generated controller metadata.
pub fn build_openapi_document(
    title: impl Into<String>,
    version: impl Into<String>,
    controllers: impl IntoIterator<Item = OpenApiControllerMetadata>,
) -> OpenApiDocument {
    let mut paths = BTreeMap::new();
    let mut schemas = BTreeMap::new();
    let mut security_schemes = BTreeMap::new();

    for controller in controllers {
        let tags = controller
            .api_tags
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let operations = controller
            .api_operations
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let params = controller.api_params.into_iter().collect::<BTreeMap<_, _>>();
        let responses = controller
            .api_responses
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        let bodies = controller.api_bodies.into_iter().collect::<BTreeMap<_, _>>();
        let bearer_auth = controller
            .api_bearer_auth
            .into_iter()
            .collect::<BTreeSet<_>>();

        for (method, route_path, handler) in controller.routes {
            let request_body = bodies.get(handler).map(|ty| {
                let schema_name = normalize_type_name(ty);
                schemas
                    .entry(schema_name.clone())
                    .or_insert_with(|| OpenApiSchema {
                        schema_type: "object".to_string(),
                    });

                OpenApiRequestBody {
                    required: true,
                    content: BTreeMap::from([(
                        "application/json".to_string(),
                        OpenApiMediaType {
                            schema_ref: schema_ref(&schema_name),
                        },
                    )]),
                }
            });

            let response_entries = responses
                .get(handler)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|(status, ty, description)| {
                    let schema_name = normalize_type_name(ty);
                    schemas
                        .entry(schema_name.clone())
                        .or_insert_with(|| OpenApiSchema {
                            schema_type: "object".to_string(),
                        });

                    (
                        status.to_string(),
                        OpenApiResponse {
                            description: description.to_string(),
                            content: BTreeMap::from([(
                                "application/json".to_string(),
                                OpenApiMediaType {
                                    schema_ref: schema_ref(&schema_name),
                                },
                            )]),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();

            let parameter_entries = params
                .get(handler)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|(name, description)| OpenApiParameter {
                    name: name.to_string(),
                    location: "path".to_string(),
                    description: description.to_string(),
                    required: true,
                    schema: OpenApiInlineSchema {
                        schema_type: "string".to_string(),
                    },
                })
                .collect::<Vec<_>>();

            let security = if bearer_auth.contains(handler) {
                security_schemes
                    .entry("bearerAuth".to_string())
                    .or_insert_with(|| OpenApiSecurityScheme {
                        scheme_type: "http".to_string(),
                        scheme: "bearer".to_string(),
                        bearer_format: Some("JWT".to_string()),
                    });

                vec![BTreeMap::from([("bearerAuth".to_string(), Vec::new())])]
            } else {
                Vec::new()
            };

            let operation = OpenApiOperation {
                tags: tags.clone(),
                summary: operations.get(handler).and_then(|summary| summary.map(str::to_string)),
                parameters: parameter_entries,
                request_body,
                responses: response_entries,
                security,
            };

            paths
                .entry(normalize_openapi_path(&route_path))
                .or_insert_with(BTreeMap::new)
                .insert(method.to_ascii_lowercase(), operation);
        }
    }

    OpenApiDocument {
        openapi: "3.0.0".to_string(),
        info: OpenApiInfo {
            title: title.into(),
            version: version.into(),
        },
        paths,
        components: OpenApiComponents {
            schemas,
            security_schemes,
        },
    }
}

fn normalize_openapi_path(path: &str) -> String {
    let trimmed = path.trim();
    let trimmed = if trimmed.is_empty() { "/" } else { trimmed };

    let segments = trimmed
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            if let Some(name) = segment.strip_prefix(':') {
                format!("{{{name}}}")
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>();

    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn normalize_type_name(ty: &str) -> String {
    ty.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn schema_ref(schema_name: &str) -> String {
    format!("#/components/schemas/{schema_name}")
}

