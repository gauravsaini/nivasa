//! Nivasa CLI tool.

use clap::Parser;

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
    let cli = Cli::parse();
    match cli.command {
        Commands::Info => {
            println!("Nivasa Framework v{}", env!("CARGO_PKG_VERSION"));
        }
        Commands::Statechart { action } => match action {
            StatechartAction::Validate { all, file } => {
                println!("Validating SCXML files...");
                // TODO: implement
            }
            StatechartAction::Parity => {
                println!("Checking SCXML-code parity...");
                // TODO: implement
            }
        },
    }
}
