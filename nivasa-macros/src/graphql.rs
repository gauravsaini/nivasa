use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    punctuated::Punctuated, spanned::Spanned, Error, FnArg, ImplItemFn, LitStr, Meta, Path, Token,
};

const GUARD_MARKER_PREFIX: &str = "nivasa-guard:";
const INTERCEPTOR_MARKER_PREFIX: &str = "nivasa-interceptor:";

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
                format!(
                    "`#[{kind}]` expects a field name like `#[{kind}(\"field_name\")]`"
                ),
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
    let guard_helper_name =
        format_ident!("__nivasa_graphql_{}_guard_metadata_for_{}", kind, method_name);
    let interceptor_helper_name = format_ident!(
        "__nivasa_graphql_{}_interceptor_metadata_for_{}",
        kind, method_name
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

fn collect_guard_names(method: &ImplItemFn) -> syn::Result<Vec<LitStr>> {
    let mut guards = Vec::new();

    for attr in &method.attrs {
        if attr
            .path()
            .segments
            .last()
            .map(|segment| segment.ident == "guard")
            .unwrap_or(false)
        {
            let paths = match &attr.meta {
                Meta::List(_) => {
                    attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?
                }
                _ => {
                    return Err(Error::new(
                        attr.span(),
                        "`#[guard]` requires at least one guard type",
                    ));
                }
            };

            if paths.is_empty() {
                return Err(Error::new(
                    attr.span(),
                    "`#[guard]` requires at least one guard type",
                ));
            }

            guards.extend(paths.into_iter().map(|path| {
                LitStr::new(
                    &path.to_token_stream().to_string().replace(' ', ""),
                    path.span(),
                )
            }));
            continue;
        }

        if !attr.path().is_ident("doc") {
            continue;
        }

        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };

        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(doc),
            ..
        }) = &meta.value
        else {
            continue;
        };

        let value = doc.value();
        let Some(rest) = value.trim().strip_prefix(GUARD_MARKER_PREFIX) else {
            continue;
        };

        let parsed = rest
            .trim()
            .split(',')
            .map(str::trim)
            .filter(|guard| !guard.is_empty())
            .map(|guard| LitStr::new(guard, doc.span()))
            .collect::<Vec<_>>();

        if parsed.is_empty() {
            return Err(Error::new(doc.span(), "invalid guard marker"));
        }

        guards.extend(parsed);
    }

    Ok(guards)
}

fn collect_interceptor_names(method: &ImplItemFn) -> syn::Result<Vec<LitStr>> {
    let mut interceptors = Vec::new();

    for attr in &method.attrs {
        if attr
            .path()
            .segments
            .last()
            .map(|segment| segment.ident == "interceptor")
            .unwrap_or(false)
        {
            let paths = match &attr.meta {
                Meta::List(_) => {
                    attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?
                }
                _ => {
                    return Err(Error::new(
                        attr.span(),
                        "`#[interceptor]` requires at least one interceptor type",
                    ));
                }
            };

            if paths.is_empty() {
                return Err(Error::new(
                    attr.span(),
                    "`#[interceptor]` requires at least one interceptor type",
                ));
            }

            interceptors.extend(paths.into_iter().map(|path| {
                LitStr::new(
                    &path.to_token_stream().to_string().replace(' ', ""),
                    path.span(),
                )
            }));
            continue;
        }

        if !attr.path().is_ident("doc") {
            continue;
        }

        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };

        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(doc),
            ..
        }) = &meta.value
        else {
            continue;
        };

        let value = doc.value();
        let Some(rest) = value.trim().strip_prefix(INTERCEPTOR_MARKER_PREFIX) else {
            continue;
        };

        let parsed = rest
            .trim()
            .split(',')
            .map(str::trim)
            .filter(|interceptor| !interceptor.is_empty())
            .map(|interceptor| LitStr::new(interceptor, doc.span()))
            .collect::<Vec<_>>();

        if parsed.is_empty() {
            return Err(Error::new(doc.span(), "invalid interceptor marker"));
        }

        interceptors.extend(parsed);
    }

    Ok(interceptors)
}
