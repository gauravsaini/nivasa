//! Nivasa CLI tool.

mod statechart;

use clap::Parser;
use statechart::{
    collect_statechart_files, diff_statecharts, inspect_statechart, registry_map,
    render_statechart_svg, resolve_statechart_path, statecharts_dir, validate_statechart_file,
    DiagramFormat,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_APP_STATECHART: &str = include_str!("../../statecharts/nivasa.application.scxml");
const DEFAULT_MODULE_STATECHART: &str = include_str!("../../statecharts/nivasa.module.scxml");
const DEFAULT_PROVIDER_STATECHART: &str = include_str!("../../statecharts/nivasa.provider.scxml");
const DEFAULT_REQUEST_STATECHART: &str = include_str!("../../statecharts/nivasa.request.scxml");

#[derive(Parser)]
#[command(name = "nivasa", about = "CLI tool for the Nivasa framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Display framework info
    Info,
    /// Scaffold a new Nivasa project
    New {
        /// Project directory and crate name
        project_name: String,
    },
    /// Generate framework files
    #[command(visible_alias = "g")]
    Generate {
        #[command(subcommand)]
        action: GenerateAction,
    },
    /// Statechart operations
    Statechart {
        #[command(subcommand)]
        action: StatechartAction,
    },
}

#[derive(clap::Subcommand)]
enum GenerateAction {
    /// Generate a module file
    Module {
        /// Module name
        name: String,
    },
    /// Generate a controller file
    Controller {
        /// Controller name
        name: String,
    },
    /// Generate a service file
    Service {
        /// Service name
        name: String,
    },
}

#[derive(clap::Subcommand)]
enum StatechartAction {
    /// Validate all SCXML files
    Validate {
        /// Validate all files, or specify a single file
        #[arg(long)]
        all: bool,
        /// Path to a specific SCXML file
        file: Option<String>,
    },
    /// Check generated code matches SCXML
    Parity,
    /// Render SCXML diagrams
    Visualize {
        /// Diagram format
        #[arg(long, value_enum, default_value_t = DiagramFormat::Svg)]
        format: DiagramFormat,
        /// Render a specific SCXML file
        file: Option<String>,
    },
    /// Show SCXML changes between commits
    Diff {
        /// Git revision to compare against
        #[arg(default_value = "HEAD~1")]
        rev: String,
    },
    /// Inspect a running app's debug statechart endpoint
    Inspect {
        /// Hostname of the running app
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port of the running app
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Info => info_command(),
        Commands::New { project_name } => new_command(&project_name),
        Commands::Generate { action } => match action {
            GenerateAction::Module { name } => generate_module_command(&name),
            GenerateAction::Controller { name } => generate_controller_command(&name),
            GenerateAction::Service { name } => generate_service_command(&name),
        },
        Commands::Statechart { action } => match action {
            StatechartAction::Validate { all, file } => validate_command(all, file),
            StatechartAction::Parity => parity_command(),
            StatechartAction::Visualize { format, file } => visualize_command(format, file),
            StatechartAction::Diff { rev } => diff_command(&rev),
            StatechartAction::Inspect { host, port } => inspect_command(&host, port),
        },
    }
}

fn info_command() -> Result<(), String> {
    let rust_version = Command::new("rustc")
        .arg("--version")
        .output()
        .map_err(|err| format!("failed to run rustc --version: {err}"))?;
    if !rust_version.status.success() {
        return Err("rustc --version exited unsuccessfully".to_string());
    }

    let rust_version = String::from_utf8(rust_version.stdout)
        .map_err(|err| format!("rustc --version returned non-utf8 output: {err}"))?;

    println!("Nivasa Framework v{}", env!("CARGO_PKG_VERSION"));
    println!("Rust {}", rust_version.trim());
    println!("OS {} {}", std::env::consts::OS, std::env::consts::ARCH);
    Ok(())
}

fn new_command(project_name: &str) -> Result<(), String> {
    scaffold_new_project(&std::env::current_dir().map_err(|err| err.to_string())?, project_name)?;
    println!("created {}", project_name);
    Ok(())
}

