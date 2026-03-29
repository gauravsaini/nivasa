use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Error, ItemStruct, LitStr, Path};

pub fn catch(attr: TokenStream, item: TokenStream) -> TokenStream {
    let exception = parse_macro_input!(attr as Path);
    let input = parse_macro_input!(item as ItemStruct);
    expand_catch(exception, input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

pub fn catch_all(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[catch_all]` does not take arguments",
        )
        .into_compile_error()
        .into();
    }

    let input = parse_macro_input!(item as ItemStruct);
    expand_catch_all(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_catch(exception: Path, input: ItemStruct) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let exception_type = exception.clone();
    let exception = exception.to_token_stream().to_string().replace(' ', "");
    let exception = LitStr::new(&exception, proc_macro2::Span::call_site());

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub const __NIVASA_FILTER_EXCEPTION: &'static str = #exception;

            pub fn __nivasa_filter_exception() -> &'static str {
                Self::__NIVASA_FILTER_EXCEPTION
            }

            pub fn __nivasa_filter_exception_type() -> &'static str {
                ::std::any::type_name::<#exception_type>()
            }
        }

        impl #impl_generics ::nivasa_filters::ExceptionFilterMetadata for #name #ty_generics #where_clause {
            fn exception_type(&self) -> Option<&'static str> {
                Some(Self::__nivasa_filter_exception_type())
            }
        }
    })
}

fn expand_catch_all(input: ItemStruct) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub const __NIVASA_FILTER_CATCH_ALL: bool = true;

            pub fn __nivasa_filter_catch_all() -> bool {
                Self::__NIVASA_FILTER_CATCH_ALL
            }
        }

        impl #impl_generics ::nivasa_filters::ExceptionFilterMetadata for #name #ty_generics #where_clause {
            fn is_catch_all(&self) -> bool {
                true
            }
        }
    })
}
