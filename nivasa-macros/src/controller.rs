use proc_macro::TokenStream;
use quote::quote;
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input,
    Error, Ident, ItemStruct, LitStr, Result, Token,
};

#[derive(Debug, Default, Clone)]
struct ControllerArgs {
    path: Option<LitStr>,
    version: Option<LitStr>,
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

pub fn controller_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ControllerArgs);
    let input = parse_macro_input!(item as ItemStruct);

    match expand_controller(args, input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
