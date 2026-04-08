use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use std::collections::HashSet;
use syn::{
    braced,
    parse::Parser,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    spanned::Spanned,
    Attribute, Error, Expr, ExprLit, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, ItemStruct, Lit,
    LitInt, LitStr, Meta, PatType, Path, Result, Token,
};

const ROUTE_MARKER_PREFIX: &str = "nivasa-route:";
const RESPONSE_MARKER_PREFIX: &str = "nivasa-response:";
const GUARD_MARKER_PREFIX: &str = "nivasa-guard:";
const ROLES_MARKER_PREFIX: &str = "nivasa-roles:";
const INTERCEPTOR_MARKER_PREFIX: &str = "nivasa-interceptor:";
const FILTER_MARKER_PREFIX: &str = "nivasa-filter:";
const PIPE_MARKER_PREFIX: &str = "nivasa-pipe:";
const SET_METADATA_MARKER_PREFIX: &str = "nivasa-set-metadata:";

#[derive(Debug, Default, Clone)]
struct ControllerArgs {
    path: Option<LitStr>,
    version: Option<LitStr>,
}

#[derive(Debug, Clone)]
struct RouteBinding {
    method: &'static str,
    path: LitStr,
}

#[derive(Debug, Clone)]
struct ParameterBinding {
    kind: &'static str,
    name: Option<LitStr>,
}

#[derive(Debug, Clone)]
struct ParameterPipeBinding {
    pipe: LitStr,
}

#[derive(Debug, Clone)]
struct ControllerMethodBinding {
    route: RouteBinding,
    handler: Ident,
    pipes: Vec<String>,
    parameters: Vec<ParameterBinding>,
    parameter_pipes: Vec<Vec<ParameterPipeBinding>>,
    guards: Vec<String>,
    roles: Vec<String>,
    interceptors: Vec<String>,
    filters: Vec<String>,
    metadata: Vec<MetadataBinding>,
    operation: Option<OperationBinding>,
    response: Option<ResponseBinding>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ParameterExtractorKind {
    Body,
    Param,
    Query,
    Headers,
    Header,
    Req,
    Res,
    CustomParam,
    Ip,
    Session,
    File,
    Files,
}

#[derive(Debug, Clone)]
struct ResponseBinding {
    status_code: Option<u16>,
    headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct MetadataBinding {
    key: String,
    value: String,
}

#[derive(Debug, Clone)]
struct OperationBinding {
    summary: LitStr,
}

impl ControllerArgs {
    fn set_path(&mut self, key: &Ident, value: LitStr) -> Result<()> {
        if self.path.is_some() {
            return Err(Error::new(
                key.span(),
                "duplicate `path` entry in `#[controller]`",
            ));
        }
        self.path = Some(value);
        Ok(())
    }

    fn set_version(&mut self, key: &Ident, value: LitStr) -> Result<()> {
        if self.version.is_some() {
            return Err(Error::new(
                key.span(),
                "duplicate `version` entry in `#[controller]`",
            ));
        }
        self.version = Some(value);
        Ok(())
    }
}

impl ParameterExtractorKind {
    fn as_str(self) -> &'static str {
        match self {
            ParameterExtractorKind::Body => "body",
            ParameterExtractorKind::Param => "param",
            ParameterExtractorKind::Query => "query",
            ParameterExtractorKind::Headers => "headers",
            ParameterExtractorKind::Header => "header",
            ParameterExtractorKind::Req => "req",
            ParameterExtractorKind::Res => "res",
            ParameterExtractorKind::CustomParam => "custom_param",
            ParameterExtractorKind::Ip => "ip",
            ParameterExtractorKind::Session => "session",
            ParameterExtractorKind::File => "file",
            ParameterExtractorKind::Files => "files",
        }
    }

    fn takes_name(self) -> bool {
        matches!(
            self,
            ParameterExtractorKind::Param
                | ParameterExtractorKind::Query
                | ParameterExtractorKind::Header
                | ParameterExtractorKind::CustomParam
        )
    }

    fn accepts_optional_name(self) -> bool {
        matches!(
            self,
            ParameterExtractorKind::Body
                | ParameterExtractorKind::Headers
                | ParameterExtractorKind::Req
                | ParameterExtractorKind::Res
        )
    }

    fn rejects_arguments(self) -> bool {
        matches!(
            self,
            ParameterExtractorKind::Ip
                | ParameterExtractorKind::Session
                | ParameterExtractorKind::File
                | ParameterExtractorKind::Files
        )
    }
}

impl Parse for ControllerArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(LitStr) {
            let path: LitStr = input.parse()?;
            let mut args = ControllerArgs {
                path: Some(path),
                version: None,
            };

            if input.is_empty() {
                return Ok(args);
            }

            input.parse::<Token![,]>()?;
            if input.is_empty() {
                return Ok(args);
            }

            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "version" => args.set_version(&key, value)?,
                other => {
                    return Err(Error::new(
                        key.span(),
                        format!("unknown `#[controller]` key `{other}`; expected `version`"),
                    ));
                }
            }

            if !input.is_empty() {
                return Err(Error::new(
                    input.span(),
                    "unexpected trailing controller arguments",
                ));
            }

            return Ok(args);
        }

        let content;
        braced!(content in input);

        let mut args = ControllerArgs::default();

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            let value: LitStr = content.parse()?;

            match key.to_string().as_str() {
                "path" => args.set_path(&key, value)?,
                "version" => args.set_version(&key, value)?,
                other => {
                    return Err(Error::new(
                        key.span(),
                        format!(
                            "unknown `#[controller]` key `{other}`; expected `path` or `version`"
                        ),
                    ));
                }
            }

            if content.is_empty() {
                break;
            }

            content.parse::<Token![,]>()?;
        }

        if args.path.is_none() {
            return Err(Error::new(
                input.span(),
                "missing `path` in `#[controller]` attribute",
            ));
        }

        Ok(args)
    }
}

