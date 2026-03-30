use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input,
    spanned::Spanned,
    Attribute, Data, DeriveInput, Error, Field, Fields, LitInt, LitStr, Result,
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
        }
    }

    Ok(checks)
}

fn parse_min_length(attr: &Attribute) -> Result<usize> {
    let value = attr
        .parse_args::<LitInt>()
        .map_err(|_| Error::new(attr.span(), "expected `#[min_length(<usize>)]`"))?;

    value
        .base10_parse::<usize>()
        .map_err(|_| Error::new(attr.span(), "expected `#[min_length(<usize>)]`"))
}
