use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct, Fields, Type, PathArguments, GenericArgument};

fn extract_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Arc" || last_segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        // If it's Option<Arc<T>>, we might need to go one level deeper
                        if last_segment.ident == "Option" {
                             if let Some(deeper_ty) = extract_inner_type(inner_ty) {
                                return Some(deeper_ty);
                             }
                        }
                        return Some(inner_ty);
                    }
                }
            }
        }
    }
    None
}

pub fn injectable_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut dependency_types = Vec::new();

    let resolved_fields = match &input.fields {
        Fields::Named(fields) => {
            let field_resolutions = fields.named.iter().map(|f| {
                let field_name = &f.ident;
                let field_type = &f.ty;

                let type_str = quote!(#field_type).to_string();
                
                if type_str.contains("Option") {
                    if let Some(inner) = extract_inner_type(field_type) {
                        dependency_types.push(inner.clone());
                        quote! {
                            #field_name: container.resolve_optional::<#inner>().await?
                        }
                    } else {
                        quote! { #field_name: None }
                    }
                } else if type_str.contains("Lazy") {
                    quote! {
                        #field_name: nivasa_core::di::Lazy::new(std::sync::Arc::new(container.clone()))
                    }
                } else {
                    // Try to extract T from Arc<T>
                    if let Some(inner) = extract_inner_type(field_type) {
                         dependency_types.push(inner.clone());
                         quote! {
                            #field_name: container.resolve::<#inner>().await?
                         }
                    } else {
                        // Fallback: assume it's already the type we want or user is doing something custom
                        dependency_types.push(field_type.clone());
                        quote! {
                            #field_name: container.resolve::<#field_type>().await?
                        }
                    }
                }
            });

            quote! {
                Self {
                    #(#field_resolutions),*
                }
            }
        }
        Fields::Unnamed(fields) => {
            let field_resolutions = fields.unnamed.iter().map(|f| {
                if let Some(inner) = extract_inner_type(&f.ty) {
                    dependency_types.push(inner.clone());
                    quote! {
                        container.resolve::<#inner>().await?
                    }
                } else {
                    dependency_types.push(f.ty.clone());
                    quote! {
                        container.resolve().await?
                    }
                }
            });

            quote! {
                Self(#(#field_resolutions),*)
            }
        }
        Fields::Unit => quote!(Self),
    };

    let expanded = quote! {
        #input

        #[async_trait::async_trait]
        impl #impl_generics nivasa_core::di::provider::Injectable for #name #ty_generics #where_clause {
            async fn build(container: &nivasa_core::di::container::DependencyContainer) -> Result<Self, nivasa_core::di::error::DiError> {
                Ok(#resolved_fields)
            }

            fn dependencies() -> Vec<std::any::TypeId> {
                vec![#(std::any::TypeId::of::<#dependency_types>()),*]
            }
        }
    };

    TokenStream::from(expanded)
}
