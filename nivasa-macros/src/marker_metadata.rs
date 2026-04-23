use quote::ToTokens;
use syn::{punctuated::Punctuated, spanned::Spanned, Error, ImplItemFn, LitStr, Meta, Path, Token};

const GUARD_MARKER_PREFIX: &str = "nivasa-guard:";
const INTERCEPTOR_MARKER_PREFIX: &str = "nivasa-interceptor:";

pub fn collect_guard_names(method: &ImplItemFn) -> syn::Result<Vec<LitStr>> {
    collect_marker_names(
        method,
        "guard",
        GUARD_MARKER_PREFIX,
        "`#[guard]` requires at least one guard type",
        "invalid guard marker",
    )
}

pub fn collect_interceptor_names(method: &ImplItemFn) -> syn::Result<Vec<LitStr>> {
    collect_marker_names(
        method,
        "interceptor",
        INTERCEPTOR_MARKER_PREFIX,
        "`#[interceptor]` requires at least one interceptor type",
        "invalid interceptor marker",
    )
}

fn collect_marker_names(
    method: &ImplItemFn,
    attr_name: &str,
    doc_prefix: &str,
    empty_attr_message: &'static str,
    empty_doc_message: &'static str,
) -> syn::Result<Vec<LitStr>> {
    let mut names = Vec::new();

    for attr in &method.attrs {
        if attr
            .path()
            .segments
            .last()
            .map(|segment| segment.ident == attr_name)
            .unwrap_or(false)
        {
            let paths = match &attr.meta {
                Meta::List(_) => {
                    attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?
                }
                _ => return Err(Error::new(attr.span(), empty_attr_message)),
            };

            if paths.is_empty() {
                return Err(Error::new(attr.span(), empty_attr_message));
            }

            names.extend(paths.into_iter().map(|path| {
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
        let Some(rest) = value.trim().strip_prefix(doc_prefix) else {
            continue;
        };

        let parsed = rest
            .trim()
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| LitStr::new(name, doc.span()))
            .collect::<Vec<_>>();

        if parsed.is_empty() {
            return Err(Error::new(doc.span(), empty_doc_message));
        }

        names.extend(parsed);
    }

    Ok(names)
}
