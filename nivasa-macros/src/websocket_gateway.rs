use proc_macro::TokenStream;
use quote::quote;
use syn::{Error, ItemStruct, LitStr};

pub fn websocket_gateway(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = match syn::parse::<LitStr>(attr) {
        Ok(path) => path,
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[websocket_gateway]` expects a string path like `#[websocket_gateway(\"/ws\")]`",
            )
            .to_compile_error()
            .into();
        }
    };

    let input = match syn::parse::<ItemStruct>(item) {
        Ok(input) => input,
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[websocket_gateway]` only supports structs",
            )
            .to_compile_error()
            .into();
        }
    };

    expand_websocket_gateway(input, path)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

fn expand_websocket_gateway(
    input: ItemStruct,
    path: LitStr,
) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn __nivasa_websocket_gateway_metadata() -> &'static str {
                #path
            }
        }
    })
}
