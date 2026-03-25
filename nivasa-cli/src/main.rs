//! Nivasa CLI tool.

use clap::Parser;
use nivasa_statechart::{codegen, validator, ScxmlDocument};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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
    /// Statechart operations
    Statechart {
        #[command(subcommand)]
        action: StatechartAction,
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
        Commands::Info => {
            println!("Nivasa Framework v{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Commands::Statechart { action } => match action {
            StatechartAction::Validate { all, file } => {
                validate_command(all, file)
            }
            StatechartAction::Parity => {
                parity_command()
            }
        },
    }
}

fn validate_command(all: bool, file: Option<String>) -> Result<(), String> {
    if all && file.is_some() {
        return Err("use either `--all` or a single file path, not both".to_string());
    }

    let files = if all || file.is_none() {
        collect_statechart_files(&statecharts_dir())?
    } else {
        vec![resolve_statechart_path(file.as_ref().unwrap())?]
    };

    for path in files {
        validate_single_file(&path)?;
    }

    Ok(())
}

fn parity_command() -> Result<(), String> {
    let statecharts_dir = statecharts_dir();
    let files = collect_statechart_files(&statecharts_dir)?;

    let compiled = registry_map();
    let mut seen = HashSet::new();

    for path in files {
        let source_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid SCXML file name: {}", path.display()))?;
        let source = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
        let document = ScxmlDocument::from_str(&source)
            .map_err(|err| format!("failed to parse {}: {}", path.display(), err))?;

        let generated = codegen::generate_rust(&document);
        let compiled_entry = compiled.get(source_name).ok_or_else(|| {
            format!(
                "missing compiled SCXML artifact for {}",
                source_name
            )
        })?;

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

fn validate_single_file(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
    let document = ScxmlDocument::from_str(&source)
        .map_err(|err| format!("failed to parse {}: {}", path.display(), err))?;
    let result = validator::validate(&document);

    for warning in &result.warnings {
        println!("warning: {}: {}", path.display(), warning.message);
    }

    if !result.errors.is_empty() {
        for error in &result.errors {
            println!("error: {}: {}", path.display(), error.message);
        }
        return Err(format!("{}: validation failed", path.display()));
    }

    println!("{}: valid", path.display());
    Ok(())
}

fn collect_statechart_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("failed to read {}: {}", dir.display(), err))?;

    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read {}: {}", dir.display(), err))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("scxml") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn resolve_statechart_path(file: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(file);
    if candidate.exists() {
        return Ok(candidate);
    }

    let from_statecharts = statecharts_dir().join(file);
    if from_statecharts.exists() {
        Ok(from_statecharts)
    } else {
        Err(format!("statechart file not found: {file}"))
    }
}

fn statecharts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../statecharts")
}

fn registry_map() -> HashMap<&'static str, &'static nivasa_statechart::GeneratedStatechart> {
    let mut map = HashMap::new();
    for entry in nivasa_statechart::GENERATED_STATECHARTS {
        map.insert(entry.file_name, entry);
    }
    map
}
