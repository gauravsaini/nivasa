use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    Attribute, Data, DeriveInput, Error, Field, Fields, GenericArgument, LitInt, LitStr, Meta,
    Path, PathArguments, Result, Token, Type, TypePath,
};

pub fn dto_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_validation_derive(&input, DeriveMode::Dto) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

pub fn partial_dto_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_validation_derive(&input, DeriveMode::PartialDto) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum DeriveMode {
    Dto,
    PartialDto,
}

impl DeriveMode {
    fn derive_name(self) -> &'static str {
        match self {
            DeriveMode::Dto => "Dto",
            DeriveMode::PartialDto => "PartialDto",
        }
    }

    fn requires_option_fields(self) -> bool {
        matches!(self, DeriveMode::PartialDto)
    }
}

fn expand_validation_derive(
    input: &DeriveInput,
    mode: DeriveMode,
) -> Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
            Fields::Unnamed(_) | Fields::Unit => {
                return Err(Error::new(
                    input.span(),
                    format!(
                        "`#[derive({})]` only supports structs with named fields",
                        mode.derive_name()
                    ),
                ));
            }
        },
        _ => {
            return Err(Error::new(
                input.span(),
                format!("`#[derive({})]` only supports structs", mode.derive_name()),
            ));
        }
    };

    let mut field_checks = Vec::new();

    for field in fields {
        field_checks.extend(build_field_checks(field, mode)?);
    }

    Ok(quote! {
        impl #impl_generics nivasa_validation::Validate for #name #ty_generics #where_clause {
            fn validate(&self) -> ::core::result::Result<(), nivasa_validation::ValidationErrors> {
                self.validate_with(&nivasa_validation::ValidationContext::new())
            }

            fn validate_with(
                &self,
                context: &nivasa_validation::ValidationContext,
            ) -> ::core::result::Result<(), nivasa_validation::ValidationErrors> {
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

fn build_field_checks(field: &Field, mode: DeriveMode) -> Result<Vec<proc_macro2::TokenStream>> {
    let field_name = field
        .ident
        .as_ref()
        .ok_or_else(|| Error::new(field.span(), "validation only supports named fields"))?;
    let field_label = LitStr::new(&field_name.to_string(), field_name.span());
    let is_optional = field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("is_optional"));
    let optional_attr = field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("is_optional"));
    let field_ty = if is_optional || mode.requires_option_fields() {
        option_inner_type(&field.ty).ok_or_else(|| {
            Error::new(
                optional_attr
                    .map(|attr| attr.span())
                    .unwrap_or_else(|| field.span()),
                match mode {
                    DeriveMode::Dto => "expected an `Option<T>` field for `#[is_optional]`",
                    DeriveMode::PartialDto => {
                        "expected an `Option<T>` field for `#[derive(PartialDto)]`"
                    }
                },
            )
        })?
    } else {
        &field.ty
    };
    let field_value_ident = format_ident!("__nivasa_validation_value");
    let field_value_access = quote!(#field_value_ident);
    let field_groups = field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("groups"))
        .try_fold(Vec::new(), |mut groups, attr| {
            groups.extend(parse_groups(attr)?);
            Ok::<_, Error>(groups)
        })?;
    let mut checks = Vec::new();

    for attr in &field.attrs {
        if attr.path().is_ident("is_optional") || attr.path().is_ident("groups") {
            continue;
        }

        if attr.path().is_ident("is_email") {
            checks.push(quote! {
                if !#field_value_access.contains('@') {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("is_email", "must be a valid email"),
                    );
                }
            });
        } else if attr.path().is_ident("is_string") {
            ensure_string_type(field_ty, attr)?;
        } else if attr.path().is_ident("is_number") {
            ensure_number_type(field_ty, attr)?;
        } else if attr.path().is_ident("is_int") {
            ensure_int_type(field_ty, attr)?;
        } else if attr.path().is_ident("is_boolean") {
            ensure_boolean_type(field_ty, attr)?;
        } else if attr.path().is_ident("is_uuid") {
            ensure_uuid_type(field_ty, attr)?;
            checks.push(quote! {
                if ::uuid::Uuid::parse_str(&#field_value_access).is_err() {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("is_uuid", "must be a valid UUID"),
                    );
                }
            });
        } else if attr.path().is_ident("is_enum") {
            ensure_enum_type(field_ty, attr)?;
            let enum_ty = parse_enum_type(attr)?;
            checks.push(quote! {
                if <#enum_ty as ::nivasa_pipes::ParseEnumTarget>::parse(#field_value_access).is_err() {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("is_enum", "must be a valid enum variant"),
                    );
                }
            });
        } else if attr.path().is_ident("is_url") {
            ensure_url_type(field_ty, attr)?;
            checks.push(quote! {
                if !nivasa_validation::is_url(&#field_value_access) {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("is_url", "must be a valid URL"),
                    );
                }
            });
        } else if attr.path().is_ident("matches") {
            ensure_matches_type(field_ty, attr)?;
            let pattern = parse_matches_pattern(attr)?;
            let pattern_lit = LitStr::new(&pattern, attr.span());
            checks.push(quote! {
                if !nivasa_validation::matches_regex(&#field_value_access, #pattern_lit) {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("matches", "must match the required pattern"),
                    );
                }
            });
        } else if attr.path().is_ident("is_not_empty") {
            ensure_not_empty_type(field_ty, attr)?;
            checks.push(quote! {
                if !nivasa_validation::is_not_empty(::core::ops::Deref::deref(#field_value_access)) {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("is_not_empty", "must not be empty"),
                    );
                }
            });
        } else if attr.path().is_ident("custom_validate") {
            let validator_path = parse_custom_validate_path(attr)?;
            checks.push(quote! {
                let __nivasa_custom_validate: fn(&#field_ty) -> bool = #validator_path;
                if !__nivasa_custom_validate(#field_value_access) {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                        .with_constraint("custom_validate", "failed custom validation"),
                    );
                }
            });
        } else if attr.path().is_ident("array_min_size") {
            ensure_array_size_type(field_ty, attr, "array_min_size")?;
            let min_size = parse_array_min_size(attr)?;
            let min_size_lit = LitInt::new(&min_size.to_string(), attr.span());
            let message = LitStr::new(
                &format!("must contain at least {} items", min_size),
                attr.span(),
            );

            checks.push(quote! {
                if #field_value_access.len() < #min_size_lit {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("array_min_size", #message),
                    );
                }
            });
        } else if attr.path().is_ident("array_max_size") {
            ensure_array_size_type(field_ty, attr, "array_max_size")?;
            let max_size = parse_array_max_size(attr)?;
            let max_size_lit = LitInt::new(&max_size.to_string(), attr.span());
            let message = LitStr::new(
                &format!("must contain at most {} items", max_size),
                attr.span(),
            );

            checks.push(quote! {
                if #field_value_access.len() > #max_size_lit {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("array_max_size", #message),
                    );
                }
            });
        } else if attr.path().is_ident("validate_nested") {
            if is_optional {
                checks.push(build_nested_validation_check_with_access(
                    field,
                    attr,
                    &field_value_access,
                    &quote!(context),
                )?);
            } else {
                checks.push(build_nested_validation_check(field, attr, &quote!(context))?);
            }
        } else if attr.path().is_ident("min_length") {
            let min_length = parse_min_length(attr)?;
            let min_length_lit = LitInt::new(&min_length.to_string(), attr.span());
            let message = LitStr::new(
                &format!("must be at least {} characters", min_length),
                attr.span(),
            );

            checks.push(quote! {
                if #field_value_access.len() < #min_length_lit {
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
                if #field_value_access.len() > #max_length_lit {
                    errors.push(
                        nivasa_validation::ValidationError::new(#field_label)
                            .with_constraint("max_length", #message),
                    );
                }
            });
        }
    }

    if checks.is_empty() {
        return Ok(Vec::new());
    }

    let field_scope = if is_optional || mode.requires_option_fields() {
        quote! {
            if let Some(#field_value_ident) = &self.#field_name {
                #(#checks)*
            }
        }
    } else {
        quote! {
            let #field_value_ident = &self.#field_name;
            #(#checks)*
        }
    };

    if field_groups.is_empty() {
        Ok(vec![field_scope])
    } else {
        let groups_guard = build_groups_guard(&field_groups);
        Ok(vec![quote! {
            if #groups_guard {
                #field_scope
            }
        }])
    }
}

