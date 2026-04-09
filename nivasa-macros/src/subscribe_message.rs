use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, FnArg, ImplItemFn, LitStr};

pub fn subscribe_message(attr: TokenStream, item: TokenStream) -> TokenStream {
    let event = match syn::parse::<LitStr>(attr) {
        Ok(event) => event,
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
    let helper_name = format_ident!(
        "__nivasa_subscribe_message_metadata_for_{}",
        method_name
    );

    Ok(quote! {
        #method

        pub fn #helper_name() -> (&'static str, &'static str) {
            (stringify!(#method_name), #event)
        }
    })
}
