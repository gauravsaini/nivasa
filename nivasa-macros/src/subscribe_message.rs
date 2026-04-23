use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, FnArg, ImplItemFn, LitStr};

use crate::marker_metadata::{collect_guard_names, collect_interceptor_names};

pub fn subscribe_message(attr: TokenStream, item: TokenStream) -> TokenStream {
    let event = match syn::parse::<LitStr>(attr) {
        Ok(event) if !event.value().trim().is_empty() => event,
        Ok(event) => {
            return Error::new(
                event.span(),
                "`#[subscribe_message]` expects a non-empty event name like `#[subscribe_message(\"event_name\")]`",
            )
            .to_compile_error()
            .into();
        }
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[subscribe_message]` expects an event name like `#[subscribe_message(\"event_name\")]`",
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
                "`#[subscribe_message]` only supports inherent methods",
            )
            .to_compile_error()
            .into();
        }
    };

    if !matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_))) {
        return Error::new(
            method.sig.ident.span(),
            "`#[subscribe_message]` only supports inherent methods",
        )
        .to_compile_error()
        .into();
    }

    expand_subscribe_message(method, event)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

fn expand_subscribe_message(
    method: ImplItemFn,
    event: LitStr,
) -> syn::Result<proc_macro2::TokenStream> {
    let method_name = &method.sig.ident;
    let helper_name = format_ident!("__nivasa_subscribe_message_metadata_for_{}", method_name);
    let guard_helper_name = format_ident!(
        "__nivasa_subscribe_message_guard_metadata_for_{}",
        method_name
    );
    let interceptor_helper_name = format_ident!(
        "__nivasa_subscribe_message_interceptor_metadata_for_{}",
        method_name
    );
    let guard_names = collect_guard_names(&method)?;
    let interceptor_names = collect_interceptor_names(&method)?;

    Ok(quote! {
        #method

        pub fn #helper_name() -> (&'static str, &'static str) {
            (stringify!(#method_name), #event)
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
