use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Deserialize, Clone)]
pub struct Policy {
    #[serde(default = "default_min_accuracy")]
    pub min_accuracy: f32,
    #[serde(default = "default_per_tenet_min_n")]
    pub per_tenet_min_n: usize,
}

fn default_min_accuracy() -> f32 {
    0.85
}
fn default_per_tenet_min_n() -> usize {
    2
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            min_accuracy: default_min_accuracy(),
            per_tenet_min_n: default_per_tenet_min_n(),
        }
    }
}

pub fn load_policy(path: &Path) -> Result<Policy, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read policy: {e}"))?;
    let policy: Policy =
        toml::from_str(&content).map_err(|e| format!("failed to parse policy: {e}"))?;
    Ok(policy)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConformanceCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub checks: Vec<ConformanceCheck>,
    pub ok: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OverallStats {
    pub ran: usize,
    pub errored: usize,
    pub passed: usize,
    pub accuracy: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerTenetStats {
    pub tenet: String,
    pub ran: usize,
    pub passed: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchReport {
    pub overall: OverallStats,
    pub per_tenet: Vec<PerTenetStats>,
    pub false_allow: Vec<String>,
}

#[derive(Debug)]
pub enum BlockReason {
    ConformanceFailed(Vec<String>),
    FalseAllowNonZero(Vec<String>),
    AccuracyTooLow { actual: f32, required: f32 },
    TenetUnderCovered { tenet: String, n: usize, required: usize },
    ConformanceBinaryFailed(String),
    BenchBinaryFailed(String),
    ParseError(String),
}

impl BlockReason {
    pub fn message(&self) -> String {
        match self {
            BlockReason::ConformanceFailed(fails) => {
                format!("CONFORMANCE FAILED: {}", fails.join(", "))
            }
            BlockReason::FalseAllowNonZero(cases) => {
                format!("FALSE ALLOW: {} case(s) — {}", cases.len(), cases.join(", "))
            }
            BlockReason::AccuracyTooLow { actual, required } => {
                format!(
                    "ACCURACY TOO LOW: {:.1}% < required {:.1}%",
                    actual * 100.0,
                    required * 100.0
                )
            }
            BlockReason::TenetUnderCovered { tenet, n, required } => {
                format!(
                    "TENET UNDER-COVERED: '{tenet}' has {n} case(s), need {required}"
                )
            }
            BlockReason::ConformanceBinaryFailed(e) => {
                format!("CONFORMANCE BINARY ERROR: {e}")
            }
            BlockReason::BenchBinaryFailed(e) => {
                format!("BENCH BINARY ERROR: {e}")
            }
            BlockReason::ParseError(e) => {
                format!("PARSE ERROR: {e}")
            }
        }
    }
}

/// Find the path to a sibling binary (e.g. `tribunal`) by checking PATH first,
/// then the directory of the current executable.
pub fn resolve_binary(name: &str) -> Option<std::path::PathBuf> {
    // Check PATH
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(std::path::PathBuf::from(s));
            }
        }
    }

    // Fall back to sibling of current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Run `tribunal conformance --owl <owl> --format json` and parse the result.
pub fn run_conformance(
    tribunal_bin: &Path,
    owl: &Path,
    axioms: Option<&Path>,
) -> Result<ConformanceReport, BlockReason> {
    let mut cmd = Command::new(tribunal_bin);
    cmd.arg("conformance").arg("--owl").arg(owl).arg("--format").arg("json");
    if let Some(ax) = axioms {
        cmd.arg("--axioms").arg(ax);
    }

    let output = cmd
        .output()
        .map_err(|e| BlockReason::ConformanceBinaryFailed(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: ConformanceReport = serde_json::from_str(stdout.trim())
        .map_err(|e| BlockReason::ParseError(format!("conformance JSON: {e} (output: {stdout:?})")))?;

    Ok(report)
}

/// Run `tribunal-bench --guard <g> --corpus <c> --format json` and parse the result.
pub fn run_bench(
    bench_bin: &Path,
    guard: &Path,
    corpus: &Path,
    owl: Option<&Path>,
) -> Result<BenchReport, BlockReason> {
    let mut cmd = Command::new(bench_bin);
    cmd.arg("--guard")
        .arg(guard)
        .arg("--corpus")
        .arg(corpus)
        .arg("--format")
        .arg("json");
    if let Some(o) = owl {
        cmd.arg("--owl").arg(o);
    }

    let output = cmd
        .output()
        .map_err(|e| BlockReason::BenchBinaryFailed(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: BenchReport = serde_json::from_str(stdout.trim())
        .map_err(|e| BlockReason::ParseError(format!("bench JSON: {e} (output: {stdout:?})")))?;

    Ok(report)
}

pub struct GateInput<'a> {
    pub owl: &'a Path,
    pub guard: &'a Path,
    pub corpus: &'a Path,
    pub policy: &'a Policy,
    pub tribunal_bin: &'a Path,
    pub bench_bin: &'a Path,
    pub axioms: Option<&'a Path>,
}

/// Run the full gate and return Ok(()) for PUBLISH ALLOWED, Err(reason) for blocked.
pub fn run_gate(input: &GateInput<'_>) -> Result<(), BlockReason> {
    // Step 1: conformance
    let conf = run_conformance(input.tribunal_bin, input.owl, input.axioms)?;
    if !conf.ok {
        let fails: Vec<String> = conf
            .checks
            .iter()
            .filter(|c| c.status == "fail")
            .map(|c| format!("{}: {}", c.name, c.detail))
            .collect();
        return Err(BlockReason::ConformanceFailed(fails));
    }

    // Step 2: bench
    let bench = run_bench(input.bench_bin, input.guard, input.corpus, Some(input.owl))?;

    // Step 3a: false_allow == 0 (non-overridable)
    if !bench.false_allow.is_empty() {
        return Err(BlockReason::FalseAllowNonZero(bench.false_allow));
    }

    // Step 3b: accuracy >= min_accuracy
    if bench.overall.accuracy < input.policy.min_accuracy {
        return Err(BlockReason::AccuracyTooLow {
            actual: bench.overall.accuracy,
            required: input.policy.min_accuracy,
        });
    }

    // Step 3c: per-tenet coverage
    for tenet in &bench.per_tenet {
        if tenet.ran < input.policy.per_tenet_min_n {
            return Err(BlockReason::TenetUnderCovered {
                tenet: tenet.tenet.clone(),
                n: tenet.ran,
                required: input.policy.per_tenet_min_n,
            });
        }
    }

    Ok(())
}
