//! `nivasa-macros` exports Nivasa's procedural macros.
//!
//! Keep this crate as entry map:
//! - module/DI wiring: `#[module]`, `#[injectable]`
//! - HTTP/controller wiring: `#[controller]`, `#[get]`, `#[post]`, `#[guard]`, `#[interceptor]`, `#[pipe]`
//! - validation derives: `#[derive(Dto)]`, `#[derive(PartialDto)]`, `#[derive(ConfigSchema)]`
//! - websocket wiring: `#[websocket_gateway]`, `#[subscribe_message]`
//!
//! # Example
//!
//! ```rust
//! use nivasa_macros::Dto;
//!
//! #[derive(Dto)]
//! struct CreateUser {
//!     #[is_string]
//!     name: String,
//!
//!     #[is_email]
//!     email: String,
//! }
//! ```
//!
//! ```rust
//! use nivasa_macros::{controller, get};
//! use nivasa_routing::Controller;
//!
//! #[controller("/users")]
//! struct UsersController;
//!
//! impl UsersController {
//!     #[get("/")]
//!     fn list(&self) {}
//! }
//!
//! assert_eq!(UsersController::__nivasa_controller_path(), "/users");
//! assert_eq!(UsersController::__nivasa_controller_version(), None);
//! assert_eq!(UsersController.metadata().path(), "/users");
//! ```
//!
//! ```rust
//! use nivasa_macros::websocket_gateway;
//!
//! #[websocket_gateway({ path: "/ws", namespace: "/chat" })]
//! struct ChatGateway;
//!
//! assert_eq!(
//!     ChatGateway::__nivasa_websocket_gateway_metadata(),
//!     ("/ws", Some("/chat"))
//! );
//! ```

mod controller;
mod config_schema;
mod filter;
mod injectable;
mod middleware;
mod module_macro;
mod scxml_handler;
mod subscribe_message;
mod validation;
mod websocket_gateway;

use proc_macro::TokenStream;

#[proc_macro_attribute]
/// Mark a module container for Nivasa's DI graph.
pub fn module(attr: TokenStream, item: TokenStream) -> TokenStream {
    module_macro::module_impl(attr, item)
}

#[proc_macro_attribute]
/// Mark a type as injectable into Nivasa's DI container.
pub fn injectable(attr: TokenStream, item: TokenStream) -> TokenStream {
    injectable::injectable_impl(attr, item)
}

