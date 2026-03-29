use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Attribute, Error, Expr, ExprLit, Ident, ItemStruct, Lit, Meta, Result,
    spanned::Spanned,
    Path, Token, Type,
};

#[derive(Default)]
struct ModuleArgs {
    imports: Vec<Type>,
    controllers: Vec<Type>,
    providers: Vec<Type>,
    exports: Vec<Type>,
    middlewares: Vec<Type>,
}

#[derive(Debug, Clone)]
struct ModuleMetadataBinding {
    key: String,
    value: String,
}

impl ModuleArgs {
    fn insert_unique(target: &mut Vec<Type>, values: Vec<Type>, key: &Ident) -> Result<()> {
        if !target.is_empty() {
            return Err(Error::new(
                key.span(),
                format!("duplicate `{}` entry in `#[module]`", key),
            ));
        }

        *target = values;
        Ok(())
    }
}

impl Parse for ModuleArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        syn::braced!(content in input);

        let mut args = ModuleArgs::default();

        while !content.is_empty() {
            let key: Ident = content.parse()?;
            content.parse::<Token![:]>()?;

            let bracketed_content;
            syn::bracketed!(bracketed_content in content);

            let values = syn::punctuated::Punctuated::<Type, Token![,]>::parse_terminated(
                &bracketed_content,
            )?
            .into_iter()
            .collect::<Vec<_>>();

            match key.to_string().as_str() {
                "imports" => Self::insert_unique(&mut args.imports, values, &key)?,
                "controllers" => Self::insert_unique(&mut args.controllers, values, &key)?,
                "providers" => Self::insert_unique(&mut args.providers, values, &key)?,
                "exports" => Self::insert_unique(&mut args.exports, values, &key)?,
                "middlewares" => Self::insert_unique(&mut args.middlewares, values, &key)?,
                other => {
                    return Err(Error::new(
                        key.span(),
                        format!(
                            "unknown `#[module]` key `{other}`; expected one of `imports`, `controllers`, `providers`, `exports`, or `middlewares`"
                        ),
                    ));
                }
            }

            if content.is_empty() {
                break;
            }

            content.parse::<Token![,]>()?;
        }

        Ok(args)
    }
}

pub fn module_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ModuleArgs);
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;
    let module_interceptors = match parse_module_interceptors(&input.attrs) {
        Ok(interceptors) => interceptors,
        Err(err) => return err.to_compile_error().into(),
    };
    let module_roles = match parse_module_roles(&input.attrs) {
        Ok(roles) => roles,
        Err(err) => return err.to_compile_error().into(),
    };
    let module_set_metadata = match parse_module_set_metadata(&input.attrs) {
        Ok(metadata) => metadata,
        Err(err) => return err.to_compile_error().into(),
    };
    let module_set_metadata_entries = module_set_metadata.iter().map(|entry| {
        let key = &entry.key;
        let value = &entry.value;
        quote! {
            (#key, #value)
        }
    });

    let imports = args.imports;
    let providers = args.providers;
    let controllers = args.controllers;
    let exports = args.exports;
    let middlewares = args.middlewares;
    let module_guards = match parse_module_guards(&input.attrs) {
        Ok(guards) => guards,
        Err(err) => return err.to_compile_error().into(),
    };

    let controller_registrations = controllers.iter().map(|controller| {
        quote! {
            nivasa_core::module::ModuleControllerRegistration::new(
                std::any::TypeId::of::<#controller>(),
                #controller::__nivasa_controller_routes()
                    .into_iter()
                    .map(|(method, path, handler)| {
                        nivasa_core::module::ControllerRouteRegistration::new(
                            method,
                            path,
                            handler,
                        )
                    })
                    .collect(),
            )
        }
    });

    let expanded = quote! {
        #input

        impl #name {
            pub fn __nivasa_module_imports() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#imports>()),*]
            }

            pub fn __nivasa_module_controllers() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#controllers>()),*]
            }

            pub fn __nivasa_module_providers() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#providers>()),*]
            }

            pub fn __nivasa_module_exports() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#exports>()),*]
            }

            pub fn __nivasa_module_middlewares() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#middlewares>()),*]
            }

            pub fn __nivasa_module_guards() -> Vec<&'static str> {
                vec![#(#module_guards),*]
            }

            pub fn __nivasa_module_interceptors() -> Vec<&'static str> {
                vec![#(#module_interceptors),*]
            }

            pub fn __nivasa_module_roles() -> Vec<&'static str> {
                vec![#(#module_roles),*]
            }

            pub fn __nivasa_module_set_metadata() -> Vec<(&'static str, &'static str)> {
                vec![
                    #(#module_set_metadata_entries),*
                ]
            }

            pub fn __nivasa_module_controller_registrations(
            ) -> Vec<nivasa_core::module::ModuleControllerRegistration> {
                vec![
                    #(#controller_registrations),*
                ]
            }

            pub fn __nivasa_module_metadata() -> nivasa_core::module::ModuleMetadata {
                nivasa_core::module::ModuleMetadata::new()
                    .with_imports(Self::__nivasa_module_imports())
                    .with_providers(Self::__nivasa_module_providers())
                    .with_controllers(Self::__nivasa_module_controllers())
                    .with_exports(Self::__nivasa_module_exports())
                    .with_global(false)
            }
        }

        #[async_trait::async_trait]
        impl nivasa_core::module::Module for #name {
            fn metadata(&self) -> nivasa_core::module::ModuleMetadata {
                Self::__nivasa_module_metadata()
            }

            fn controller_registrations(&self) -> Vec<nivasa_core::module::ModuleControllerRegistration> {
                Self::__nivasa_module_controller_registrations()
            }

            async fn configure(&self, container: &nivasa_core::di::container::DependencyContainer) -> Result<(), nivasa_core::di::error::DiError> {
                #(
                    container.register_injectable::<#providers>(
                        nivasa_core::di::ProviderScope::Singleton,
                        <#providers as nivasa_core::di::provider::Injectable>::dependencies()
                    ).await;
                )*
                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}

