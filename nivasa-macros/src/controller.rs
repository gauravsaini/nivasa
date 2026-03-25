use proc_macro::TokenStream;
use quote::quote;
use syn::{
    braced,
    parse_quote,
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    Attribute, Error, Expr, ExprLit, Ident, ImplItem, ImplItemFn, ItemImpl, ItemStruct, Lit, LitStr, Meta, Result, Token,
};
use std::collections::HashSet;

const ROUTE_MARKER_PREFIX: &str = "nivasa-route:";

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

impl ControllerArgs {
    fn set_path(&mut self, key: &Ident, value: LitStr) -> Result<()> {
        if self.path.is_some() {
            return Err(Error::new(key.span(), "duplicate `path` entry in `#[controller]`"));
        }
        self.path = Some(value);
        Ok(())
    }

    fn set_version(&mut self, key: &Ident, value: LitStr) -> Result<()> {
        if self.version.is_some() {
            return Err(Error::new(key.span(), "duplicate `version` entry in `#[controller]`"));
        }
        self.version = Some(value);
        Ok(())
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
    let version = args.version.map(|value| quote!(Some(#value))).unwrap_or_else(|| quote!(None));

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub const __NIVASA_CONTROLLER_PATH: &'static str = #path;
            pub const __NIVASA_CONTROLLER_VERSION: Option<&'static str> = #version;

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

fn parse_route_marker(attr: &Attribute) -> Result<Option<RouteBinding>> {
    if !attr.path().is_ident("doc") {
        return Ok(None);
    }

    let Meta::NameValue(meta) = &attr.meta else {
        return Ok(None);
    };

    let Expr::Lit(ExprLit {
        lit: Lit::Str(doc),
        ..
    }) = &meta.value else {
        return Ok(None);
    };

    let value = doc.value();
    let Some(rest) = value.trim().strip_prefix(ROUTE_MARKER_PREFIX) else {
        return Ok(None);
    };

    let rest = rest.trim();
    let Some((method, path)) = rest.split_once(' ') else {
        return Err(Error::new(
            doc.span(),
            "invalid controller route marker",
        ));
    };

    let method = method.trim();
    let path = path.trim();
    if method.is_empty() || path.is_empty() {
        return Err(Error::new(
            doc.span(),
            "invalid controller route marker",
        ));
    }

    let path = LitStr::new(path, doc.span());
    let method = match method {
        "GET" => "GET",
        "POST" => "POST",
        other => {
            return Err(Error::new(
                doc.span(),
                format!("unsupported controller route method `{other}`"),
            ));
        }
    };

    Ok(Some(RouteBinding { method, path }))
}

fn parse_route_binding(attr: &Attribute) -> Result<Option<RouteBinding>> {
    let method = if attr_path_matches(attr, "get") {
        Some("GET")
    } else if attr_path_matches(attr, "post") {
        Some("POST")
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

    let mut routes = Vec::new();
    let mut seen_routes = HashSet::new();

    for item in &mut input.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };

        let mut method_route: Option<RouteBinding> = None;
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
                None => retained_attrs.push(attr),
            }
        }

        method.attrs = retained_attrs;

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

            routes.push((
                binding.method,
                binding.path,
                method.sig.ident.to_string(),
            ));
        }
    }

    let route_entries = routes.iter().map(|(method, path, handler)| {
        quote! {
            (
                #method,
                Self::__nivasa_controller_join_route(Self::__NIVASA_CONTROLLER_PATH, #path),
                #handler,
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

pub fn impl_controller(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(proc_macro2::Span::call_site(), "#[impl_controller] takes no arguments")
            .to_compile_error()
            .into();
    }

    let input = parse_macro_input!(item as ItemImpl);

    match expand_impl_controller(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