fn expand_controller(
    args: ControllerArgs,
    mut input: ItemStruct,
) -> Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut controller_pipes = Vec::new();
    let mut controller_guards = Vec::new();
    let mut controller_roles = Vec::new();
    let mut controller_interceptors = Vec::new();
    let mut controller_filters = Vec::new();
    let mut controller_metadata = Vec::new();
    let mut retained_attrs = Vec::new();

    for attr in input.attrs.drain(..) {
        match parse_pipe_binding(&attr)? {
            Some(pipes) => controller_pipes.extend(pipes),
            None => match parse_guard_binding(&attr)? {
                Some(guards) => controller_guards.extend(guards),
                None => match parse_roles_binding(&attr)? {
                    Some(roles) => controller_roles.extend(roles),
                    None => match parse_interceptor_binding(&attr)? {
                        Some(interceptors) => controller_interceptors.extend(interceptors),
                        None => match parse_filter_binding(&attr)? {
                            Some(filters) => controller_filters.extend(filters),
                            None => match parse_set_metadata_binding(&attr)? {
                                Some(metadata) => controller_metadata.extend(metadata),
                                None => retained_attrs.push(attr),
                            },
                        },
                    },
                },
            },
        }
    }

    input.attrs = retained_attrs;
    let path = args
        .path
        .ok_or_else(|| Error::new(name.span(), "missing controller path"))?;
    let version = args.version;
    let version_const = version
        .as_ref()
        .map(|value| quote!(Some(#value)))
        .unwrap_or_else(|| quote!(None));
    let metadata_expr = version
        .as_ref()
        .map(|value| {
            quote! {
                ::nivasa_routing::ControllerMetadata::new(Self::__NIVASA_CONTROLLER_PATH)
                    .with_version(#value)
            }
        })
        .unwrap_or_else(|| {
            quote! {
                ::nivasa_routing::ControllerMetadata::new(Self::__NIVASA_CONTROLLER_PATH)
            }
        });
    let controller_metadata_entries = controller_metadata.iter().map(|entry| {
        let key = &entry.key;
        let value = &entry.value;
        quote! {
            (#key, #value)
        }
    });

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub const __NIVASA_CONTROLLER_PATH: &'static str = #path;
            pub const __NIVASA_CONTROLLER_VERSION: Option<&'static str> = #version_const;

            pub fn __nivasa_controller_path() -> &'static str {
                Self::__NIVASA_CONTROLLER_PATH
            }

            pub fn __nivasa_controller_version() -> Option<&'static str> {
                Self::__NIVASA_CONTROLLER_VERSION
            }

            pub fn __nivasa_controller_metadata() -> (&'static str, Option<&'static str>) {
                (
                    Self::__NIVASA_CONTROLLER_PATH,
                    Self::__NIVASA_CONTROLLER_VERSION,
                )
            }

            pub fn __nivasa_controller_guards() -> Vec<&'static str> {
                vec![
                    #(#controller_guards),*
                ]
            }

            pub fn __nivasa_controller_pipes() -> Vec<&'static str> {
                vec![
                    #(#controller_pipes),*
                ]
            }

            pub fn __nivasa_controller_roles() -> Vec<&'static str> {
                vec![
                    #(#controller_roles),*
                ]
            }

            pub fn __nivasa_controller_interceptors() -> Vec<&'static str> {
                vec![
                    #(#controller_interceptors),*
                ]
            }

            pub fn __nivasa_controller_filters() -> Vec<&'static str> {
                vec![
                    #(#controller_filters),*
                ]
            }

            pub fn __nivasa_controller_set_metadata(
            ) -> Vec<(&'static str, &'static str)> {
                vec![
                    #(#controller_metadata_entries),*
                ]
            }
        }

        impl #impl_generics ::nivasa_routing::Controller for #name #ty_generics #where_clause {
            fn metadata(&self) -> ::nivasa_routing::ControllerMetadata {
                #metadata_expr
            }
        }
    })
}

fn route_marker_attr(method: &'static str, path: &LitStr) -> Attribute {
    let marker = LitStr::new(
        &format!("{ROUTE_MARKER_PREFIX} {method} {}", path.value().trim()),
        path.span(),
    );
    parse_quote!(#[doc = #marker])
}

fn guard_marker_attr(guards: &[Path]) -> Attribute {
    let payload = guards
        .iter()
        .map(|guard| guard.to_token_stream().to_string().replace(' ', ""))
        .collect::<Vec<_>>()
        .join(",");
    let marker = LitStr::new(
        &format!("{GUARD_MARKER_PREFIX} {payload}"),
        proc_macro2::Span::call_site(),
    );
    parse_quote!(#[doc = #marker])
}

fn interceptor_marker_attr(interceptors: &[Path]) -> Attribute {
    let payload = interceptors
        .iter()
        .map(|interceptor| interceptor.to_token_stream().to_string().replace(' ', ""))
        .collect::<Vec<_>>()
        .join(",");
    let marker = LitStr::new(
        &format!("{INTERCEPTOR_MARKER_PREFIX} {payload}"),
        proc_macro2::Span::call_site(),
    );
    parse_quote!(#[doc = #marker])
}

fn filter_marker_attr(filters: &[Path]) -> Attribute {
    let payload = filters
        .iter()
        .map(|filter| filter.to_token_stream().to_string().replace(' ', ""))
        .collect::<Vec<_>>()
        .join(",");
    let marker = LitStr::new(
        &format!("{FILTER_MARKER_PREFIX} {payload}"),
        proc_macro2::Span::call_site(),
    );
    parse_quote!(#[doc = #marker])
}

fn pipe_marker_attr(pipes: &[Path]) -> Attribute {
    let payload = pipes
        .iter()
        .map(|pipe| pipe.to_token_stream().to_string().replace(' ', ""))
        .collect::<Vec<_>>()
        .join(",");
    let marker = LitStr::new(
        &format!("{PIPE_MARKER_PREFIX} {payload}"),
        proc_macro2::Span::call_site(),
    );
    parse_quote!(#[doc = #marker])
}

fn attr_path_matches(attr: &Attribute, name: &str) -> bool {
    attr.path().is_ident(name)
        || attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
}

fn parse_guard_binding(attr: &Attribute) -> Result<Option<Vec<String>>> {
    if attr_path_matches(attr, "guard") {
        let guards: syn::punctuated::Punctuated<Path, Token![,]> =
            attr.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;

        if guards.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[guard]` requires at least one guard type",
            ));
        }

        return Ok(Some(
            guards
                .into_iter()
                .map(|path| path.to_token_stream().to_string().replace(' ', ""))
                .collect(),
        ));
    }

    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(GUARD_MARKER_PREFIX) else {
        return Ok(None);
    };

    let guards = rest
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|guard| !guard.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if guards.is_empty() {
        return Err(Error::new(doc.span(), "invalid controller guard marker"));
    }

    Ok(Some(guards))
}

fn parse_roles_binding(attr: &Attribute) -> Result<Option<Vec<String>>> {
    if attr_path_matches(attr, "roles") {
        let roles: syn::punctuated::Punctuated<LitStr, Token![,]> =
            attr.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;

        if roles.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[roles]` requires at least one role name",
            ));
        }

        return Ok(Some(
            roles
                .into_iter()
                .map(|role| role.value().trim().to_owned())
                .collect(),
        ));
    }

    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(ROLES_MARKER_PREFIX) else {
        return Ok(None);
    };

    let roles = rest
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if roles.is_empty() {
        return Err(Error::new(doc.span(), "invalid controller roles marker"));
    }

    Ok(Some(roles))
}

fn parse_interceptor_binding(attr: &Attribute) -> Result<Option<Vec<String>>> {
    if attr_path_matches(attr, "interceptor") {
        let interceptors: syn::punctuated::Punctuated<Path, Token![,]> =
            attr.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;

        if interceptors.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[interceptor]` requires at least one interceptor type",
            ));
        }

        return Ok(Some(
            interceptors
                .into_iter()
                .map(|path| path.to_token_stream().to_string().replace(' ', ""))
                .collect(),
        ));
    }

    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(INTERCEPTOR_MARKER_PREFIX) else {
        return Ok(None);
    };

    let interceptors = rest
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|interceptor| !interceptor.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if interceptors.is_empty() {
        return Err(Error::new(
            doc.span(),
            "invalid controller interceptor marker",
        ));
    }

    Ok(Some(interceptors))
}

fn parse_filter_binding(attr: &Attribute) -> Result<Option<Vec<String>>> {
    if attr_path_matches(attr, "use_filters") {
        let filters: syn::punctuated::Punctuated<Path, Token![,]> =
            attr.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;

        if filters.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[use_filters]` requires at least one filter type",
            ));
        }

        return Ok(Some(
            filters
                .into_iter()
                .map(|path| path.to_token_stream().to_string().replace(' ', ""))
                .collect(),
        ));
    }

    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(FILTER_MARKER_PREFIX) else {
        return Ok(None);
    };

    let filters = rest
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|filter| !filter.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if filters.is_empty() {
        return Err(Error::new(doc.span(), "invalid controller filter marker"));
    }

    Ok(Some(filters))
}

