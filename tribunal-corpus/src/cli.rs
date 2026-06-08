use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tribunal", about = "Ethics corpus validator for ousia-guard")]
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
}

#[derive(Subcommand)]
pub enum CorpusAction {
    /// Validate corpus schema conformance and provenance independence
    Validate {
        /// Path to the corpus directory
        #[arg(long, default_value = "corpus")]
        corpus: std::path::PathBuf,
    },
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Corpus { action } => match action {
            CorpusAction::Validate { corpus } => crate::validate::run(&corpus),
        },
    }
}