fn generate_module_command(name: &str) -> Result<(), String> {
    let path = generate_module(&std::env::current_dir().map_err(|err| err.to_string())?, name)?;
    println!("created {}", path.display());
    Ok(())
}

fn generate_controller_command(name: &str) -> Result<(), String> {
    let path =
        generate_controller(&std::env::current_dir().map_err(|err| err.to_string())?, name)?;
    println!("created {}", path.display());
    Ok(())
}

fn generate_service_command(name: &str) -> Result<(), String> {
    let path = generate_service(&std::env::current_dir().map_err(|err| err.to_string())?, name)?;
    println!("created {}", path.display());
    Ok(())
}

fn validate_command(all: bool, file: Option<String>) -> Result<(), String> {
    if all && file.is_some() {
        return Err("use either `--all` or a single file path, not both".to_string());
    }

    let files = if all || file.is_none() {
        collect_statechart_files(&statecharts_dir())?
    } else {
        vec![resolve_statechart_path(
            &statecharts_dir(),
            file.as_ref().unwrap(),
        )?]
    };

    for path in files {
        println!("{}", validate_statechart_file(&path)?);
    }

    Ok(())
}

fn parity_command() -> Result<(), String> {
    let statecharts_dir = statecharts_dir();
    let files = collect_statechart_files(&statecharts_dir)?;
    let compiled = registry_map();
    let mut seen = std::collections::HashSet::new();

    for path in files {
        let source_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid SCXML file name: {}", path.display()))?;
        let source = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
        let document = nivasa_statechart::ScxmlDocument::from_str(&source)
            .map_err(|err| format!("failed to parse {}: {}", path.display(), err))?;

        let generated = nivasa_statechart::codegen::generate_rust(&document);
        let compiled_entry = compiled
            .get(source_name)
            .ok_or_else(|| format!("missing compiled SCXML artifact for {}", source_name))?;

        if compiled_entry.scxml_hash != document.content_hash() {
            return Err(format!(
                "parity mismatch for {}: compiled hash {} does not match source hash {}",
                source_name,
                compiled_entry.scxml_hash,
                document.content_hash()
            ));
        }

        if !generated.contains(compiled_entry.scxml_hash) {
            return Err(format!(
                "generated Rust for {} does not embed the expected SCXML hash",
                source_name
            ));
        }

        println!("{}: parity ok", source_name);
        seen.insert(source_name.to_string());
    }

    for source_name in compiled.keys() {
        if !seen.contains(*source_name) {
            return Err(format!(
                "compiled SCXML registry contains {} but the source file was not found",
                source_name
            ));
        }
    }

    Ok(())
}

fn visualize_command(format: DiagramFormat, file: Option<String>) -> Result<(), String> {
    match format {
        DiagramFormat::Svg => {
            let outputs = if let Some(file) = file {
                let path = resolve_statechart_path(&statecharts_dir(), &file)?;
                let svg = render_statechart_svg(&path)?;
                vec![(path, svg)]
            } else {
                collect_statechart_files(&statecharts_dir())?
                    .into_iter()
                    .map(|path| render_statechart_svg(&path).map(|svg| (path, svg)))
                    .collect::<Result<Vec<_>, _>>()?
            };

            for (index, (path, svg)) in outputs.into_iter().enumerate() {
                if index > 0 {
                    println!();
                }
                println!("<!-- {} -->", path.display());
                print!("{svg}");
            }
            Ok(())
        }
    }
}

fn diff_command(rev: &str) -> Result<(), String> {
    let diff = diff_statecharts(rev)?;
    println!("{diff}");
    Ok(())
}

fn inspect_command(host: &str, port: u16) -> Result<(), String> {
    let report = inspect_statechart(host, port)?;
    println!("{report}");
    Ok(())
}

