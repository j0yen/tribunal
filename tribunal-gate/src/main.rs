use clap::Parser;
use std::path::PathBuf;
use tribunal_gate::gate;

#[derive(Parser)]
#[command(
    name = "tribunal-gate",
    about = "Publish precondition gate: conformance + bench + policy"
)]
struct Cli {
    /// Path to the OWL ontology to check
    #[arg(long)]
    owl: PathBuf,

    /// Path to the guard binary
    #[arg(long)]
    guard: PathBuf,

    /// Path to the corpus directory
    #[arg(long)]
    corpus: PathBuf,

    /// Minimum accuracy (overrides policy file)
    #[arg(long)]
    min_accuracy: Option<f32>,

    /// Path to gate-policy.toml
    #[arg(long)]
    policy: Option<PathBuf>,

    /// Path to axioms.toml (for conformance check)
    #[arg(long)]
    axioms: Option<PathBuf>,

    /// Dry-run: print what would be checked but don't exit non-zero on failure
    #[arg(long)]
    dry_run: bool,
}

fn main() {
    sigpipe::reset();

    let cli = Cli::parse();

    // Load policy
    let mut policy = if let Some(ref p) = cli.policy {
        match gate::load_policy(p) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error loading policy: {e}");
                std::process::exit(2);
            }
        }
    } else {
        gate::Policy::default()
    };

    // CLI override for min_accuracy
    if let Some(acc) = cli.min_accuracy {
        policy.min_accuracy = acc;
    }

    // Resolve binaries
    let tribunal_bin = match gate::resolve_binary("tribunal") {
        Some(p) => p,
        None => {
            eprintln!("error: 'tribunal' binary not found on PATH or next to tribunal-gate");
            std::process::exit(2);
        }
    };
    let bench_bin = match gate::resolve_binary("tribunal-bench") {
        Some(p) => p,
        None => {
            eprintln!("error: 'tribunal-bench' binary not found on PATH or next to tribunal-gate");
            std::process::exit(2);
        }
    };

    let input = gate::GateInput {
        owl: &cli.owl,
        guard: &cli.guard,
        corpus: &cli.corpus,
        policy: &policy,
        tribunal_bin: &tribunal_bin,
        bench_bin: &bench_bin,
        axioms: cli.axioms.as_deref(),
    };

    match gate::run_gate(&input) {
        Ok(()) => {
            println!("PUBLISH ALLOWED");
        }
        Err(reason) => {
            if cli.dry_run {
                println!("DRY-RUN BLOCKED: {}", reason.message());
            } else {
                eprintln!("PUBLISH BLOCKED: {}", reason.message());
                std::process::exit(1);
            }
        }
    }
}
