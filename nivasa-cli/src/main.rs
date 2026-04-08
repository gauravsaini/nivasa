//! Nivasa CLI tool.

mod statechart;

use clap::Parser;
use statechart::{
    collect_statechart_files, diff_statecharts, inspect_statechart, registry_map,
    render_statechart_svg, resolve_statechart_path, statecharts_dir, validate_statechart_file,
    DiagramFormat,
};
use std::fs;
use std::process::Command;

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