fn build_nested_validation_check(
    field: &Field,
    attr: &Attribute,
    context_access: &proc_macro2::TokenStream,
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

    if is_option_like_type(&field.ty) {
        if let Some(inner) = option_inner_type(&field.ty) {
            if is_vec_like_type(inner) {
                return build_nested_validation_check_for_vec_option(
                    field_label,
                    field_access,
                    context_access.clone(),
                );
            }
        }

        return build_nested_validation_check_from_option(
            field_label,
            field_access,
            context_access.clone(),
        );
    }

    if is_vec_like_type(&field.ty) {
        return build_nested_validation_check_for_vec(
            field_label,
            field_access,
            context_access.clone(),
        );
    }

    build_nested_validation_check_direct(field_label, field_access, context_access.clone())
}

fn build_nested_validation_check_with_access(
    field: &Field,
    attr: &Attribute,
    field_access: &proc_macro2::TokenStream,
    context_access: &proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    if !matches!(&attr.meta, Meta::Path(_)) {
        return Err(Error::new(
            attr.span(),
            "expected bare `#[validate_nested]`",
        ));
    }

    build_nested_validation_check_direct(
        LitStr::new(&field.ident.as_ref().unwrap().to_string(), field.ident.as_ref().unwrap().span()),
        field_access.clone(),
        context_access.clone(),
    )
}