fn parse_pipe_binding(attr: &Attribute) -> Result<Option<Vec<String>>> {
    if attr_path_matches(attr, "pipe") {
        let pipes: syn::punctuated::Punctuated<Path, Token![,]> = attr
            .parse_args_with(syn::punctuated::Punctuated::parse_terminated)
            .map_err(|_| Error::new(attr.span(), "`#[pipe]` requires at least one pipe type"))?;

        if pipes.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[pipe]` requires at least one pipe type",
            ));
        }

        return Ok(Some(
            pipes
                .into_iter()
                .map(|pipe| pipe.to_token_stream().to_string().replace(' ', ""))
                .collect(),
        ));
    }

    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(PIPE_MARKER_PREFIX) else {
        return Ok(None);
    };

    let pipes = rest
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|pipe| !pipe.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if pipes.is_empty() {
        return Err(Error::new(doc.span(), "invalid controller pipe marker"));
    }

    Ok(Some(pipes))
}

fn parse_set_metadata_binding(attr: &Attribute) -> Result<Option<Vec<MetadataBinding>>> {
    if attr_path_matches(attr, "set_metadata") {
        let mut key: Option<LitStr> = None;
        let mut value: Option<LitStr> = None;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("key") {
                key = Some(meta.value()?.parse()?);
                return Ok(());
            }

            if meta.path.is_ident("value") {
                value = Some(meta.value()?.parse()?);
                return Ok(());
            }

            Err(meta.error("expected `key` or `value` in `#[set_metadata]`"))
        })?;

        let key =
            key.ok_or_else(|| Error::new(attr.span(), "`#[set_metadata]` requires a `key` entry"))?;
        let value = value
            .ok_or_else(|| Error::new(attr.span(), "`#[set_metadata]` requires a `value` entry"))?;

        let key = key.value().trim().to_owned();
        let value = value.value().trim().to_owned();

        if key.is_empty() || value.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[set_metadata]` key and value cannot be empty",
            ));
        }

        return Ok(Some(vec![MetadataBinding { key, value }]));
    }

    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(SET_METADATA_MARKER_PREFIX) else {
        return Ok(None);
    };

    let rest = rest.trim();
    let Some((key, value)) = rest.split_once('=') else {
        return Err(Error::new(doc.span(), "invalid controller metadata marker"));
    };

    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err(Error::new(doc.span(), "invalid controller metadata marker"));
    }

    Ok(Some(vec![MetadataBinding {
        key: key.to_owned(),
        value: value.to_owned(),
    }]))
}

fn parse_api_operation_binding(attr: &Attribute) -> Result<Option<OperationBinding>> {
    if !attr_path_matches(attr, "api_operation") {
        return Ok(None);
    }

    let mut summary: Option<LitStr> = None;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("summary") {
            if summary.is_some() {
                return Err(meta.error("duplicate `summary` entry in `#[api_operation]`"));
            }

            summary = Some(meta.value()?.parse()?);
            return Ok(());
        }

        Err(meta.error("expected `summary = \"...\"` in `#[api_operation]`"))
    })?;

    let summary = summary.ok_or_else(|| {
        Error::new(attr.span(), "`#[api_operation]` requires a `summary` entry")
    })?;

    if summary.value().trim().is_empty() {
        return Err(Error::new(
            summary.span(),
            "`#[api_operation]` summary cannot be empty",
        ));
    }

    Ok(Some(OperationBinding { summary }))
}

