use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input,
    spanned::Spanned,
    Attribute, Data, DeriveInput, Error, Field, Fields, LitInt, LitStr, Meta, Result, Type,
};

pub fn dto_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_dto(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_dto(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
            Fields::Unnamed(_) | Fields::Unit => {
                return Err(Error::new(
                    input.span(),
                    "`#[derive(Dto)]` only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(Error::new(
                input.span(),
                "`#[derive(Dto)]` only supports structs",
            ));
        }
    };

    let mut field_checks = Vec::new();

    for field in fields {
        field_checks.extend(build_field_checks(field)?);
    }

    Ok(quote! {
        impl #impl_generics nivasa_validation::Validate for #name #ty_generics #where_clause {
            fn validate(&self) -> ::core::result::Result<(), nivasa_validation::ValidationErrors> {
                let mut errors = nivasa_validation::ValidationErrors::new();
                #(#field_checks)*
                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }
    })
}

fn build_field_checks(field: &Field) -> Result<Vec<proc_macro2::TokenStream>> {
    let field_name = field
        .ident
        .as_ref()
        .ok_or_else(|| Error::new(field.span(), "validation only supports named fields"))?;
    let field_label = LitStr::new(&field_name.to_string(), field_name.span());
    let field_access = quote!(self.#field_name);
    let mut checks = Vec::new();

    for attr in &field.attrs {
        if attr.path().is_ident("is_email") {
            checks.push(quote! {
                if !#field_access.contains('@') {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("is_email", "must be a valid email"),
                    );
                }
            });
        } else if attr.path().is_ident("is_string") {
            ensure_string_type(field, attr)?;
        } else if attr.path().is_ident("is_number") {
            ensure_number_type(field, attr)?;
        } else if attr.path().is_ident("is_boolean") {
            ensure_boolean_type(field, attr)?;
        } else if attr.path().is_ident("validate_nested") {
            checks.push(build_nested_validation_check(field, attr)?);
        } else if attr.path().is_ident("min_length") {
            let min_length = parse_min_length(attr)?;
            let min_length_lit = LitInt::new(&min_length.to_string(), attr.span());
            let message = LitStr::new(
                &format!("must be at least {} characters", min_length),
                attr.span(),
            );

            checks.push(quote! {
                if #field_access.len() < #min_length_lit {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("min_length", #message),
                    );
                }
            });
        } else if attr.path().is_ident("max_length") {
            let max_length = parse_max_length(attr)?;
            let max_length_lit = LitInt::new(&max_length.to_string(), attr.span());
            let message = LitStr::new(
                &format!("must be at most {} characters", max_length),
                attr.span(),
            );

            checks.push(quote! {
                if #field_access.len() > #max_length_lit {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("max_length", #message),
                    );
                }
            });
        }
    }

    Ok(checks)
}

fn build_nested_validation_check(
    field: &Field,
    attr: &Attribute,
) -> Result<proc_macro2::TokenStream> {
    if !matches!(&attr.meta, Meta::Path(_)) {
        return Err(Error::new(
            attr.span(),
            "expected bare `#[validate_nested]`",
        ));
    }

    let field_name = field
        .ident
        .as_ref()
        .ok_or_else(|| Error::new(field.span(), "validation only supports named fields"))?;
    let field_label = LitStr::new(&field_name.to_string(), field_name.span());
    let field_access = quote!(self.#field_name);

    let child_errors_push = quote! {
        for mut child_error in child_errors.into_errors() {
            if child_error.field.is_empty() {
                child_error.field = #field_label.to_string();
            } else {
                child_error.field = ::std::format!("{}.{}", #field_label, child_error.field);
            }

            errors.push(child_error);
        }
    };

    if is_option_like_type(&field.ty) {
        Ok(quote! {
            if let Some(child) = &#field_access {
                match nivasa_validation::Validate::validate(child) {
                    Ok(()) => {}
                    Err(child_errors) => {
                        #child_errors_push
                    }
                }
            }
        })
    } else {
        Ok(quote! {
            match nivasa_validation::Validate::validate(&#field_access) {
                Ok(()) => {}
                Err(child_errors) => {
                    #child_errors_push
                }
            }
        })
    }
}

fn ensure_string_type(field: &Field, attr: &Attribute) -> Result<()> {
    if is_string_like_type(&field.ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a string field for `#[is_string]`",
        ))
    }
}

fn ensure_boolean_type(field: &Field, attr: &Attribute) -> Result<()> {
    if is_boolean_like_type(&field.ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a bool field for `#[is_boolean]`",
        ))
    }
}

fn ensure_number_type(field: &Field, attr: &Attribute) -> Result<()> {
    if is_number_like_type(&field.ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a numeric field for `#[is_number]`",
        ))
    }
}

fn is_string_like_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident == "String" || segment.ident == "str")
            .unwrap_or(false),
        Type::Reference(reference) => is_string_like_type(reference.elem.as_ref()),
        Type::Group(group) => is_string_like_type(group.elem.as_ref()),
        Type::Paren(paren) => is_string_like_type(paren.elem.as_ref()),
        _ => false,
    }
}

fn is_boolean_like_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident == "bool")
            .unwrap_or(false),
        Type::Reference(reference) => is_boolean_like_type(reference.elem.as_ref()),
        Type::Group(group) => is_boolean_like_type(group.elem.as_ref()),
        Type::Paren(paren) => is_boolean_like_type(paren.elem.as_ref()),
        _ => false,
    }
}

fn is_number_like_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| is_numeric_primitive(&segment.ident))
            .unwrap_or(false),
        Type::Reference(reference) => is_number_like_type(reference.elem.as_ref()),
        Type::Group(group) => is_number_like_type(group.elem.as_ref()),
        Type::Paren(paren) => is_number_like_type(paren.elem.as_ref()),
        _ => false,
    }
}

fn is_numeric_primitive(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
            | "u128" | "usize" | "f32" | "f64"
    )
}

fn is_option_like_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident == "Option")
            .unwrap_or(false),
        Type::Reference(reference) => is_option_like_type(reference.elem.as_ref()),
        Type::Group(group) => is_option_like_type(group.elem.as_ref()),
        Type::Paren(paren) => is_option_like_type(paren.elem.as_ref()),
        _ => false,
    }
}

fn parse_min_length(attr: &Attribute) -> Result<usize> {
    let value = attr
        .parse_args::<LitInt>()
        .map_err(|_| Error::new(attr.span(), "expected `#[min_length(<usize>)]`"))?;

    value
        .base10_parse::<usize>()
        .map_err(|_| Error::new(attr.span(), "expected `#[min_length(<usize>)]`"))
}

fn parse_max_length(attr: &Attribute) -> Result<usize> {
    let value = attr
        .parse_args::<LitInt>()
        .map_err(|_| Error::new(attr.span(), "expected `#[max_length(<usize>)]`"))?;

    value
        .base10_parse::<usize>()
        .map_err(|_| Error::new(attr.span(), "expected `#[max_length(<usize>)]`"))
}
