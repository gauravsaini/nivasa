mod controller;
mod filter;
mod injectable;
mod middleware;
mod module_macro;
mod validation;
mod scxml_handler;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn module(attr: TokenStream, item: TokenStream) -> TokenStream {
    module_macro::module_impl(attr, item)
}

#[proc_macro_attribute]
pub fn injectable(attr: TokenStream, item: TokenStream) -> TokenStream {
    injectable::injectable_impl(attr, item)
}

#[proc_macro_attribute]
pub fn controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::controller_impl(attr, item)
}

#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::get(attr, item)
}

#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::post(attr, item)
}

#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::put(attr, item)
}

#[proc_macro_attribute]
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::delete(attr, item)
}

#[proc_macro_attribute]
pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::patch(attr, item)
}

#[proc_macro_attribute]
pub fn head(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::head(attr, item)
}

#[proc_macro_attribute]
pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::options(attr, item)
}

#[proc_macro_attribute]
pub fn all(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::all(attr, item)
}

#[proc_macro_attribute]
pub fn impl_controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::impl_controller(attr, item)
}

#[proc_macro_attribute]
pub fn scxml_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    scxml_handler::scxml_handler_impl(_attr, item)
}

#[proc_macro_derive(
    Dto,
    attributes(
        is_email,
        is_string,
        is_number,
        is_int,
        is_boolean,
        is_uuid,
        is_url,
        is_enum,
        matches,
        is_not_empty,
        is_optional,
        validate_nested,
        min_length,
        max_length
    )
)]
pub fn dto(input: TokenStream) -> TokenStream {
    validation::dto_impl(input)
}

#[proc_macro_derive(
    PartialDto,
    attributes(
        is_email,
        is_string,
        is_number,
        is_int,
        is_boolean,
        is_uuid,
        is_url,
        is_enum,
        matches,
        is_not_empty,
        is_optional,
        validate_nested,
        min_length,
        max_length
    )
)]
pub fn partial_dto(input: TokenStream) -> TokenStream {
    validation::partial_dto_impl(input)
}

#[proc_macro_attribute]
pub fn guard(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::guard(_attr, item)
}

#[proc_macro_attribute]
pub fn roles(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::roles(attr, item)
}

#[proc_macro_attribute]
pub fn set_metadata(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::set_metadata(attr, item)
}

#[proc_macro_attribute]
pub fn catch(attr: TokenStream, item: TokenStream) -> TokenStream {
    filter::catch(attr, item)
}

#[proc_macro_attribute]
pub fn catch_all(attr: TokenStream, item: TokenStream) -> TokenStream {
    filter::catch_all(attr, item)
}

#[proc_macro_attribute]
pub fn interceptor(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::interceptor(_attr, item)
}

#[proc_macro_attribute]
pub fn use_filters(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::use_filters(attr, item)
}

#[proc_macro_attribute]
pub fn middleware(attr: TokenStream, item: TokenStream) -> TokenStream {
    middleware::middleware(attr, item)
}

#[proc_macro_attribute]
pub fn body(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn param(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn query(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn headers(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn header(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::header(_attr, item)
}

#[proc_macro_attribute]
pub fn req(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn res(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn custom_param(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn http_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::http_code(attr, item)
}

#[proc_macro_attribute]
pub fn ip(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn session(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn file(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn files(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