fn parse_parameter_extractor(attr: &Attribute) -> Result<Option<ParameterBinding>> {
    let kind = if attr_path_matches(attr, "body") {
        Some(ParameterExtractorKind::Body)
    } else if attr_path_matches(attr, "param") {
        Some(ParameterExtractorKind::Param)
    } else if attr_path_matches(attr, "query") {
        Some(ParameterExtractorKind::Query)
    } else if attr_path_matches(attr, "headers") {
        Some(ParameterExtractorKind::Headers)
    } else if attr_path_matches(attr, "header") {
        Some(ParameterExtractorKind::Header)
    } else if attr_path_matches(attr, "req") {
        Some(ParameterExtractorKind::Req)
    } else if attr_path_matches(attr, "res") {
        Some(ParameterExtractorKind::Res)
    } else if attr_path_matches(attr, "custom_param") {
        Some(ParameterExtractorKind::CustomParam)
    } else if attr_path_matches(attr, "ip") {
        Some(ParameterExtractorKind::Ip)
    } else if attr_path_matches(attr, "session") {
        Some(ParameterExtractorKind::Session)
    } else if attr_path_matches(attr, "file") {
        Some(ParameterExtractorKind::File)
    } else if attr_path_matches(attr, "files") {
        Some(ParameterExtractorKind::Files)
    } else {
        None
    };

    let Some(kind) = kind else {
        return Ok(None);
    };

    let binding = if kind == ParameterExtractorKind::CustomParam {
        let path: Path = attr.parse_args()?;
        let rendered = path.to_token_stream().to_string().replace(' ', "");

        if rendered.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[custom_param]` requires a parameter extractor type",
            ));
        }

        ParameterBinding {
            kind: kind.as_str(),
            name: Some(LitStr::new(&rendered, path.span())),
        }
    } else if kind.takes_name() {
        let name: LitStr = attr.parse_args()?;
        if name.value().trim().is_empty() {
            return Err(Error::new(name.span(), "extractor name cannot be empty"));
        }

        ParameterBinding {
            kind: kind.as_str(),
            name: Some(name),
        }
    } else if kind.accepts_optional_name() {
        match &attr.meta {
            Meta::Path(_) => ParameterBinding {
                kind: kind.as_str(),
                name: None,
            },
            Meta::List(_) => {
                let name: LitStr = attr.parse_args()?;
                if name.value().trim().is_empty() {
                    return Err(Error::new(name.span(), "extractor name cannot be empty"));
                }

                ParameterBinding {
                    kind: kind.as_str(),
                    name: Some(name),
                }
            }
            Meta::NameValue(_) => {
                return Err(Error::new(
                    attr.span(),
                    format!(
                        "`#[{}]` only supports bare or string-list syntax",
                        kind.as_str()
                    ),
                ));
            }
        }
    } else if kind.rejects_arguments() {
        match &attr.meta {
            Meta::Path(_) => ParameterBinding {
                kind: kind.as_str(),
                name: None,
            },
            _ => {
                return Err(Error::new(
                    attr.span(),
                    format!("`#[{}]` does not take arguments", kind.as_str()),
                ));
            }
        }
    } else {
        unreachable!("unsupported extractor kind")
    };

    Ok(Some(binding))
}

fn parse_parameter_pipe(attr: &Attribute) -> Result<Option<Vec<ParameterPipeBinding>>> {
    if !attr_path_matches(attr, "pipe") {
        return Ok(None);
    }

    let paths: syn::punctuated::Punctuated<Path, Token![,]> = attr
        .parse_args_with(syn::punctuated::Punctuated::parse_terminated)
        .map_err(|_| Error::new(attr.span(), "`#[pipe]` requires at least one pipe type"))?;

    if paths.is_empty() {
        return Err(Error::new(
            attr.span(),
            "`#[pipe]` requires at least one pipe type",
        ));
    }

    Ok(Some(
        paths
            .into_iter()
            .map(|path| ParameterPipeBinding {
                pipe: LitStr::new(&path.to_token_stream().to_string().replace(' ', ""), path.span()),
            })
            .collect(),
    ))
}

fn response_marker_attr(text: &str) -> Attribute {
    let marker = LitStr::new(text, proc_macro2::Span::call_site());
    parse_quote!(#[doc = #marker])
}

fn parse_response_marker(attr: &Attribute) -> Result<Option<ResponseBinding>> {
    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(RESPONSE_MARKER_PREFIX) else {
        return Ok(None);
    };

    let rest = rest.trim();
    let Some((kind, payload)) = rest.split_once(' ') else {
        return Err(Error::new(doc.span(), "invalid controller response marker"));
    };

    let kind = kind.trim();
    let payload = payload.trim();
    if kind.is_empty() || payload.is_empty() {
        return Err(Error::new(doc.span(), "invalid controller response marker"));
    }

    match kind {
        "http_code" => {
            let status_code = payload
                .parse::<u16>()
                .map_err(|_| Error::new(doc.span(), "invalid controller response status code"))?;

            if !(100..=599).contains(&status_code) {
                return Err(Error::new(
                    doc.span(),
                    "controller response status code must be between 100 and 599",
                ));
            }

            Ok(Some(ResponseBinding {
                status_code: Some(status_code),
                headers: Vec::new(),
            }))
        }
        "header" => {
            let Some((name, value)) = payload.split_once(' ') else {
                return Err(Error::new(
                    doc.span(),
                    "invalid controller response header marker",
                ));
            };

            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return Err(Error::new(
                    doc.span(),
                    "controller response header name and value cannot be empty",
                ));
            }

            Ok(Some(ResponseBinding {
                status_code: None,
                headers: vec![(name.to_string(), value.to_string())],
            }))
        }
        other => Err(Error::new(
            doc.span(),
            format!("unsupported controller response marker `{other}`"),
        )),
    }
}

fn parse_route_marker(attr: &Attribute) -> Result<Option<RouteBinding>> {
    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc), ..
    }) = &meta.value
    else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(ROUTE_MARKER_PREFIX) else {
        return Ok(None);
    };

    let rest = rest.trim();
    let Some((method, path)) = rest.split_once(' ') else {
        return Err(Error::new(doc.span(), "invalid controller route marker"));
    };

    let method = method.trim();
    let path = path.trim();
    if method.is_empty() || path.is_empty() {
        return Err(Error::new(doc.span(), "invalid controller route marker"));
    }

    let path = LitStr::new(path, doc.span());
    let method = match method {
        "GET" => "GET",
        "POST" => "POST",
        "PUT" => "PUT",
        "DELETE" => "DELETE",
        "PATCH" => "PATCH",
        "HEAD" => "HEAD",
        "OPTIONS" => "OPTIONS",
        "ALL" => "ALL",
        other => {
            return Err(Error::new(
                doc.span(),
                format!("unsupported controller route method `{other}`"),
            ));
        }
    };

    Ok(Some(RouteBinding { method, path }))
}

