use clap::Parser;
use std::path::PathBuf;
use tribunal_bench::bench;

#[derive(Parser)]
#[command(
    name = "tribunal-bench",
    about = "Run ousia-guard over a corpus and report accuracy"
)]
struct Cli {
    /// Path to the guard binary (or stub)
    #[arg(long)]
    guard: PathBuf,

    /// Path to the corpus directory
    #[arg(long)]
    corpus: PathBuf,

    /// Optional OWL ontology to pass to the guard
    #[arg(long)]
    owl: Option<PathBuf>,

    /// Output format: text (default) or json
    #[arg(long, default_value = "text")]
    format: String,
}

fn main() {
    sigpipe::reset();

    let cli = Cli::parse();

    let result = bench::run(&cli.guard, &cli.corpus, cli.owl.as_deref());

    let (report, had_errors) = match result {
        Ok(r) => (r, false),
        Err(r) => (r, true),
    };

    match cli.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&report).unwrap();
            println!("{json}");
        }
        _ => {
            println!("tribunal bench report");
            println!("=====================");
            println!(
                "ran: {}  errored: {}  passed: {}  accuracy: {:.1}%",
                report.overall.ran,
                report.overall.errored,
                report.overall.passed,
                report.overall.accuracy * 100.0
            );
            println!();
            println!("per-tenet:");
            for t in &report.per_tenet {
                let acc = if t.ran > 0 {
                    t.passed as f32 / t.ran as f32 * 100.0
                } else {
                    0.0
                };
                println!("  {}: {}/{} ({:.1}%)", t.tenet, t.passed, t.ran, acc);
            }
            println!();
            println!("confusion matrix (expected vs actual):");
            println!("         allow  flag  deny");
            println!(
                "  allow:  {:5}  {:4}  {:4}",
                report.confusion.allow_allow,
                report.confusion.allow_flag,
                report.confusion.allow_deny
            );
            println!(
                "  flag:   {:5}  {:4}  {:4}",
                report.confusion.flag_allow,
                report.confusion.flag_flag,
                report.confusion.flag_deny
            );
            println!(
                "  deny:   {:5}  {:4}  {:4}",
                report.confusion.deny_allow,
                report.confusion.deny_flag,
                report.confusion.deny_deny
            );
            println!();
            if report.wrong_rule > 0 {
                println!("wrong-rule count (correct verdict, wrong rule): {}", report.wrong_rule);
            }
            if !report.false_allow.is_empty() {
                println!("FALSE ALLOW (expected=deny, actual=allow):");
                for id in &report.false_allow {
                    println!("  - {id}");
                }
            }
            if !report.errored_cases.is_empty() {
                println!("errored cases:");
                for id in &report.errored_cases {
                    println!("  - {id}");
                }
            }
        }
    }

    if had_errors {
        std::process::exit(1);
    }
}
