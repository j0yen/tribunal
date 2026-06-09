use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "tribunal", about = "Ethics corpus validator and ontology conformance judge")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage the ethics corpus
    Corpus {
        #[command(subcommand)]
        action: CorpusAction,
    },
    /// Verify structural conformance claims of an OWL ontology
    Conformance {
        /// Path to the OWL/XML ontology file to check
        #[arg(long)]
        owl: PathBuf,
        /// Path to the axioms.toml manifest (§8.3 ten-axiom table)
        #[arg(long, default_value = "fixtures/axioms.toml")]
        axioms: PathBuf,
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
pub enum CorpusAction {
    /// Validate corpus schema conformance and provenance independence
    Validate {
        /// Path to the corpus directory
        #[arg(long, default_value = "corpus")]
        corpus: PathBuf,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Corpus { action } => match action {
            CorpusAction::Validate { corpus } => crate::validate::run(&corpus),
        },
        Commands::Conformance {
            owl,
            axioms,
            format,
        } => {
            let report = crate::conformance::run_check(&owl, &axioms)?;
            match format {
                OutputFormat::Table => {
                    print!("{}", crate::conformance::format_table(&report));
                }
                OutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&report)
                        .map_err(|e| anyhow::anyhow!("JSON serialization error: {e}"))?;
                    println!("{json}");
                }
            }
            if !report.ok {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}