fn scaffold_new_project(base_dir: &Path, project_name: &str) -> Result<(), String> {
    let project_name = project_name.trim();
    if project_name.is_empty() {
        return Err("project name cannot be empty".to_string());
    }

    let project_dir = base_dir.join(project_name);
    if project_dir.exists() {
        return Err(format!(
            "project directory already exists: {}",
            project_dir.display()
        ));
    }

    fs::create_dir_all(project_dir.join("src"))
        .map_err(|err| format!("failed to create src directory: {err}"))?;
    fs::create_dir_all(project_dir.join("statecharts"))
        .map_err(|err| format!("failed to create statecharts directory: {err}"))?;

    write_project_file(
        &project_dir.join("Cargo.toml"),
        &new_project_cargo_toml(project_name),
    )?;
    write_project_file(&project_dir.join(".env"), "PORT=3000\n")?;
    write_project_file(&project_dir.join(".gitignore"), "/target\n.env.local\n")?;
    write_project_file(&project_dir.join("src/main.rs"), &new_project_main_rs())?;
    write_project_file(
        &project_dir.join("src/app_module.rs"),
        &new_project_app_module_rs(),
    )?;
    write_project_file(
        &project_dir.join("statecharts/nivasa.application.scxml"),
        DEFAULT_APP_STATECHART,
    )?;
    write_project_file(
        &project_dir.join("statecharts/nivasa.module.scxml"),
        DEFAULT_MODULE_STATECHART,
    )?;
    write_project_file(
        &project_dir.join("statecharts/nivasa.provider.scxml"),
        DEFAULT_PROVIDER_STATECHART,
    )?;
    write_project_file(
        &project_dir.join("statecharts/nivasa.request.scxml"),
        DEFAULT_REQUEST_STATECHART,
    )?;

    Ok(())
}

fn generate_module(base_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let module_name = normalize_generator_name(name)?;
    let module_dir = base_dir.join(&module_name);
    fs::create_dir_all(&module_dir)
        .map_err(|err| format!("failed to create module directory: {err}"))?;

    let file_path = module_dir.join(format!("{module_name}_module.rs"));
    if file_path.exists() {
        return Err(format!("module file already exists: {}", file_path.display()));
    }

    let struct_name = to_pascal_case(&module_name);
    write_project_file(
        &file_path,
        &new_module_template(&module_name, &struct_name),
    )?;

    Ok(file_path)
}

fn generate_controller(base_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let controller_name = normalize_generator_name(name)?;
    let controller_dir = base_dir.join(&controller_name);
    fs::create_dir_all(&controller_dir)
        .map_err(|err| format!("failed to create controller directory: {err}"))?;

    let file_path = controller_dir.join(format!("{controller_name}_controller.rs"));
    if file_path.exists() {
        return Err(format!(
            "controller file already exists: {}",
            file_path.display()
        ));
    }

    let struct_name = to_pascal_case(&controller_name);
    write_project_file(
        &file_path,
        &new_controller_template(&controller_name, &struct_name),
    )?;

    Ok(file_path)
}

fn generate_service(base_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let service_name = normalize_generator_name(name)?;
    let service_dir = base_dir.join(&service_name);
    fs::create_dir_all(&service_dir)
        .map_err(|err| format!("failed to create service directory: {err}"))?;

    let file_path = service_dir.join(format!("{service_name}_service.rs"));
    if file_path.exists() {
        return Err(format!("service file already exists: {}", file_path.display()));
    }

    let struct_name = to_pascal_case(&service_name);
    write_project_file(
        &file_path,
        &new_service_template(&service_name, &struct_name),
    )?;

    Ok(file_path)
}

fn write_project_file(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn new_project_cargo_toml(project_name: &str) -> String {
    format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
nivasa = {{ path = "../nivasa", features = ["config"] }}
"#
    )
}

fn new_project_main_rs() -> String {
    r#"mod app_module;

use app_module::AppModule;
use nivasa::prelude::*;

fn main() {
    let _app = NestApplication::create(AppModule);
}
"#
    .to_string()
}

fn new_project_app_module_rs() -> String {
    r#"use nivasa::prelude::*;

#[module({})]
pub struct AppModule;
"#
    .to_string()
}

fn new_module_template(module_name: &str, struct_name: &str) -> String {
    format!(
        r#"use nivasa::prelude::*;

#[module({{}})]
pub struct {struct_name}Module;

impl {struct_name}Module {{
    pub const PATH: &'static str = "{module_name}";
}}
"#
    )
}

