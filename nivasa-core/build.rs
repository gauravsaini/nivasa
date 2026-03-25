use std::env;
use std::fs;
use std::path::Path;
use nivasa_statechart::parser::ScxmlDocument;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let statecharts_dir = Path::new("../statecharts");

    let scxml_files = [
        ("nivasa.application.scxml", "application_scxml.rs"),
        ("nivasa.module.scxml", "module_scxml.rs"),
        ("nivasa.provider.scxml", "provider_scxml.rs"),
        ("nivasa.request.scxml", "request_scxml.rs"),
    ];

    for (src, dest) in scxml_files {
        let scxml_path = statecharts_dir.join(src);
        println!("cargo:rerun-if-changed={}", scxml_path.display());

        let scxml_content = fs::read_to_string(&scxml_path)
            .expect(&format!("Could not read SCXML file: {}", src));

        let scxml = ScxmlDocument::from_str(&scxml_content)
            .expect(&format!("Could not parse SCXML file: {}", src));

        let rust_code = nivasa_statechart::codegen::generate_rust(&scxml);
        
        let dest_path = Path::new(&out_dir).join(dest);
        fs::write(dest_path, rust_code)
            .expect(&format!("Could not write generated code to: {}", dest));
    }
}