const INTERCEPTOR_MARKER_PREFIX: &str = "__NIVASA_INTERCEPTOR__";
const GUARD_MARKER_PREFIX: &str = "__NIVASA_GUARD__";
const ROLES_MARKER_PREFIX: &str = "__NIVASA_ROLES__";
const SET_METADATA_MARKER_PREFIX: &str = "nivasa-set-metadata:";

fn attr_path_matches(attr: &Attribute, name: &str) -> bool {
    attr.path().is_ident(name)
        || attr
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
}

fn parse_interceptor_binding(attr: &Attribute) -> Result<Option<Vec<String>>> {
    if attr_path_matches(attr, "interceptor") {
        let interceptors: syn::punctuated::Punctuated<syn::Path, Token![,]> =
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
                .map(|path| path.into_token_stream().to_string().replace(' ', ""))
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
        return Err(Error::new(doc.span(), "invalid module interceptor marker"));
    }

    Ok(Some(interceptors))
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
                .map(|path| path.into_token_stream().to_string().replace(' ', ""))
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
        return Err(Error::new(doc.span(), "invalid module guard marker"));
    }

    Ok(Some(guards))
}

fn parse_module_interceptors(attrs: &[Attribute]) -> Result<Vec<String>> {
    let mut interceptors = Vec::new();

    for attr in attrs {
        if let Some(parsed) = parse_interceptor_binding(attr)? {
            interceptors.extend(parsed);
        }
    }

    Ok(interceptors)
}

fn parse_module_guards(attrs: &[Attribute]) -> Result<Vec<String>> {
    let mut guards = Vec::new();

    for attr in attrs {
        if let Some(parsed) = parse_guard_binding(attr)? {
            guards.extend(parsed);
        }
    }

    Ok(guards)
}

fn parse_roles_binding(attr: &Attribute) -> Result<Option<Vec<String>>> {
    if attr_path_matches(attr, "roles") {
        let roles: syn::punctuated::Punctuated<Path, Token![,]> =
            attr.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;

        if roles.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[roles]` requires at least one role type",
            ));
        }

        return Ok(Some(
            roles
                .into_iter()
                .map(|path| path.into_token_stream().to_string().replace(' ', ""))
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
        return Err(Error::new(doc.span(), "invalid module roles marker"));
    }

    Ok(Some(roles))
}

fn parse_module_roles(attrs: &[Attribute]) -> Result<Vec<String>> {
    let mut roles = Vec::new();

    for attr in attrs {
        if let Some(parsed) = parse_roles_binding(attr)? {
            roles.extend(parsed);
        }
    }

    Ok(roles)
}

fn parse_set_metadata_binding(attr: &Attribute) -> Result<Option<Vec<ModuleMetadataBinding>>> {
    if attr_path_matches(attr, "set_metadata") {
        let mut key: Option<syn::LitStr> = None;
        let mut value: Option<syn::LitStr> = None;

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

        let key = key.ok_or_else(|| {
            Error::new(
                attr.span(),
                "`#[set_metadata]` requires a `key` entry",
            )
        })?;
        let value = value.ok_or_else(|| {
            Error::new(
                attr.span(),
                "`#[set_metadata]` requires a `value` entry",
            )
        })?;

        let key = key.value().trim().to_owned();
        let value = value.value().trim().to_owned();

        if key.is_empty() || value.is_empty() {
            return Err(Error::new(
                attr.span(),
                "`#[set_metadata]` key and value cannot be empty",
            ));
        }

        return Ok(Some(vec![ModuleMetadataBinding { key, value }]));
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
        return Err(Error::new(doc.span(), "invalid module set_metadata marker"));
    };

    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err(Error::new(doc.span(), "invalid module set_metadata marker"));
    }

    Ok(Some(vec![ModuleMetadataBinding {
        key: key.to_owned(),
        value: value.to_owned(),
    }]))
}

fn parse_module_set_metadata(attrs: &[Attribute]) -> Result<Vec<ModuleMetadataBinding>> {
    let mut metadata = Vec::new();

    for attr in attrs {
        if let Some(parsed) = parse_set_metadata_binding(attr)? {
            metadata.extend(parsed);
        }
    }

    Ok(metadata)
}
