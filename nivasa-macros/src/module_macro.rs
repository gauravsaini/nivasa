use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct, Meta, MetaList, parse::{Parse, ParseStream}, Token, Expr, Member, Type};
use std::collections::HashMap;

struct ModuleArgs {
    pub properties: HashMap<String, Vec<Type>>,
}

impl Parse for ModuleArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        syn::braced!(content in input);
        
        let mut properties = HashMap::new();
        
        while !content.is_empty() {
            let key: syn::Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            
            let bracketed_content;
            syn::bracketed!(bracketed_content in content);
            
            let types = syn::punctuated::Punctuated::<Type, Token![,]>::parse_terminated(&bracketed_content)?;
            properties.insert(key.to_string(), types.into_iter().collect());
            
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }
        }
        
        Ok(ModuleArgs { properties })
    }
}

pub fn module_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ModuleArgs);
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;

    let imports = args.properties.get("imports").cloned().unwrap_or_default();
    let providers = args.properties.get("providers").cloned().unwrap_or_default();
    let controllers = args.properties.get("controllers").cloned().unwrap_or_default();
    let exports = args.properties.get("exports").cloned().unwrap_or_default();

    let expanded = quote! {
        #input

        #[async_trait::async_trait]
        impl nivasa_core::module::Module for #name {
            fn metadata(&self) -> nivasa_core::module::ModuleMetadata {
                nivasa_core::module::ModuleMetadata {
                    imports: vec![#(std::any::TypeId::of::<#imports>()),*],
                    providers: vec![#(std::any::TypeId::of::<#providers>()),*],
                    controllers: vec![#(std::any::TypeId::of::<#controllers>()),*],
                    exports: vec![#(std::any::TypeId::of::<#exports>()),*],
                }
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
