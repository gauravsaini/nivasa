//! # nivasa-macros
//!
//! Procedural macros for the Nivasa framework.
//!
//! Provides: `#[module]`, `#[injectable]`, `#[controller]`,
//! `#[get]`, `#[post]`, `#[guard]`, `#[interceptor]`, `#[pipe]`,
//! `#[catch]`, `#[scxml_handler]`, and more.

use proc_macro::TokenStream;

/// Placeholder for the `#[module]` attribute macro.
#[proc_macro_attribute]
pub fn module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Placeholder for the `#[injectable]` attribute macro.
#[proc_macro_attribute]
pub fn injectable(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Placeholder for the `#[controller]` attribute macro.
#[proc_macro_attribute]
pub fn controller(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Placeholder for the `#[scxml_handler]` attribute macro.
#[proc_macro_attribute]
pub fn scxml_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