fn build_nested_validation_check_from_option(
    field_label: LitStr,
    field_access: proc_macro2::TokenStream,
    context_access: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
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

    Ok(quote! {
        if let Some(child) = &#field_access {
            match nivasa_validation::Validate::validate_with(child, #context_access) {
                Ok(()) => {}
                Err(child_errors) => {
                    #child_errors_push
                }
            }
        }
    })
}

fn build_nested_validation_check_for_vec_option(
    field_label: LitStr,
    field_access: proc_macro2::TokenStream,
    context_access: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    let child_errors_push = quote! {
        for (index, child) in child.iter().enumerate() {
            match nivasa_validation::Validate::validate_with(child, #context_access) {
                Ok(()) => {}
                Err(child_errors) => {
                    for mut child_error in child_errors.into_errors() {
                        if child_error.field.is_empty() {
                            child_error.field = ::std::format!("{}[{}]", #field_label, index);
                        } else {
                            child_error.field =
                                ::std::format!("{}[{}].{}", #field_label, index, child_error.field);
                        }

                        errors.push(child_error);
                    }
                }
            }
        }
    };

    Ok(quote! {
        if let Some(child) = &#field_access {
            #child_errors_push
        }
    })
}

fn build_nested_validation_check_for_vec(
    field_label: LitStr,
    field_access: proc_macro2::TokenStream,
    context_access: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    Ok(quote! {
        for (index, child) in #field_access.iter().enumerate() {
            match nivasa_validation::Validate::validate_with(child, #context_access) {
                Ok(()) => {}
                Err(child_errors) => {
                    for mut child_error in child_errors.into_errors() {
                        if child_error.field.is_empty() {
                            child_error.field = ::std::format!("{}[{}]", #field_label, index);
                        } else {
                            child_error.field =
                                ::std::format!("{}[{}].{}", #field_label, index, child_error.field);
                        }

                        errors.push(child_error);
                    }
                }
            }
        }
    })
}

fn build_nested_validation_check_direct(
    field_label: LitStr,
    field_access: proc_macro2::TokenStream,
    context_access: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
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

    Ok(quote! {
        match nivasa_validation::Validate::validate_with(&#field_access, #context_access) {
            Ok(()) => {}
            Err(child_errors) => {
                #child_errors_push
            }
        }
    })
}

fn ensure_string_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_string_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a string field for `#[is_string]`",
        ))
    }
}

fn ensure_boolean_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_boolean_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a bool field for `#[is_boolean]`",
        ))
    }
}

fn ensure_uuid_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_string_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a string field for `#[is_uuid]`",
        ))
    }
}

fn ensure_url_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_string_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a string field for `#[is_url]`",
        ))
    }
}

fn ensure_enum_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_string_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a string field for `#[is_enum]`",
        ))
    }
}

fn ensure_matches_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_string_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a string field for `#[matches]`",
        ))
    }
}

fn ensure_not_empty_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_string_like_type(ty) || is_vec_like_type(ty) || is_slice_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a string, slice, or vec-like field for `#[is_not_empty]`",
        ))
    }
}

fn ensure_array_size_type(ty: &Type, attr: &Attribute, rule_name: &str) -> Result<()> {
    if is_vec_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            format!("expected a vec field for `#[{}]`", rule_name),
        ))
    }
}

fn ensure_number_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_number_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected a numeric field for `#[is_number]`",
        ))
    }
}

