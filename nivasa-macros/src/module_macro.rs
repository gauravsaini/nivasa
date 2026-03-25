use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Error, Ident, ItemStruct, Result, Token, Type,
};

#[derive(Default)]
struct ModuleArgs {
    imports: Vec<Type>,
    controllers: Vec<Type>,
    providers: Vec<Type>,
    exports: Vec<Type>,
    middlewares: Vec<Type>,
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

    let imports = args.imports;
    let providers = args.providers;
    let controllers = args.controllers;
    let exports = args.exports;
    let middlewares = args.middlewares;

    let expanded = quote! {
        #input

        impl #name {
            pub fn __nivasa_module_middlewares() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#middlewares>()),*]
            }
        }

        #[async_trait::async_trait]
        impl nivasa_core::module::Module for #name {
            fn metadata(&self) -> nivasa_core::module::ModuleMetadata {
                nivasa_core::module::ModuleMetadata::new()
                    .with_imports(vec![#(std::any::TypeId::of::<#imports>()),*])
                    .with_providers(vec![#(std::any::TypeId::of::<#providers>()),*])
                    .with_controllers(vec![#(std::any::TypeId::of::<#controllers>()),*])
                    .with_exports(vec![#(std::any::TypeId::of::<#exports>()),*])
                    .with_global(false)
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
