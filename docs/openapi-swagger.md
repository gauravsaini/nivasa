# OpenAPI / Swagger

Nivasa currently ships a practical OpenAPI and Swagger UI surface built around controller metadata and bootstrap-time route registration.

## What Exists

The current stack includes:

1. `#[api_tags("Users")]` for controller grouping.
1. `#[api_operation(summary = "...")]` for operation summaries.
1. `#[api_param(name = "...", description = "...")]` for path/query docs.
1. `#[api_body(type = ...)]` for request body schemas.
1. `#[api_response(status = ..., type = ..., description = "...")]` for response schemas.
1. `#[api_bearer_auth]` for auth documentation.
1. Generated OpenAPI 3.0 document assembly from controller and DTO metadata.
1. Swagger UI shell generation that points at the OpenAPI JSON route.

## Default Routes

By default, the bootstrap layer serves:

1. OpenAPI JSON at `/api/docs/openapi.json`.
1. Swagger UI at `/api/docs`.

You can override both paths with `AppBootstrapConfig::with_openapi_spec_path(...)` and `AppBootstrapConfig::with_swagger_ui_path(...)`.

## How To Use It

The current bootstrap surface exposes two helpers:

```rust
use nivasa::prelude::*;
use nivasa::openapi::OpenApiDocument;

let bootstrap = AppBootstrapConfig::default();
let spec = OpenApiDocument::default();

let _spec_server = bootstrap
    .serve_openapi_spec(&spec)
    .expect("openapi route should register");

let _ui_server = bootstrap
    .serve_swagger_ui()
    .expect("swagger ui route should register");
```

At the controller layer, decorate handlers directly:

```rust
use nivasa::prelude::*;

#[controller("/users")]
#[api_tags("Users")]
pub struct UserController;

#[impl_controller]
impl UserController {
    #[get("/")]
    #[api_operation(summary = "List users")]
    #[api_response(status = 200, type = UserListDto, description = "Success")]
    pub fn list(&self) -> Vec<UserListDto> {
        vec![]
    }
}
```

## Honest Boundaries

This surface is real, but it is still bootstrap-oriented:

1. The OpenAPI document is assembled from current controller metadata and DTO shapes.
1. The Swagger UI shell is a generated HTML page that points at the configured JSON route.
1. The full live-server story still depends on the broader app listen path that Phase 8 is finishing.

## Verify

The current repo has focused tests around these pieces, including generated spec and Swagger UI route checks.
