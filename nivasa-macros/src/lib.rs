mod injectable;
mod controller;
mod module_macro;
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
pub fn impl_controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::impl_controller(attr, item)
}

#[proc_macro_attribute]
pub fn scxml_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    scxml_handler::scxml_handler_impl(_attr, item)
}
