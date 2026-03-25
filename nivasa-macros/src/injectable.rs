use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    Error, Field, Fields, GenericArgument, Ident, ItemStruct, LitStr, PathArguments, Result, Token,
    Type,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InjectableScope {
    Singleton,
    Scoped,
    Transient,
}

impl Default for InjectableScope {
    fn default() -> Self {
        Self::Singleton
    }
}

impl InjectableScope {
    fn to_tokens(self) -> proc_macro2::TokenStream {
        match self {
            Self::Singleton => quote!(nivasa_core::di::ProviderScope::Singleton),
            Self::Scoped => quote!(nivasa_core::di::ProviderScope::Scoped),
            Self::Transient => quote!(nivasa_core::di::ProviderScope::Transient),
        }
    }
}

#[derive(Debug, Default)]
struct InjectableArgs {
    scope: InjectableScope,
}

impl Parse for InjectableArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut args = InjectableArgs::default();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "scope" => {
                    args.scope = parse_scope(&value)?;
                }
                other => {
                    return Err(Error::new(
                        key.span(),
                        format!(
                            "unknown `#[injectable]` argument `{other}`; expected `scope = \"singleton\" | \"scoped\" | \"transient\"`"
                        ),
                    ));
                }
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        Ok(args)
    }
}

fn parse_scope(value: &LitStr) -> Result<InjectableScope> {
    match value.value().as_str() {
        "singleton" => Ok(InjectableScope::Singleton),
        "scoped" => Ok(InjectableScope::Scoped),
        "transient" => Ok(InjectableScope::Transient),
        other => Err(Error::new(
            value.span(),
            format!(
                "invalid injectable scope `{other}`; expected `singleton`, `scoped`, or `transient`"
            ),
        )),
    }
}

fn has_inject_attr(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("inject"))
}

fn strip_inject_attr(field: &mut Field) {
    field.attrs.retain(|attr| !attr.path().is_ident("inject"));
}

fn extract_arc_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Arc" {
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }

    None
}

fn extract_optional_arc_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return extract_arc_inner_type(inner_ty);
                    }
                }
            }
        }
    }

    None
}

fn extract_lazy_arc_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Lazy" {
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return extract_arc_inner_type(inner_ty);
                    }
                }
            }
        }
    }

    None
}

fn build_lazy_resolution(
    field: &Field,
    field_name: Option<&Ident>,
) -> Result<(proc_macro2::TokenStream, Vec<Type>)> {
    let ty = &field.ty;

    if let Some(inner_ty) = extract_lazy_arc_inner_type(ty) {
        let inner_ty = inner_ty.clone();
        let resolution = match field_name {
            Some(name) => quote! {
                #name: {
                    let container = <nivasa_core::di::container::DependencyContainer as ::core::clone::Clone>::clone(container);
                    nivasa_core::di::Lazy::new(move || {
                        let container = container.clone();
                        async move {
                            container.resolve::<#inner_ty>().await
                        }
                    })
                }
            },
            None => quote! {
                {
                    let container = <nivasa_core::di::container::DependencyContainer as ::core::clone::Clone>::clone(container);
                    nivasa_core::di::Lazy::new(move || {
                        let container = container.clone();
                        async move {
                            container.resolve::<#inner_ty>().await
                        }
                    })
                }
            },
        };

        return Ok((resolution, vec![]));
    }

    Err(Error::new(
        field.span(),
        "`Lazy` fields must use `Lazy<Arc<T>>`",
    ))
}

fn build_injected_resolution(
    field: &Field,
    field_name: Option<&Ident>,
) -> Result<(proc_macro2::TokenStream, Vec<Type>)> {
    let ty = &field.ty;

    if extract_lazy_arc_inner_type(ty).is_some() {
        return build_lazy_resolution(field, field_name);
    }

    if let Some(inner_ty) = extract_arc_inner_type(ty) {
        let inner_ty = inner_ty.clone();
        let resolution = match field_name {
            Some(name) => quote! {
                #name: container.resolve::<#inner_ty>().await?
            },
            None => quote! {
                container.resolve::<#inner_ty>().await?
            },
        };
        return Ok((resolution, vec![inner_ty]));
    }

    if let Some(inner_ty) = extract_optional_arc_inner_type(ty) {
        let inner_ty = inner_ty.clone();
        let resolution = match field_name {
            Some(name) => quote! {
                #name: container.resolve_optional::<#inner_ty>().await?
            },
            None => quote! {
                container.resolve_optional::<#inner_ty>().await?
            },
        };
        return Ok((resolution, vec![inner_ty]));
    }

    Err(Error::new(
        field.span(),
        "`#[inject]` fields must use `Arc<T>` or `Option<Arc<T>>`",
    ))
}

