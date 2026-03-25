use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use std::collections::HashSet;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    spanned::Spanned,
    Attribute, Error, Expr, ExprLit, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, ItemStruct,
    Lit, LitInt, LitStr, Meta, PatType, Path, Result, Token,
};

const ROUTE_MARKER_PREFIX: &str = "nivasa-route:";
const RESPONSE_MARKER_PREFIX: &str = "nivasa-response:";

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
struct ControllerMethodBinding {
    route: RouteBinding,
    handler: Ident,
    parameters: Vec<ParameterBinding>,
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

fn expand_controller(args: ControllerArgs, input: ItemStruct) -> Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
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

fn attr_path_matches(attr: &Attribute, name: &str) -> bool {
    attr.path().is_ident(name)
        || attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
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
        let rendered = path
            .to_token_stream()
            .to_string()
            .replace(' ', "");

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
                    format!("`#[{}]` only supports bare or string-list syntax", kind.as_str()),
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
            let status_code = payload.parse::<u16>().map_err(|_| {
                Error::new(doc.span(), "invalid controller response status code")
            })?;

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
        let status_code = code.base10_parse::<u16>().map_err(|_| {
            Error::new(code.span(), "invalid controller response status code")
        })?;

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

fn collect_parameter_bindings(method: &mut ImplItemFn) -> Result<Vec<ParameterBinding>> {
    let mut parameters = Vec::new();

    for input in &mut method.sig.inputs {
        let FnArg::Typed(PatType { attrs, .. }) = input else {
            continue;
        };

        let mut retained_attrs = Vec::new();
        let mut parameter_binding: Option<ParameterBinding> = None;

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
                None => retained_attrs.push(attr),
            }
        }

        *attrs = retained_attrs;

        if let Some(binding) = parameter_binding {
            parameters.push(binding);
        }
    }

    Ok(parameters)
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
        let mut response_bindings = Vec::new();
        let mut retained_attrs = Vec::new();

        for attr in method.attrs.drain(..) {
            match parse_route_binding(&attr)? {
                Some(binding) => {
                    if method_route.is_some() {
                        return Err(Error::new(
                            attr.span(),
                            "a controller method can only use one HTTP method attribute",
                        ));
                    }
                    method_route = Some(binding);
                }
                None => match parse_response_binding(&attr)? {
                    Some(binding) => response_bindings.push(binding),
                    None => retained_attrs.push(attr),
                },
            }
        }

        method.attrs = retained_attrs;
        let parameters = collect_parameter_bindings(method)?;
        let has_controller_metadata = !response_bindings.is_empty() || !parameters.is_empty();
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
                parameters,
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

            pub fn __nivasa_controller_response_metadata(
            ) -> Vec<(&'static str, Option<u16>, Vec<(&'static str, &'static str)>)> {
                vec![
                    #(#response_entries),*
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
