use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod conformance;

#[derive(Parser)]
#[command(name = "tribunal", about = "Tribunal ethics engine tooling")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check an OWL file against structural conformance claims
    Conformance {
        /// Path to the OWL file to check
        #[arg(long)]
        owl: PathBuf,

        /// Path to axioms.toml (default: axioms.toml next to the OWL file)
        #[arg(long)]
        axioms: Option<PathBuf>,

        /// Output format: text (default) or json
        #[arg(long, default_value = "text")]
        format: String,
    },
}

fn main() {
    sigpipe::reset();

    let cli = Cli::parse();

    match cli.command {
        Commands::Conformance {
            owl,
            axioms,
            format,
        } => {
            // Default axioms path: same dir as OWL file
            let axioms_path = axioms.unwrap_or_else(|| {
                owl.parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join("axioms.toml")
            });

            let result = conformance::run(&owl, &axioms_path);

            match format.as_str() {
                "json" => {
                    let json = serde_json::to_string_pretty(&result).unwrap();
                    println!("{json}");
                }
                _ => {
                    // Human table
                    println!("{:<40} {:<8} {}", "Check", "Status", "Detail");
                    println!("{}", "-".repeat(80));
                    for check in &result.checks {
                        println!(
                            "{:<40} {:<8} {}",
                            check.name, check.status, check.detail
                        );
                    }
                    println!();
                    if result.ok {
                        println!("CONFORMANCE OK");
                    } else {
                        println!("CONFORMANCE FAILED");
                        std::process::exit(1);
                    }
                }
            }

            if !result.ok && format == "json" {
                std::process::exit(1);
            }
        }
    }
}