fn ensure_int_type(ty: &Type, attr: &Attribute) -> Result<()> {
    if is_int_like_type(ty) {
        Ok(())
    } else {
        Err(Error::new(
            attr.span(),
            "expected an integer field for `#[is_int]`",
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

fn is_int_like_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| is_integer_primitive(&segment.ident))
            .unwrap_or(false),
        Type::Reference(reference) => is_int_like_type(reference.elem.as_ref()),
        Type::Group(group) => is_int_like_type(group.elem.as_ref()),
        Type::Paren(paren) => is_int_like_type(paren.elem.as_ref()),
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

fn is_integer_primitive(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
            | "u128" | "usize"
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

fn is_vec_like_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident == "Vec")
            .unwrap_or(false),
        Type::Reference(reference) => is_vec_like_type(reference.elem.as_ref()),
        Type::Group(group) => is_vec_like_type(group.elem.as_ref()),
        Type::Paren(paren) => is_vec_like_type(paren.elem.as_ref()),
        _ => false,
    }
}

fn is_slice_like_type(ty: &Type) -> bool {
    match ty {
        Type::Slice(_) => true,
        Type::Reference(reference) => is_slice_like_type(reference.elem.as_ref()),
        Type::Group(group) => is_slice_like_type(group.elem.as_ref()),
        Type::Paren(paren) => is_slice_like_type(paren.elem.as_ref()),
        _ => false,
    }
}

fn option_inner_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            if segment.ident != "Option" {
                return None;
            }

            match &segment.arguments {
                PathArguments::AngleBracketed(arguments) => arguments.args.iter().find_map(|arg| {
                    if let GenericArgument::Type(inner) = arg {
                        Some(inner)
                    } else {
                        None
                    }
                }),
                _ => None,
            }
        }
        Type::Reference(reference) => option_inner_type(reference.elem.as_ref()),
        Type::Group(group) => option_inner_type(group.elem.as_ref()),
        Type::Paren(paren) => option_inner_type(paren.elem.as_ref()),
        _ => None,
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

fn parse_matches_pattern(attr: &Attribute) -> Result<String> {
    attr.parse_args::<LitStr>()
        .map(|pattern| pattern.value())
        .map_err(|_| Error::new(attr.span(), "expected `#[matches(\"<regex>\")]`"))
}

fn parse_groups(attr: &Attribute) -> Result<Vec<LitStr>> {
    if !matches!(&attr.meta, Meta::List(_)) {
        return Err(Error::new(
            attr.span(),
            "expected `#[groups(\"create\", \"update\")]`",
        ));
    }

    let groups = attr
        .parse_args_with(Punctuated::<LitStr, Token![,]>::parse_terminated)
        .map_err(|_| {
            Error::new(
                attr.span(),
                "expected `#[groups(\"create\", \"update\")]`",
            )
        })?
        .into_iter()
        .collect::<Vec<_>>();

    if groups.is_empty() {
        return Err(Error::new(
            attr.span(),
            "expected `#[groups(\"create\", \"update\")]`",
        ));
    }

    Ok(groups)
}

fn build_groups_guard(groups: &[LitStr]) -> proc_macro2::TokenStream {
    quote! {
        #(context.has_group(#groups))||*
    }
}

fn parse_array_min_size(attr: &Attribute) -> Result<usize> {
    let value = attr
        .parse_args::<LitInt>()
        .map_err(|_| Error::new(attr.span(), "expected `#[array_min_size(<usize>)]`"))?;

    value
        .base10_parse::<usize>()
        .map_err(|_| Error::new(attr.span(), "expected `#[array_min_size(<usize>)]`"))
}

fn parse_array_max_size(attr: &Attribute) -> Result<usize> {
    let value = attr
        .parse_args::<LitInt>()
        .map_err(|_| Error::new(attr.span(), "expected `#[array_max_size(<usize>)]`"))?;

    value
        .base10_parse::<usize>()
        .map_err(|_| Error::new(attr.span(), "expected `#[array_max_size(<usize>)]`"))
}

fn parse_enum_type(attr: &Attribute) -> Result<TypePath> {
    if !matches!(&attr.meta, Meta::List(_)) {
        return Err(Error::new(attr.span(), "expected `#[is_enum(MyEnum)]`"));
    }

    attr.parse_args::<TypePath>()
        .map_err(|_| Error::new(attr.span(), "expected `#[is_enum(MyEnum)]`"))
}

fn parse_custom_validate_path(attr: &Attribute) -> Result<Path> {
    if !matches!(&attr.meta, Meta::List(_)) {
        return Err(Error::new(
            attr.span(),
            "expected `#[custom_validate(path_to_fn)]`",
        ));
    }

    attr.parse_args::<Path>().map_err(|_| {
        Error::new(
            attr.span(),
            "expected `#[custom_validate(path_to_fn)]`",
        )
    })
}