fn build_fallback_resolution(
    field: &Field,
    field_name: Option<&Ident>,
) -> (proc_macro2::TokenStream, Vec<Type>) {
    let ty = &field.ty;

    if let Some(inner_ty) = extract_lazy_arc_inner_type(ty) {
        let inner_ty = inner_ty.clone();
        let resolution = match field_name {
            Some(name) => quote! {
                #name: {
                    let container = <nivasa_core::di::container::DependencyContainer as ::core::clone::Clone>::clone(container);
                    nivasa_core::di::Lazy::new(move || {
                        let container = container.clone();
                        async move {
                            container.resolve::<#inner_ty>().await
                        }
                    })
                }
            },
            None => quote! {
                {
                    let container = <nivasa_core::di::container::DependencyContainer as ::core::clone::Clone>::clone(container);
                    nivasa_core::di::Lazy::new(move || {
                        let container = container.clone();
                        async move {
                            container.resolve::<#inner_ty>().await
                        }
                    })
                }
            },
        };

        return (resolution, vec![]);
    }

    if let Some(inner_ty) = extract_arc_inner_type(ty) {
        let inner_ty = inner_ty.clone();
        let resolution = match field_name {
            Some(name) => quote! {
                #name: container.resolve::<#inner_ty>().await?
            },
            None => quote! {
                container.resolve::<#inner_ty>().await?
            },
        };
        return (resolution, vec![inner_ty]);
    }

    if let Some(inner_ty) = extract_optional_arc_inner_type(ty) {
        let inner_ty = inner_ty.clone();
        let resolution = match field_name {
            Some(name) => quote! {
                #name: container.resolve_optional::<#inner_ty>().await?
            },
            None => quote! {
                container.resolve_optional::<#inner_ty>().await?
            },
        };
        return (resolution, vec![inner_ty]);
    }

    let resolution = match field_name {
        Some(name) => quote! {
            #name: container.resolve::<#ty>().await?
        },
        None => quote! {
            container.resolve::<#ty>().await?
        },
    };

    (resolution, vec![ty.clone()])
}

fn strip_inject_attrs_from_struct(item: &mut ItemStruct) {
    match &mut item.fields {
        Fields::Named(fields) => {
            for field in &mut fields.named {
                strip_inject_attr(field);
            }
        }
        Fields::Unnamed(fields) => {
            for field in &mut fields.unnamed {
                strip_inject_attr(field);
            }
        }
        Fields::Unit => {}
    }
}

fn expand_injectable(
    args: InjectableArgs,
    mut input: ItemStruct,
) -> Result<proc_macro2::TokenStream> {
    let name = input.ident.clone();
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut dependency_types: Vec<Type> = Vec::new();
    let mut field_resolutions = Vec::new();

    match &input.fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                let field_name = field.ident.as_ref().ok_or_else(|| {
                    Error::new(
                        field.span(),
                        "named injectable fields must have identifiers",
                    )
                })?;

                let (resolution, mut deps) = if has_inject_attr(field) {
                    build_injected_resolution(field, Some(field_name))?
                } else {
                    build_fallback_resolution(field, Some(field_name))
                };

                dependency_types.append(&mut deps);
                field_resolutions.push(resolution);
            }
        }
        Fields::Unnamed(fields) => {
            for field in &fields.unnamed {
                let (resolution, mut deps) = if has_inject_attr(field) {
                    build_injected_resolution(field, None)?
                } else {
                    build_fallback_resolution(field, None)
                };

                dependency_types.append(&mut deps);
                field_resolutions.push(resolution);
            }
        }
        Fields::Unit => {}
    }

    strip_inject_attrs_from_struct(&mut input);

    let scope_tokens = args.scope.to_tokens();
    let dependency_types = dependency_types.iter();

    let struct_init = match &input.fields {
        Fields::Named(_) => quote! {
            Self {
                #(#field_resolutions),*
            }
        },
        Fields::Unnamed(_) => quote! {
            Self(#(#field_resolutions),*)
        },
        Fields::Unit => quote!(Self),
    };

    Ok(quote! {
        #input

        impl #impl_generics #name #ty_generics #where_clause {
            pub const __NIVASA_INJECTABLE_SCOPE: nivasa_core::di::ProviderScope = #scope_tokens;
        }

        #[async_trait::async_trait]
        impl #impl_generics nivasa_core::di::provider::Injectable for #name #ty_generics #where_clause {
            async fn build(container: &nivasa_core::di::container::DependencyContainer) -> Result<Self, nivasa_core::di::error::DiError> {
                Ok(#struct_init)
            }

            fn dependencies() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#dependency_types>()),*]
            }
        }
    })
}

pub fn injectable_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InjectableArgs);
    let input = parse_macro_input!(item as ItemStruct);

    match expand_injectable(args, input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
