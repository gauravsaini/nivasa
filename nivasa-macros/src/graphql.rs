use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, FnArg, ImplItemFn, LitStr};

use crate::marker_metadata::{collect_guard_names, collect_interceptor_names};

pub fn resolver(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_graphql(attr, item, "resolver")
}

pub fn query(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_graphql(attr, item, "query")
}

pub fn mutation(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_graphql(attr, item, "mutation")
}

pub fn subscription(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_graphql(attr, item, "subscription")
}

fn expand_graphql(attr: TokenStream, item: TokenStream, kind: &str) -> TokenStream {
    let name = match syn::parse::<LitStr>(attr) {
        Ok(name) if !name.value().trim().is_empty() => name,
        Ok(name) => {
            return Error::new(
                name.span(),
                format!(
                    "`#[{kind}]` expects a non-empty GraphQL field name like `#[{kind}(\"field_name\")]`"
                ),
            )
            .to_compile_error()
            .into();
        }
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                format!("`#[{kind}]` expects a field name like `#[{kind}(\"field_name\")]`"),
            )
            .to_compile_error()
            .into();
        }
    };

    let method = match syn::parse::<ImplItemFn>(item) {
        Ok(method) => method,
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                format!("`#[{kind}]` only supports inherent methods"),
            )
            .to_compile_error()
            .into();
        }
    };

    if !matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_))) {
        return Error::new(
            method.sig.ident.span(),
            format!("`#[{kind}]` only supports inherent methods"),
        )
        .to_compile_error()
        .into();
    }

    expand_graphql_method(method, name, kind)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

fn expand_graphql_method(
    method: ImplItemFn,
    name: LitStr,
    kind: &str,
) -> syn::Result<proc_macro2::TokenStream> {
    let method_name = &method.sig.ident;
    let helper_name = format_ident!("__nivasa_graphql_{}_metadata_for_{}", kind, method_name);
    let guard_helper_name = format_ident!(
        "__nivasa_graphql_{}_guard_metadata_for_{}",
        kind,
        method_name
    );
    let interceptor_helper_name = format_ident!(
        "__nivasa_graphql_{}_interceptor_metadata_for_{}",
        kind,
        method_name
    );
    let guard_names = collect_guard_names(&method)?;
    let interceptor_names = collect_interceptor_names(&method)?;

    Ok(quote! {
        #method

        pub fn #helper_name() -> (&'static str, &'static str) {
            (stringify!(#method_name), #name)
        }

        pub fn #guard_helper_name() -> Vec<&'static str> {
            vec![
                #(#guard_names),*
            ]
        }

        pub fn #interceptor_helper_name() -> Vec<&'static str> {
            vec![
                #(#interceptor_names),*
            ]
        }
    })
}
