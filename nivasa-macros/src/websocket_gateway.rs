use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    Error, Ident, ItemStruct, LitStr, Result, Token,
};

struct WebSocketGatewayArgs {
    path: LitStr,
    namespace: Option<LitStr>,
}

impl Parse for WebSocketGatewayArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        if let Ok(path) = input.parse::<LitStr>() {
            return Ok(Self {
                path,
                namespace: None,
            });
        }

        let content;
        syn::braced!(content in input);

        let mut path = None;
        let mut namespace = None;

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            let value: LitStr = content.parse()?;

            match key.to_string().as_str() {
                "path" => {
                    if path.replace(value).is_some() {
                        return Err(Error::new(
                            key.span(),
                            "duplicate `path` entry in `#[websocket_gateway]`",
                        ));
                    }
                }
                "namespace" => {
                    if namespace.replace(value).is_some() {
                        return Err(Error::new(
                            key.span(),
                            "duplicate `namespace` entry in `#[websocket_gateway]`",
                        ));
                    }
                }
                other => {
                    return Err(Error::new(
                        key.span(),
                        format!(
                            "unknown `#[websocket_gateway]` key `{other}`; expected `path` or `namespace`"
                        ),
                    ));
                }
            }

            if content.is_empty() {
                break;
            }

            content.parse::<Token![,]>()?;
        }

        Ok(Self {
            path: path.ok_or_else(|| {
                Error::new(
                    proc_macro2::Span::call_site(),
                    "`#[websocket_gateway]` requires a `path` entry",
                )
            })?,
            namespace,
        })
    }
}

pub fn websocket_gateway(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = match syn::parse::<WebSocketGatewayArgs>(attr) {
        Ok(args) => args,
        Err(error) => return error.to_compile_error().into(),
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

    expand_websocket_gateway(input, args)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

fn expand_websocket_gateway(
    input: ItemStruct,
    args: WebSocketGatewayArgs,
) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let path = &args.path;
    let namespace = args
        .namespace
        .as_ref()
        .map(|namespace| quote!(Some(#namespace)))
        .unwrap_or_else(|| quote!(None));

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn __nivasa_websocket_gateway_metadata() -> (&'static str, Option<&'static str>) {
                (#path, #namespace)
            }
        }
    })
}
