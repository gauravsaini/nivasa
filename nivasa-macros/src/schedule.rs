use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, FnArg, ImplItemFn, LitInt, LitStr};

#[allow(dead_code)]
pub fn cron(attr: TokenStream, item: TokenStream) -> TokenStream {
    let expression = match syn::parse::<LitStr>(attr) {
        Ok(expression) if !expression.value().trim().is_empty() => expression,
        Ok(expression) => {
            return Error::new(
                expression.span(),
                "`#[cron]` expects a non-empty cron expression like `#[cron(\"0 */5 * * * *\")]`",
            )
            .to_compile_error()
            .into();
        }
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[cron]` expects a cron expression like `#[cron(\"0 */5 * * * *\")]`",
            )
            .to_compile_error()
            .into();
        }
    };

    let schedule_path = quote! {
        ::nivasa_scheduling::SchedulePattern::cron(#expression)
    };

    expand_schedule(item, "cron", schedule_path).into()
}

pub fn interval(attr: TokenStream, item: TokenStream) -> TokenStream {
    let millis = match syn::parse::<LitInt>(attr) {
        Ok(millis) => millis,
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[interval]` expects a millisecond integer like `#[interval(5000)]`",
            )
            .to_compile_error()
            .into();
        }
    };

    let value = match millis.base10_parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Error::new(
                millis.span(),
                "`#[interval]` expects a millisecond integer like `#[interval(5000)]`",
            )
            .to_compile_error()
            .into();
        }
    };

    if value == 0 {
        return Error::new(
            millis.span(),
            "`#[interval]` expects a positive millisecond value",
        )
        .to_compile_error()
        .into();
    }

    let schedule_path = quote! {
        ::nivasa_scheduling::SchedulePattern::interval(::std::time::Duration::from_millis(#value))
    };

    expand_schedule(item, "interval", schedule_path).into()
}

pub fn timeout(attr: TokenStream, item: TokenStream) -> TokenStream {
    let millis = match syn::parse::<LitInt>(attr) {
        Ok(millis) => millis,
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                "`#[timeout]` expects a millisecond integer like `#[timeout(3000)]`",
            )
            .to_compile_error()
            .into();
        }
    };

    let value = match millis.base10_parse::<u64>() {
        Ok(value) => value,
        Err(_) => {
            return Error::new(
                millis.span(),
                "`#[timeout]` expects a millisecond integer like `#[timeout(3000)]`",
            )
            .to_compile_error()
            .into();
        }
    };

    let schedule_path = quote! {
        ::nivasa_scheduling::SchedulePattern::timeout(::std::time::Duration::from_millis(#value))
    };

    expand_schedule(item, "timeout", schedule_path).into()
}

fn expand_schedule(
    item: TokenStream,
    kind_name: &str,
    schedule_path: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let method = match syn::parse::<ImplItemFn>(item) {
        Ok(method) => method,
        Err(_) => {
            return Error::new(
                proc_macro2::Span::call_site(),
                format!("`#[{kind_name}]` only supports inherent methods"),
            )
            .to_compile_error()
            .into();
        }
    };

    if !matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_))) {
        return Error::new(
            method.sig.ident.span(),
            format!("`#[{kind_name}]` only supports inherent methods"),
        )
        .to_compile_error()
        .into();
    }

    let method_name = &method.sig.ident;
    let helper_name = format_ident!("__nivasa_{}_metadata_for_{}", kind_name, method_name);

    quote! {
        #method

        pub fn #helper_name() -> ::nivasa_scheduling::SchedulePattern {
            #schedule_path
        }
    }
}
