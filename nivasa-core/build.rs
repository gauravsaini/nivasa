use nivasa_statechart::parser::ScxmlDocument;
use nivasa_statechart::{validate_scxml_schema, validator};
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
    let statecharts_dir = statecharts_dir(&manifest_dir);

    let scxml_files = [
        ("nivasa.module.scxml", "module_scxml.rs"),
        ("nivasa.provider.scxml", "provider_scxml.rs"),
    ];

    for (src, dest) in scxml_files {
        let scxml_path = statecharts_dir.join(src);
        println!("cargo:rerun-if-changed={}", scxml_path.display());

        validate_scxml_schema(&scxml_path).unwrap_or_else(|err| {
            panic!(
                "schema validation failed for {}: {}",
                scxml_path.display(),
                err
            )
        });

        let scxml_content = fs::read_to_string(&scxml_path)
            .unwrap_or_else(|_| panic!("Could not read SCXML file: {}", src));

        let scxml = ScxmlDocument::from_str(&scxml_content)
            .unwrap_or_else(|_| panic!("Could not parse SCXML file: {}", src));
        let validation = validator::validate(&scxml);
        if !validation.errors.is_empty() {
            let mut message = format!("SCXML validation failed for {}:\n", scxml_path.display());
            for error in validation.errors {
                let _ = writeln!(message, "  - {}", error.message);
            }
            panic!("{message}");
        }

        for warning in validation.warnings {
            println!("cargo:warning={} ({:?})", warning.message, warning.rule);
        }

        let rust_code = nivasa_statechart::codegen::generate_rust(&scxml);

        let dest_path = Path::new(&out_dir).join(dest);
        fs::write(dest_path, rust_code)
            .unwrap_or_else(|_| panic!("Could not write generated code to: {}", dest));
    }
}

fn statecharts_dir(manifest_dir: &Path) -> PathBuf {
    let workspace_statecharts = manifest_dir.join("../statecharts");
    if workspace_statecharts.is_dir() {
        workspace_statecharts
    } else {
        manifest_dir.join("statecharts")
    }
}