fn parse_response_binding(attr: &Attribute) -> Result<Option<ResponseBinding>> {
    let response_code = if attr_path_matches(attr, "http_code") {
        Some(true)
    } else if attr_path_matches(attr, "header") {
        Some(false)
    } else {
        None
    };

    let Some(is_code) = response_code else {
        return parse_response_marker(attr);
    };

    if is_code {
        let code: syn::LitInt = attr.parse_args()?;
        let status_code = code
            .base10_parse::<u16>()
            .map_err(|_| Error::new(code.span(), "invalid controller response status code"))?;

        if !(100..=599).contains(&status_code) {
            return Err(Error::new(
                code.span(),
                "controller response status code must be between 100 and 599",
            ));
        }

        Ok(Some(ResponseBinding {
            status_code: Some(status_code),
            headers: Vec::new(),
        }))
    } else {
        let args: syn::punctuated::Punctuated<LitStr, Token![,]> =
            attr.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;

        if args.len() != 2 {
            return Err(Error::new(
                attr.span(),
                "`#[header]` expects exactly two string arguments",
            ));
        }

        let mut iter = args.into_iter();
        let name = iter.next().expect("header name exists");
        let value = iter.next().expect("header value exists");

        if name.value().trim().is_empty() || value.value().trim().is_empty() {
            return Err(Error::new(
                attr.span(),
                "controller response header name and value cannot be empty",
            ));
        }

        Ok(Some(ResponseBinding {
            status_code: None,
            headers: vec![(name.value(), value.value())],
        }))
    }
}

fn parse_route_binding(attr: &Attribute) -> Result<Option<RouteBinding>> {
    let method = if attr_path_matches(attr, "get") {
        Some("GET")
    } else if attr_path_matches(attr, "post") {
        Some("POST")
    } else if attr_path_matches(attr, "put") {
        Some("PUT")
    } else if attr_path_matches(attr, "delete") {
        Some("DELETE")
    } else if attr_path_matches(attr, "patch") {
        Some("PATCH")
    } else if attr_path_matches(attr, "head") {
        Some("HEAD")
    } else if attr_path_matches(attr, "options") {
        Some("OPTIONS")
    } else if attr_path_matches(attr, "all") {
        Some("ALL")
    } else {
        None
    };

    let Some(method) = method else {
        return parse_route_marker(attr);
    };

    let path: LitStr = attr.parse_args()?;
    if path.value().trim().is_empty() {
        return Err(Error::new(path.span(), "route path cannot be empty"));
    }

    Ok(Some(RouteBinding { method, path }))
}

fn collect_parameter_bindings(
    method: &mut ImplItemFn,
) -> Result<(Vec<ParameterBinding>, Vec<Vec<ParameterPipeBinding>>)> {
    let mut parameters = Vec::new();
    let mut parameter_pipes = Vec::new();

    for input in &mut method.sig.inputs {
        let FnArg::Typed(PatType { attrs, .. }) = input else {
            continue;
        };

        let mut retained_attrs = Vec::new();
        let mut parameter_binding: Option<ParameterBinding> = None;
        let mut parameter_pipe_bindings = Vec::new();
        let mut parameter_pipe_attr_count = 0usize;

        for attr in attrs.drain(..) {
            match parse_parameter_extractor(&attr)? {
                Some(binding) => {
                    if parameter_binding.is_some() {
                        return Err(Error::new(
                            attr.span(),
                            "a controller parameter can only use one extractor attribute",
                        ));
                    }
                    parameter_binding = Some(binding);
                }
                None => match parse_parameter_pipe(&attr)? {
                    Some(mut pipes) => {
                        parameter_pipe_attr_count += 1;
                        if parameter_pipe_attr_count > 1 {
                            return Err(Error::new(
                                attr.span(),
                                "a controller parameter can only use one `#[pipe]` attribute",
                            ));
                        }
                        parameter_pipe_bindings.append(&mut pipes);
                    }
                    None => retained_attrs.push(attr),
                },
            }
        }

        *attrs = retained_attrs;

        if let Some(binding) = parameter_binding {
            parameters.push(binding);
        }
        parameter_pipes.push(parameter_pipe_bindings);
    }

    Ok((parameters, parameter_pipes))
}

