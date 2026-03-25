use proc_macro::TokenStream;
use quote::quote;
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Error, Item, LitStr, Result, Token};

#[derive(Debug, Default)]
struct ScxmlHandlerArgs {
    statechart: Option<LitStr>,
    state: Option<LitStr>,
}

impl Parse for ScxmlHandlerArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut args = ScxmlHandlerArgs::default();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "statechart" => {
                    if args.statechart.is_some() {
                        return Err(Error::new(key.span(), "duplicate `statechart` argument"));
                    }
                    args.statechart = Some(value);
                }
                "state" => {
                    if args.state.is_some() {
                        return Err(Error::new(key.span(), "duplicate `state` argument"));
                    }
                    args.state = Some(value);
                }
                other => {
                    return Err(Error::new(
                        key.span(),
                        format!(
                            "unknown `#[scxml_handler]` argument `{other}`; expected `statechart` and `state`"
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

pub fn scxml_handler_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ScxmlHandlerArgs);
    let input: Item = match syn::parse(item.clone()) {
        Ok(item) => item,
        Err(err) => return err.to_compile_error().into(),
    };

    match validate_scxml_handler(&args) {
        Ok(()) => quote!(#input).into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn validate_scxml_handler(args: &ScxmlHandlerArgs) -> Result<()> {
    let statechart = args
        .statechart
        .as_ref()
        .ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required `statechart` argument"))?;
    let state = args
        .state
        .as_ref()
        .ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required `state` argument"))?;

    let statechart_name = statechart.value();
    let state_name = state.value();

    validate_statechart_name(&statechart_name, statechart)?;
    validate_state_name(&state_name, state)?;

    let (scxml_path, source) = load_scxml_source(&statechart_name, statechart)?;
    let states = extract_state_ids(&source).map_err(|err| {
        Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "failed to parse SCXML file `{}`: {err}",
                scxml_path.display()
            ),
        )
    })?;

    if !states.contains(&state_name) {
        let available = if states.is_empty() {
            "none".to_string()
        } else {
            states.into_iter().collect::<Vec<_>>().join(", ")
        };
        return Err(Error::new(
            state.span(),
            format!(
                "state `{state_name}` was not found in `{}`; available states: {available}",
                scxml_path.display()
            ),
        ));
    }

    Ok(())
}

fn validate_statechart_name(value: &str, span: &LitStr) -> Result<()> {
    if value.is_empty() {
        return Err(Error::new(span.span(), "`statechart` cannot be empty"));
    }

    if value.contains('/') || value.contains('\\') || value.contains("..") || value.contains('.') {
        return Err(Error::new(
            span.span(),
            "`statechart` must be a bare SCXML name like `request`, not a path or dotted value",
        ));
    }

    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(Error::new(
            span.span(),
            "`statechart` may only contain ASCII letters, digits, `_`, or `-`",
        ));
    }

    Ok(())
}

fn validate_state_name(value: &str, span: &LitStr) -> Result<()> {
    if value.is_empty() {
        return Err(Error::new(span.span(), "`state` cannot be empty"));
    }

    if value.contains('/') || value.contains('\\') || value.contains("..") {
        return Err(Error::new(
            span.span(),
            "`state` must be a single SCXML state identifier, not a path",
        ));
    }

    Ok(())
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("nivasa-macros must live inside the workspace")
        .to_path_buf()
}

fn load_scxml_source(statechart_name: &str, span: &LitStr) -> Result<(PathBuf, String)> {
    let root = workspace_root();
    let scxml_path = root
        .join("statecharts")
        .join(format!("nivasa.{statechart_name}.scxml"));

    if !scxml_path.exists() {
        return Err(Error::new(
            span.span(),
            format!(
                "SCXML file not found for `statechart = \"{statechart_name}\"`; expected `{}`",
                scxml_path.display()
            ),
        ));
    }

    let canonical_root = fs::canonicalize(&root).unwrap_or(root);
    let canonical_path = fs::canonicalize(&scxml_path).unwrap_or(scxml_path.clone());

    if !canonical_path.starts_with(&canonical_root) {
        return Err(Error::new(
            span.span(),
            format!(
                "resolved SCXML path `{}` escapes the workspace root",
                canonical_path.display()
            ),
        ));
    }

    let source = fs::read_to_string(&canonical_path).map_err(|err| {
        Error::new(
            span.span(),
            format!("failed to read `{}`: {err}", canonical_path.display()),
        )
    })?;

    Ok((canonical_path, source))
}

fn extract_state_ids(source: &str) -> Result<BTreeSet<String>> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut states = BTreeSet::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(ref e)) | Ok(XmlEvent::Empty(ref e)) => {
                let name = e.name();
                let tag_name = std::str::from_utf8(name.as_ref())
                    .map_err(|err| Error::new(proc_macro2::Span::call_site(), err.to_string()))?;

                if matches!(tag_name, "state" | "parallel" | "final") {
                    if let Some(id) = read_attr(e, "id")? {
                        states.insert(id);
                    }
                }
            }
            Ok(XmlEvent::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(Error::new(
                    proc_macro2::Span::call_site(),
                    format!("XML parse error: {err}"),
                ));
            }
        }
        buf.clear();
    }

    Ok(states)
}

fn read_attr(e: &quick_xml::events::BytesStart, name: &str) -> Result<Option<String>> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| Error::new(proc_macro2::Span::call_site(), err.to_string()))?;
        if attr.key.as_ref() == name.as_bytes() {
            let value = attr
                .unescape_value()
                .map_err(|err| Error::new(proc_macro2::Span::call_site(), err.to_string()))?;
            return Ok(Some(value.to_string()));
        }
    }

    Ok(None)
}