#[proc_macro_attribute]
/// Mark a controller type and route group.
pub fn controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::controller_impl(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP `GET` handler.
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::get(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP `POST` handler.
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::post(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP `PUT` handler.
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::put(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP `DELETE` handler.
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::delete(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP `PATCH` handler.
pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::patch(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP `HEAD` handler.
pub fn head(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::head(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP `OPTIONS` handler.
pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::options(attr, item)
}

#[proc_macro_attribute]
/// Register an HTTP handler for any verb.
pub fn all(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::all(attr, item)
}

#[proc_macro_attribute]
/// Expand controller methods into route registrations.
pub fn impl_controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::impl_controller(attr, item)
}

#[proc_macro_attribute]
/// Validate SCXML handler hooks at compile time.
pub fn scxml_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    scxml_handler::scxml_handler_impl(_attr, item)
}

#[proc_macro_derive(
    Dto,
    attributes(
        groups,
        min,
        max,
        is_email,
        is_string,
        is_number,
        is_int,
        is_boolean,
        is_uuid,
        is_url,
        is_enum,
        validate_if,
        matches,
        is_not_empty,
        custom_validate,
        array_min_size,
        array_max_size,
        is_optional,
        validate_nested,
        min_length,
        max_length
    )
)]
/// Derive validation metadata for required DTO fields.
///
/// ```rust
/// use nivasa_macros::Dto;
///
/// #[derive(Dto)]
/// struct SignupForm {
///     #[is_email]
///     email: String,
///
///     #[min_length(8)]
///     password: String,
/// }
/// ```
pub fn dto(input: TokenStream) -> TokenStream {
    validation::dto_impl(input)
}

#[proc_macro_derive(
    PartialDto,
    attributes(
        groups,
        min,
        max,
        is_email,
        is_string,
        is_number,
        is_int,
        is_boolean,
        is_uuid,
        is_url,
        is_enum,
        validate_if,
        matches,
        is_not_empty,
        custom_validate,
        array_min_size,
        array_max_size,
        is_optional,
        validate_nested,
        min_length,
        max_length
    )
)]
/// Derive validation metadata for optional DTO fields.
///
/// ```rust
/// use nivasa_macros::PartialDto;
///
/// #[derive(PartialDto)]
/// struct ProfilePatch {
///     #[is_optional]
///     #[is_string]
///     display_name: Option<String>,
/// }
/// ```
pub fn partial_dto(input: TokenStream) -> TokenStream {
    validation::partial_dto_impl(input)
}

#[proc_macro_derive(ConfigSchema, attributes(schema))]
/// Derive the static config schema contract from named struct fields.
///
/// ```rust,ignore
/// use nivasa_config::ConfigSchema;
/// use nivasa_macros::ConfigSchema as DeriveConfigSchema;
///
/// #[derive(DeriveConfigSchema)]
/// struct AppConfig {
///     host: String,
///     #[schema(default = "3000")]
///     port: String,
/// }
///
/// assert_eq!(AppConfig::required_keys(), &["host"]);
/// assert_eq!(AppConfig::defaults(), &[("port", "3000")]);
/// ```
pub fn config_schema(input: TokenStream) -> TokenStream {
    config_schema::config_schema_impl(input)
}

#[proc_macro_attribute]
/// Attach a guard to controller execution.
pub fn guard(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::guard(_attr, item)
}

#[proc_macro_attribute]
/// Mark route roles or permissions.
pub fn roles(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::roles(attr, item)
}

#[proc_macro_attribute]
/// Attach custom metadata to a controller or handler.
pub fn set_metadata(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::set_metadata(attr, item)
}

#[proc_macro_attribute]
/// Register an exception filter handler.
pub fn catch(attr: TokenStream, item: TokenStream) -> TokenStream {
    filter::catch(attr, item)
}

#[proc_macro_attribute]
/// Register a catch-all exception filter handler.
pub fn catch_all(attr: TokenStream, item: TokenStream) -> TokenStream {
    filter::catch_all(attr, item)
}

#[proc_macro_attribute]
/// Attach an interceptor to controller execution.
pub fn interceptor(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::interceptor(_attr, item)
}

#[proc_macro_attribute]
/// Attach filters to a controller or handler.
pub fn use_filters(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::use_filters(attr, item)
}

#[proc_macro_attribute]
/// Register middleware.
pub fn middleware(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::middleware(attr, item)
}

#[proc_macro_attribute]
/// Mark a websocket gateway type.
///
/// ```rust
/// use nivasa_macros::websocket_gateway;
///
/// #[websocket_gateway("/ws")]
/// struct ChatGateway;
///
/// assert_eq!(ChatGateway::__nivasa_websocket_gateway_metadata(), ("/ws", None));
/// ```
pub fn websocket_gateway(attr: TokenStream, item: TokenStream) -> TokenStream {
    websocket_gateway::websocket_gateway(attr, item)
}

#[proc_macro_attribute]
/// Register a websocket message handler.
pub fn subscribe_message(attr: TokenStream, item: TokenStream) -> TokenStream {
    subscribe_message::subscribe_message(attr, item)
}

#[proc_macro_attribute]
/// Bind request body data into a controller parameter.
pub fn body(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind websocket message body into a handler parameter.
pub fn message_body(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind connected socket context into a handler parameter.
pub fn connected_socket(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind a path parameter into a handler parameter.
pub fn param(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind query data into a handler parameter.
pub fn query(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Attach a pipe to a controller parameter.
pub fn pipe(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::pipe(attr, item)
}

#[proc_macro_attribute]
/// Bind all headers into a handler parameter.
pub fn headers(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind one header into a handler parameter.
pub fn header(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::header(_attr, item)
}

#[proc_macro_attribute]
/// Bind raw request into a handler parameter.
pub fn req(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind response builder into a handler parameter.
pub fn res(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind custom extracted parameter data.
pub fn custom_param(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Attach explicit HTTP status metadata.
pub fn http_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::http_code(attr, item)
}

#[proc_macro_attribute]
/// Bind client IP into a handler parameter.
pub fn ip(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind session data into a handler parameter.
pub fn session(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind a single uploaded file into a handler parameter.
pub fn file(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
/// Bind uploaded files into a handler parameter.
pub fn files(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