fn new_controller_template(controller_name: &str, struct_name: &str) -> String {
    format!(
        r#"use nivasa::prelude::*;

#[controller("/{controller_name}")]
pub struct {struct_name}Controller;

#[impl_controller]
impl {struct_name}Controller {{
    #[get("/")]
    pub fn list(&self) -> &'static str {{
        "{controller_name}"
    }}
}}
"#
    )
}

fn new_service_template(service_name: &str, struct_name: &str) -> String {
    format!(
        r#"use nivasa::prelude::*;

#[injectable]
pub struct {struct_name}Service;

impl {struct_name}Service {{
    pub const NAME: &'static str = "{service_name}";
}}
"#
    )
}

fn normalize_generator_name(name: &str) -> Result<String, String> {
    let normalized = name
        .trim()
        .chars()
        .map(|ch| match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' => ch,
            '-' | '_' | ' ' => '_',
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if normalized.is_empty() {
        return Err("name cannot be empty".to_string());
    }

    Ok(normalized)
}

fn to_pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    first.to_ascii_uppercase().to_string() + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        generate_controller, generate_module, generate_service, new_controller_template,
        new_module_template, new_project_app_module_rs, new_project_cargo_toml,
        new_project_main_rs, new_service_template, normalize_generator_name,
        scaffold_new_project, to_pascal_case,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scaffold_new_project_creates_expected_structure() {
        let root = temp_dir("new-project");
        scaffold_new_project(&root, "myapp").expect("project scaffold should succeed");

        let project_dir = root.join("myapp");
        assert!(project_dir.join("Cargo.toml").is_file());
        assert!(project_dir.join(".env").is_file());
        assert!(project_dir.join(".gitignore").is_file());
        assert!(project_dir.join("src/main.rs").is_file());
        assert!(project_dir.join("src/app_module.rs").is_file());
        assert!(project_dir.join("statecharts").is_dir());
        assert!(project_dir
            .join("statecharts/nivasa.application.scxml")
            .is_file());
        assert!(project_dir.join("statecharts/nivasa.module.scxml").is_file());
        assert!(project_dir
            .join("statecharts/nivasa.provider.scxml")
            .is_file());
        assert!(project_dir.join("statecharts/nivasa.request.scxml").is_file());

        assert_eq!(
            fs::read_to_string(project_dir.join("Cargo.toml")).unwrap(),
            new_project_cargo_toml("myapp")
        );
        assert_eq!(
            fs::read_to_string(project_dir.join("src/main.rs")).unwrap(),
            new_project_main_rs()
        );
        assert_eq!(
            fs::read_to_string(project_dir.join("src/app_module.rs")).unwrap(),
            new_project_app_module_rs()
        );
    }

    #[test]
    fn scaffold_new_project_rejects_existing_directory() {
        let root = temp_dir("new-project-existing");
        fs::create_dir_all(root.join("myapp")).unwrap();

        let error = scaffold_new_project(&root, "myapp").unwrap_err();
        assert!(error.contains("project directory already exists"));
    }

    #[test]
    fn generate_module_creates_expected_file() {
        let root = temp_dir("generate-module");
        let file_path = generate_module(&root, "users").expect("module generation should succeed");

        assert_eq!(file_path, root.join("users/users_module.rs"));
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            new_module_template("users", "Users")
        );
    }

    #[test]
    fn normalize_generator_name_and_pascal_case_work() {
        assert_eq!(normalize_generator_name(" User Profile ").unwrap(), "user_profile");
        assert_eq!(to_pascal_case("user_profile"), "UserProfile");
    }

    #[test]
    fn generate_controller_creates_expected_file() {
        let root = temp_dir("generate-controller");
        let file_path =
            generate_controller(&root, "users").expect("controller generation should succeed");

        assert_eq!(file_path, root.join("users/users_controller.rs"));
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            new_controller_template("users", "Users")
        );
    }

    #[test]
    fn generate_service_creates_expected_file() {
        let root = temp_dir("generate-service");
        let file_path =
            generate_service(&root, "users").expect("service generation should succeed");

        assert_eq!(file_path, root.join("users/users_service.rs"));
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            new_service_template("users", "Users")
        );
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("nivasa-cli-{prefix}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
