use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, spanned::Spanned, Attribute, Data, DeriveInput, Error, Fields, LitStr,
    Result,
};

pub fn config_schema_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_config_schema_derive(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Default)]
struct SchemaFieldArgs {
    default: Option<LitStr>,
}

fn expand_config_schema_derive(input: &DeriveInput) -> Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
            Fields::Unnamed(_) | Fields::Unit => {
                return Err(Error::new(
                    input.span(),
                    "`#[derive(ConfigSchema)]` only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(Error::new(
                input.span(),
                "`#[derive(ConfigSchema)]` only supports structs",
            ));
        }
    };

    let mut required_keys = Vec::new();
    let mut defaults = Vec::new();

    for field in fields {
        let field_name = field
            .ident
            .as_ref()
            .ok_or_else(|| Error::new(field.span(), "config schema only supports named fields"))?;
        let field_key = field_name.to_string();
        let mut schema_args = SchemaFieldArgs::default();
        let mut seen_schema_attr = false;

        for attr in &field.attrs {
            if !attr.path().is_ident("schema") {
                continue;
            }

            if seen_schema_attr {
                return Err(Error::new(
                    attr.span(),
                    "duplicate `#[schema]` attribute on config field",
                ));
            }
            seen_schema_attr = true;
            schema_args = parse_schema_attr(attr)?;
        }

        if let Some(default) = schema_args.default {
            defaults.push(quote!((#field_key, #default)));
        } else {
            required_keys.push(quote!(#field_key));
        }
    }

    Ok(quote! {
        impl #impl_generics ConfigSchema for #name #ty_generics #where_clause {
            fn required_keys() -> &'static [&'static str] {
                &[#(#required_keys),*]
            }

            fn defaults() -> &'static [(&'static str, &'static str)] {
                &[#(#defaults),*]
            }
        }
    })
}

fn parse_schema_attr(attr: &Attribute) -> Result<SchemaFieldArgs> {
    let mut args = SchemaFieldArgs::default();

    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("default") {
            if args.default.is_some() {
                return Err(meta.error("duplicate `default` entry in `#[schema]`"));
            }

            let value: LitStr = meta.value()?.parse()?;
            args.default = Some(value);
            Ok(())
        } else {
            Err(meta.error("unknown `#[schema]` entry; expected `default = \"...\"`"))
        }
    })?;

    Ok(args)
}
