use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct OverallStats {
    pub ran: usize,
    pub errored: usize,
    pub passed: usize,
    pub accuracy: f32,
}

/// 3×3 confusion matrix: rows = expected (allow/flag/deny), cols = actual (allow/flag/deny)
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ConfusionMatrix {
    /// confusion[expected][actual] counts
    pub allow_allow: usize,
    pub allow_flag: usize,
    pub allow_deny: usize,
    pub flag_allow: usize,
    pub flag_flag: usize,
    pub flag_deny: usize,
    pub deny_allow: usize,
    pub deny_flag: usize,
    pub deny_deny: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PerTenetStats {
    pub tenet: String,
    pub ran: usize,
    pub passed: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchReport {
    pub overall: OverallStats,
    pub per_tenet: Vec<PerTenetStats>,
    pub confusion: ConfusionMatrix,
    /// Cases where expected=deny but actual=allow (critical failures)
    pub false_allow: Vec<String>,
    /// Count of cases where verdict matched but rule didn't
    pub wrong_rule: usize,
    /// Cases that errored (guard invocation failed or bad JSON)
    pub errored_cases: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExpectedToml {
    pub verdict: String,
    pub rule: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GuardResponse {
    pub verdict: String,
    pub rule: String,
    #[serde(default)]
    pub axioms: Vec<String>,
}

/// A single resolved corpus case.
#[derive(Debug)]
pub struct CorpusCase {
    pub id: String,
    pub tenet: String,
    pub action_path: PathBuf,
    pub expected: ExpectedToml,
}

/// Walk `corpus_dir` and collect all cases.
pub fn discover_cases(corpus_dir: &Path) -> Vec<CorpusCase> {
    let mut cases = Vec::new();

    let tenet_entries = match std::fs::read_dir(corpus_dir) {
        Ok(e) => e,
        Err(_) => return cases,
    };

    let mut tenet_dirs: Vec<_> = tenet_entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    tenet_dirs.sort();

    for tenet_path in tenet_dirs {
        let tenet_name = tenet_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let case_entries = match std::fs::read_dir(&tenet_path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut case_dirs: Vec<_> = case_entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect();
        case_dirs.sort();

        for case_path in case_dirs {
            let case_name = case_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let action_path = case_path.join("action.json");
            let expected_path = case_path.join("expected.toml");

            if !action_path.exists() || !expected_path.exists() {
                continue;
            }

            let content = match std::fs::read_to_string(&expected_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let expected: ExpectedToml = match toml::from_str(&content) {
                Ok(e) => e,
                Err(_) => continue,
            };

            cases.push(CorpusCase {
                id: format!("{tenet_name}/{case_name}"),
                tenet: tenet_name.clone(),
                action_path,
                expected,
            });
        }
    }

    cases
}

/// Invoke the guard binary for one case and return the parsed response.
pub fn invoke_guard(
    guard: &Path,
    owl: Option<&Path>,
    action: &Path,
) -> Result<GuardResponse, String> {
    let mut cmd = Command::new(guard);
    cmd.arg("check");

    if let Some(owl_path) = owl {
        cmd.arg("--owl").arg(owl_path);
    }

    cmd.arg("--action")
        .arg(action)
        .arg("--format")
        .arg("json")
        .arg("--explain");

    let output = cmd
        .output()
        .map_err(|e| format!("failed to invoke guard: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: GuardResponse = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("invalid JSON from guard: {e} (output: {stdout:?})"))?;

    Ok(response)
}

fn verdict_index(v: &str) -> usize {
    match v {
        "allow" => 0,
        "flag" => 1,
        "deny" => 2,
        _ => 0,
    }
}

fn increment_confusion(matrix: &mut ConfusionMatrix, expected: &str, actual: &str) {
    match (verdict_index(expected), verdict_index(actual)) {
        (0, 0) => matrix.allow_allow += 1,
        (0, 1) => matrix.allow_flag += 1,
        (0, 2) => matrix.allow_deny += 1,
        (1, 0) => matrix.flag_allow += 1,
        (1, 1) => matrix.flag_flag += 1,
        (1, 2) => matrix.flag_deny += 1,
        (2, 0) => matrix.deny_allow += 1,
        (2, 1) => matrix.deny_flag += 1,
        (2, 2) => matrix.deny_deny += 1,
        _ => {}
    }
}

/// Run the full bench over a corpus directory.
/// Returns Err if any invocation failed the wire contract (bad JSON etc.).
pub fn run(
    guard: &Path,
    corpus_dir: &Path,
    owl: Option<&Path>,
) -> Result<BenchReport, BenchReport> {
    let cases = discover_cases(corpus_dir);

    let mut overall = OverallStats::default();
    let mut confusion = ConfusionMatrix::default();
    let mut per_tenet_map: std::collections::HashMap<String, PerTenetStats> =
        std::collections::HashMap::new();
    let mut false_allow: Vec<String> = Vec::new();
    let mut wrong_rule = 0usize;
    let mut errored_cases: Vec<String> = Vec::new();
    let mut any_error = false;

    for case in &cases {
        overall.ran += 1;

        // Ensure tenet entry exists
        per_tenet_map
            .entry(case.tenet.clone())
            .or_insert_with(|| PerTenetStats {
                tenet: case.tenet.clone(),
                ran: 0,
                passed: 0,
            })
            .ran += 1;

        match invoke_guard(guard, owl, &case.action_path) {
            Ok(response) => {
                let verdict_match = response.verdict == case.expected.verdict;
                let rule_match = response.rule == case.expected.rule;

                increment_confusion(&mut confusion, &case.expected.verdict, &response.verdict);

                if verdict_match {
                    overall.passed += 1;
                    per_tenet_map.get_mut(&case.tenet).unwrap().passed += 1;

                    if !rule_match {
                        wrong_rule += 1;
                    }
                }

                // false_allow: expected deny, got allow
                if case.expected.verdict == "deny" && response.verdict == "allow" {
                    false_allow.push(case.id.clone());
                }
            }
            Err(e) => {
                overall.errored += 1;
                errored_cases.push(format!("{}: {e}", case.id));
                any_error = true;
            }
        }
    }

    let ran_ok = overall.ran.saturating_sub(overall.errored);
    overall.accuracy = if ran_ok > 0 {
        overall.passed as f32 / ran_ok as f32
    } else {
        0.0
    };

    let mut per_tenet: Vec<PerTenetStats> = per_tenet_map.into_values().collect();
    per_tenet.sort_by(|a, b| a.tenet.cmp(&b.tenet));

    let report = BenchReport {
        overall,
        per_tenet,
        confusion,
        false_allow,
        wrong_rule,
        errored_cases,
    };

    if any_error {
        Err(report)
    } else {
        Ok(report)
    }
}