fn expand_impl_controller(mut input: ItemImpl) -> Result<proc_macro2::TokenStream> {
    if input.trait_.is_some() {
        return Err(Error::new(
            input.impl_token.span(),
            "#[impl_controller] only supports inherent impl blocks",
        ));
    }

    let self_ty = input.self_ty.clone();
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut methods = Vec::new();
    let mut seen_routes = HashSet::new();

    for item in &mut input.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };

        let mut method_route: Option<RouteBinding> = None;
        let mut pipe_bindings = Vec::new();
        let mut method_pipe_attr_count = 0usize;
        let mut response_bindings = Vec::new();
        let mut guard_bindings = Vec::new();
        let mut role_bindings = Vec::new();
        let mut interceptor_bindings = Vec::new();
        let mut filter_bindings = Vec::new();
        let mut metadata_bindings = Vec::new();
        let mut operation_binding: Option<OperationBinding> = None;
        let mut retained_attrs = Vec::new();

        for attr in method.attrs.drain(..) {
            match parse_guard_binding(&attr)? {
                Some(mut guards) => guard_bindings.append(&mut guards),
                None => match parse_roles_binding(&attr)? {
                    Some(mut roles) => role_bindings.append(&mut roles),
                    None => match parse_interceptor_binding(&attr)? {
                        Some(mut interceptors) => interceptor_bindings.append(&mut interceptors),
                        None => match parse_filter_binding(&attr)? {
                            Some(mut filters) => filter_bindings.append(&mut filters),
                            None => match parse_set_metadata_binding(&attr)? {
                                Some(mut metadata) => metadata_bindings.append(&mut metadata),
                                None => match parse_api_operation_binding(&attr)? {
                                    Some(operation) => {
                                        if operation_binding.is_some() {
                                            return Err(Error::new(
                                                attr.span(),
                                                "a controller method can only use one `#[api_operation]` attribute",
                                            ));
                                        }
                                        operation_binding = Some(operation);
                                    }
                                    None => match parse_route_binding(&attr)? {
                                        Some(binding) => {
                                            if method_route.is_some() {
                                                return Err(Error::new(
                                                    attr.span(),
                                                    "a controller method can only use one HTTP method attribute",
                                                ));
                                            }
                                            method_route = Some(binding);
                                        }
                                        None => match parse_pipe_binding(&attr)? {
                                            Some(mut pipes) => {
                                                method_pipe_attr_count += 1;
                                                pipe_bindings.append(&mut pipes);
                                            }
                                            None => match parse_response_binding(&attr)? {
                                                Some(binding) => response_bindings.push(binding),
                                                None => retained_attrs.push(attr),
                                            },
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            }
        }

        method.attrs = retained_attrs;
        let (parameters, parameter_pipes) = collect_parameter_bindings(method)?;
        if method_pipe_attr_count > 1 {
            return Err(Error::new(
                method.sig.ident.span(),
                "a controller method can only use one `#[pipe]` attribute",
            ));
        }
        let has_controller_metadata = !response_bindings.is_empty()
            || !pipe_bindings.is_empty()
            || !parameters.is_empty()
            || parameter_pipes.iter().any(|pipes| !pipes.is_empty())
            || !guard_bindings.is_empty()
            || !role_bindings.is_empty()
            || !interceptor_bindings.is_empty()
            || !filter_bindings.is_empty()
            || !metadata_bindings.is_empty()
            || operation_binding.is_some();
        let response = if response_bindings.is_empty() {
            None
        } else {
            let mut merged = ResponseBinding {
                status_code: None,
                headers: Vec::new(),
            };

            for binding in response_bindings {
                if let Some(status_code) = binding.status_code {
                    if merged.status_code.is_some() {
                        return Err(Error::new(
                            method.sig.ident.span(),
                            "a controller method can only use one `#[http_code]` attribute",
                        ));
                    }
                    merged.status_code = Some(status_code);
                }
                merged.headers.extend(binding.headers);
            }

            Some(merged)
        };

        if method_route.is_none() && has_controller_metadata {
            return Err(Error::new(
                method.sig.ident.span(),
                "controller metadata requires an HTTP method attribute",
            ));
        }

        if let Some(binding) = method_route {
            let route_path = binding.path.value();
            let route_key = (binding.method, route_path.clone());
            if !seen_routes.insert(route_key.clone()) {
                return Err(Error::new(
                    method.sig.ident.span(),
                    format!(
                        "duplicate controller route `{}` `{}`",
                        binding.method, route_path
                    ),
                ));
            }

            methods.push(ControllerMethodBinding {
                route: binding,
                handler: method.sig.ident.clone(),
                pipes: pipe_bindings,
                parameters,
                parameter_pipes,
                guards: guard_bindings,
                roles: role_bindings,
                interceptors: interceptor_bindings,
                filters: filter_bindings,
                metadata: metadata_bindings,
                operation: operation_binding,
                response,
            });
        }
    }

    let route_entries = methods.iter().map(|method| {
        let route_method = method.route.method;
        let route_path = &method.route.path;
        let handler = &method.handler;
        quote! {
            (
                #route_method,
                Self::__nivasa_controller_join_route(Self::__NIVASA_CONTROLLER_PATH, #route_path),
                stringify!(#handler),
            )
        }
    });

    let parameter_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let parameters = method.parameters.iter().map(|parameter| {
            let kind = parameter.kind;
            let name = parameter
                .name
                .as_ref()
                .map(|value| quote!(Some(#value)))
                .unwrap_or_else(|| quote!(None));

            quote! {
                (#kind, #name)
            }
        });

        quote! {
            (
                stringify!(#handler),
                vec![
                    #(#parameters),*
                ]
            )
        }
    });

    let pipe_entries = methods
        .iter()
        .filter(|method| !method.pipes.is_empty())
        .map(|method| {
            let handler = &method.handler;
            let pipes = method.pipes.iter().map(|pipe| quote!(#pipe));

            quote! {
                (
                    stringify!(#handler),
                    vec![
                        #(#pipes),*
                    ]
                )
            }
        });

    let parameter_pipe_entries = methods
        .iter()
        .filter(|method| method.parameter_pipes.iter().any(|pipes| !pipes.is_empty()))
        .map(|method| {
            let handler = &method.handler;
            let parameter_pipes = method.parameter_pipes.iter().map(|pipe| {
                let pipes = pipe.iter().map(|binding| {
                    let pipe = &binding.pipe;
                    quote!(#pipe)
                });

                quote! {
                    vec![
                        #(#pipes),*
                    ]
                }
            });

            quote! {
                (
                    stringify!(#handler),
                    vec![
                        #(#parameter_pipes),*
                    ]
                )
            }
        });

    let response_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let status_code = method
            .response
            .as_ref()
            .and_then(|response| response.status_code)
            .map(|value| quote!(Some(#value)))
            .unwrap_or_else(|| quote!(None));
        let headers = method
            .response
            .as_ref()
            .map(|response| {
                response
                    .headers
                    .iter()
                    .map(|(name, value)| quote! { (#name, #value) })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        quote! {
            (
                stringify!(#handler),
                #status_code,
                vec![
                    #(#headers),*
                ]
            )
        }
    });

    let guard_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let guards = method.guards.iter().map(|guard| quote!(#guard));

        quote! {
            (
                stringify!(#handler),
                vec![
                    #(#guards),*
                ]
            )
        }
    });

    let role_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let roles = method.roles.iter().map(|role| quote!(#role));

        quote! {
            (
                stringify!(#handler),
                vec![
                    #(#roles),*
                ]
            )
        }
    });

    let interceptor_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let interceptors = method
            .interceptors
            .iter()
            .map(|interceptor| quote!(#interceptor));

        quote! {
            (
                stringify!(#handler),
                vec![
                    #(#interceptors),*
                ]
            )
        }
    });

    let filter_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let filters = method.filters.iter().map(|filter| quote!(#filter));

        quote! {
            (
                stringify!(#handler),
                vec![
                    #(#filters),*
                ]
            )
        }
    });

    let metadata_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let metadata = method.metadata.iter().map(|entry| {
            let key = &entry.key;
            let value = &entry.value;
            quote! { (#key, #value) }
        });

        quote! {
            (
                stringify!(#handler),
                vec![
                    #(#metadata),*
                ]
            )
        }
    });

    let operation_entries = methods.iter().map(|method| {
        let handler = &method.handler;
        let summary = method
            .operation
            .as_ref()
            .map(|operation| {
                let summary = &operation.summary;
                quote!(Some(#summary))
            })
            .unwrap_or_else(|| quote!(None));

        quote! {
            (
                stringify!(#handler),
                #summary
            )
        }
    });

    Ok(quote! {
        #input

        impl #impl_generics #self_ty #ty_generics #where_clause {
            fn __nivasa_controller_join_route(prefix: &str, path: &str) -> String {
                let prefix = prefix.trim();
                let path = path.trim();

                let normalized_prefix = prefix.trim_end_matches('/');
                let normalized_path = path.trim_start_matches('/');

                match (normalized_prefix.is_empty(), normalized_path.is_empty()) {
                    (true, true) => "/".to_string(),
                    (true, false) => format!("/{}", normalized_path),
                    (false, true) => normalized_prefix.to_string(),
                    (false, false) => format!("{}/{}", normalized_prefix, normalized_path),
                }
            }

            pub fn __nivasa_controller_routes() -> Vec<(&'static str, String, &'static str)> {
                vec![
                    #(#route_entries),*
                ]
            }

            pub fn __nivasa_controller_parameter_metadata(
            ) -> Vec<(&'static str, Vec<(&'static str, Option<&'static str>)>)> {
                vec![
                    #(#parameter_entries),*
                ]
            }

            pub fn __nivasa_controller_pipe_metadata(
            ) -> Vec<(&'static str, Vec<&'static str>)> {
                vec![
                    #(#pipe_entries),*
                ]
            }

            pub fn __nivasa_controller_parameter_pipe_metadata(
            ) -> Vec<(&'static str, Vec<Vec<&'static str>>)> {
                vec![
                    #(#parameter_pipe_entries),*
                ]
            }

            pub fn __nivasa_controller_response_metadata(
            ) -> Vec<(&'static str, Option<u16>, Vec<(&'static str, &'static str)>)> {
                vec![
                    #(#response_entries),*
                ]
            }

            pub fn __nivasa_controller_guard_metadata(
            ) -> Vec<(&'static str, Vec<&'static str>)> {
                vec![
                    #(#guard_entries),*
                ]
            }

            pub fn __nivasa_controller_role_metadata(
            ) -> Vec<(&'static str, Vec<&'static str>)> {
                vec![
                    #(#role_entries),*
                ]
            }

            pub fn __nivasa_controller_interceptor_metadata(
            ) -> Vec<(&'static str, Vec<&'static str>)> {
                vec![
                    #(#interceptor_entries),*
                ]
            }

            pub fn __nivasa_controller_filter_metadata(
            ) -> Vec<(&'static str, Vec<&'static str>)> {
                vec![
                    #(#filter_entries),*
                ]
            }

            pub fn __nivasa_controller_set_metadata_metadata(
            ) -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
                vec![
                    #(#metadata_entries),*
                ]
            }

            pub fn __nivasa_controller_api_operation_metadata(
            ) -> Vec<(&'static str, Option<&'static str>)> {
                vec![
                    #(#operation_entries),*
                ]
            }
        }
    })
}

pub fn controller_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ControllerArgs);
    let input = parse_macro_input!(item as ItemStruct);

    match expand_controller(args, input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("GET", &path));
    quote!(#method).into()
}

pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("POST", &path));
    quote!(#method).into()
}

pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("PUT", &path));
    quote!(#method).into()
}

pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("DELETE", &path));
    quote!(#method).into()
}

pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("PATCH", &path));
    quote!(#method).into()
}

pub fn head(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("HEAD", &path));
    quote!(#method).into()
}

pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("OPTIONS", &path));
    quote!(#method).into()
}

pub fn all(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attr as LitStr);
    let mut method = parse_macro_input!(item as ImplItemFn);
    method.attrs.insert(0, route_marker_attr("ALL", &path));
    quote!(#method).into()
}

pub fn pipe(attr: TokenStream, item: TokenStream) -> TokenStream {
    let pipes = match syn::punctuated::Punctuated::<Path, Token![,]>::parse_terminated
        .parse(attr.clone())
    {
        Ok(pipes) if !pipes.is_empty() => pipes.into_iter().collect::<Vec<_>>(),
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[pipe]` requires at least one pipe type",
            )
            .to_compile_error()
            .into();
        }
        Ok(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[pipe]` requires at least one pipe type",
            )
            .to_compile_error()
            .into();
        }
    };

    if syn::parse::<PatType>(item.clone()).is_ok() {
        return item;
    }

    if let Ok(mut method) = syn::parse::<ImplItemFn>(item.clone()) {
        method.attrs.insert(0, pipe_marker_attr(&pipes));
        return quote!(#method).into();
    }

    if let Ok(mut item_struct) = syn::parse::<ItemStruct>(item.clone()) {
        item_struct.attrs.insert(0, pipe_marker_attr(&pipes));
        return quote!(#item_struct).into();
    }

    Error::new(
        proc_macro2::Span::call_site(),
        "`#[pipe]` only supports controller structs, controller method parameters, and inherent controller methods",
    )
    .to_compile_error()
    .into()
}

pub fn guard(attr: TokenStream, item: TokenStream) -> TokenStream {
    let guards = parse_macro_input!(
        attr with syn::punctuated::Punctuated::<Path, Token![,]>::parse_terminated
    );

    if guards.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[guard]` requires at least one guard type",
        )
        .to_compile_error()
        .into();
    }

    let guard_attr = guard_marker_attr(&guards.iter().cloned().collect::<Vec<_>>());

    if let Ok(mut method) = syn::parse::<ImplItemFn>(item.clone()) {
        method.attrs.insert(0, guard_attr);
        return quote!(#method).into();
    }

    if let Ok(mut item_struct) = syn::parse::<ItemStruct>(item.clone()) {
        item_struct.attrs.insert(0, guard_attr);
        return quote!(#item_struct).into();
    }

    Error::new(
        proc_macro2::Span::call_site(),
        "`#[guard]` only supports controller structs and inherent controller methods",
    )
    .to_compile_error()
    .into()
}

pub fn roles(attr: TokenStream, item: TokenStream) -> TokenStream {
    let roles = parse_macro_input!(
        attr with syn::punctuated::Punctuated::<LitStr, Token![,]>::parse_terminated
    );

    if roles.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[roles]` requires at least one role name",
        )
        .to_compile_error()
        .into();
    }

    let role_payload = roles
        .iter()
        .map(|role| role.value().trim().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let marker = LitStr::new(
        &format!("{ROLES_MARKER_PREFIX} {role_payload}"),
        proc_macro2::Span::call_site(),
    );
    let role_attr = parse_quote!(#[doc = #marker]);

    if let Ok(mut method) = syn::parse::<ImplItemFn>(item.clone()) {
        method.attrs.insert(0, role_attr);
        return quote!(#method).into();
    }

    if let Ok(mut item_struct) = syn::parse::<ItemStruct>(item.clone()) {
        item_struct.attrs.insert(0, role_attr);
        return quote!(#item_struct).into();
    }

    Error::new(
        proc_macro2::Span::call_site(),
        "`#[roles]` only supports controller structs and inherent controller methods",
    )
    .to_compile_error()
    .into()
}

pub fn set_metadata(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut key: Option<LitStr> = None;
    let mut value: Option<LitStr> = None;
    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("key") {
            key = Some(meta.value()?.parse()?);
            return Ok(());
        }

        if meta.path.is_ident("value") {
            value = Some(meta.value()?.parse()?);
            return Ok(());
        }

        Err(meta.error("expected `key` or `value` in `#[set_metadata]`"))
    });

    if let Err(err) = parser.parse(attr) {
        return err.to_compile_error().into();
    }

    let key = match key {
        Some(value) => value,
        None => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[set_metadata]` requires a `key` entry",
            )
            .to_compile_error()
            .into();
        }
    };

    let value = match value {
        Some(value) => value,
        None => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[set_metadata]` requires a `value` entry",
            )
            .to_compile_error()
            .into();
        }
    };

    if key.value().trim().is_empty() || value.value().trim().is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[set_metadata]` key and value cannot be empty",
        )
        .to_compile_error()
        .into();
    }

    let marker = LitStr::new(
        &format!(
            "{SET_METADATA_MARKER_PREFIX} {}={}",
            key.value().trim(),
            value.value().trim()
        ),
        proc_macro2::Span::call_site(),
    );
    let metadata_attr = parse_quote!(#[doc = #marker]);

    if let Ok(mut method) = syn::parse::<ImplItemFn>(item.clone()) {
        method.attrs.insert(0, metadata_attr);
        return quote!(#method).into();
    }

    if let Ok(mut item_struct) = syn::parse::<ItemStruct>(item.clone()) {
        item_struct.attrs.insert(0, metadata_attr);
        return quote!(#item_struct).into();
    }

    Error::new(
        proc_macro2::Span::call_site(),
        "`#[set_metadata]` only supports controller structs and inherent controller methods",
    )
    .to_compile_error()
    .into()
}

pub fn interceptor(attr: TokenStream, item: TokenStream) -> TokenStream {
    let interceptors = parse_macro_input!(
        attr with syn::punctuated::Punctuated::<Path, Token![,]>::parse_terminated
    );

    if interceptors.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[interceptor]` requires at least one interceptor type",
        )
        .to_compile_error()
        .into();
    }

    let interceptor_attr =
        interceptor_marker_attr(&interceptors.iter().cloned().collect::<Vec<_>>());

    if let Ok(mut method) = syn::parse::<ImplItemFn>(item.clone()) {
        method.attrs.insert(0, interceptor_attr);
        return quote!(#method).into();
    }

    if let Ok(mut item_struct) = syn::parse::<ItemStruct>(item.clone()) {
        item_struct.attrs.insert(0, interceptor_attr);
        return quote!(#item_struct).into();
    }

    Error::new(
        proc_macro2::Span::call_site(),
        "`#[interceptor]` only supports controller structs and inherent controller methods",
    )
    .to_compile_error()
    .into()
}

pub fn use_filters(attr: TokenStream, item: TokenStream) -> TokenStream {
    let filters = parse_macro_input!(
        attr with syn::punctuated::Punctuated::<Path, Token![,]>::parse_terminated
    );

    if filters.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "`#[use_filters]` requires at least one filter type",
        )
        .to_compile_error()
        .into();
    }

    let filter_attr = filter_marker_attr(&filters.iter().cloned().collect::<Vec<_>>());

    if let Ok(mut method) = syn::parse::<ImplItemFn>(item.clone()) {
        method.attrs.insert(0, filter_attr);
        return quote!(#method).into();
    }

    if let Ok(mut item_struct) = syn::parse::<ItemStruct>(item.clone()) {
        item_struct.attrs.insert(0, filter_attr);
        return quote!(#item_struct).into();
    }

    Error::new(
        proc_macro2::Span::call_site(),
        "`#[use_filters]` only supports controller structs and inherent controller methods",
    )
    .to_compile_error()
    .into()
}

pub fn http_code(attr: TokenStream, item: TokenStream) -> TokenStream {
    match syn::parse::<ImplItemFn>(item.clone()) {
        Ok(mut method) => {
            let code = parse_macro_input!(attr as LitInt);
            let marker = response_marker_attr(&format!("http_code {}", code.base10_digits()));
            method.attrs.insert(0, marker);
            quote!(#method).into()
        }
        Err(_) => item,
    }
}

pub fn header(attr: TokenStream, item: TokenStream) -> TokenStream {
    match syn::parse::<ImplItemFn>(item.clone()) {
        Ok(mut method) => {
            let args = parse_macro_input!(
                attr with syn::punctuated::Punctuated::<LitStr, Token![,]>::parse_terminated
            );
            if args.len() != 2 {
                return Error::new(
                    proc_macro2::Span::call_site(),
                    "`#[header]` expects exactly two string arguments",
                )
                .to_compile_error()
                .into();
            }

            let mut iter = args.into_iter();
            let name = iter.next().expect("header name exists");
            let value = iter.next().expect("header value exists");
            let marker = response_marker_attr(&format!(
                "header {} {}",
                name.value().trim(),
                value.value().trim()
            ));
            method.attrs.insert(0, marker);
            quote!(#method).into()
        }
        Err(_) => item,
    }
}

pub fn impl_controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(
            proc_macro2::Span::call_site(),
            "#[impl_controller] takes no arguments",
        )
        .to_compile_error()
        .into();
    }

    let input = parse_macro_input!(item as ItemImpl);

    match expand_impl_controller(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
