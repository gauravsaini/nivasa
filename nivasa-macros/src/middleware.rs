use proc_macro::TokenStream;
use quote::quote;
use syn::{Error, ItemStruct};

pub fn middleware(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !proc_macro2::TokenStream::from(attr).is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[middleware]` does not accept arguments",
        )
        .to_compile_error()
        .into();
    }

    let input = match syn::parse::<ItemStruct>(item) {
        Ok(input) => input,
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[middleware]` only supports structs",
            )
            .to_compile_error()
            .into();
        }
    };

    expand_middleware(input)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

fn expand_middleware(input: ItemStruct) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn __nivasa_middleware_name() -> &'static str {
                stringify!(#name)
            }

            pub fn __nivasa_middleware_type_id() -> ::std::any::TypeId {
                ::std::any::TypeId::of::<Self>()
            }
        }
    })
}
