use anyhow::{bail, Context};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use walkdir::WalkDir;

/// Required fields for a valid ousia-guard action ABox.
const REQUIRED_FIELDS: &[&str] = &["agent_id", "action_type", "target", "payload", "context"];

/// Valid verdict values.
const VALID_VERDICTS: &[&str] = &["allow", "flag", "deny"];

/// Known Federation Model tenets.
const KNOWN_TENETS: &[&str] = &[
    "diversity_as_strength",
    "growth_over_acquisition",
    "honesty_about_failure",
    "humility_before_unknown",
    "justice_nonnegotiable",
    "material_conditions_for_goodness",
    "power_demands_restraint",
    "primacy_of_sentient_dignity",
    "reason_and_empathy_together",
    "struggle_is_the_point",
];

#[derive(Debug, Deserialize)]
pub struct ExpectedToml {
    pub verdict: String,
    /// Expected rule fired; included for round-trip completeness.
    #[allow(dead_code)]
    pub rule: String,
    /// Tenet tag; present for schema completeness checks.
    #[allow(dead_code)]
    pub tenet: String,
    /// Human-readable rationale from independent source.
    #[allow(dead_code)]
    pub rationale: String,
}

#[derive(Debug, Deserialize)]
pub struct ProvenanceToml {
    pub source: String,
    /// Source reference for human auditors.
    #[allow(dead_code)]
    pub source_ref: String,
    pub author: String,
    /// Human spot-checker; empty until manually reviewed.
    #[allow(dead_code)]
    pub spot_checked_by: String,
}

/// A single corpus case discovered on disk.
#[derive(Debug)]
struct Case {
    tenet: String,
    expected: ExpectedToml,
}

pub fn run(corpus_dir: &Path) -> anyhow::Result<()> {
    if !corpus_dir.exists() {
        bail!("corpus directory '{}' does not exist", corpus_dir.display());
    }

    let cases = discover_cases(corpus_dir)?;

    if cases.is_empty() {
        bail!("no cases found under '{}'", corpus_dir.display());
    }

    // Per-tenet counts and verdict distributions.
    let mut tenet_counts: HashMap<&str, usize> = HashMap::new();
    let mut verdict_dist: HashMap<&str, usize> = HashMap::new();

    for c in &cases {
        *tenet_counts.entry(c.tenet.as_str()).or_insert(0) += 1;
        *verdict_dist.entry(c.expected.verdict.as_str()).or_insert(0) += 1;
    }

    println!("tribunal corpus validate");
    println!("========================");
    println!("total cases: {}", cases.len());
    println!();
    println!("per-tenet counts:");
    let mut tenets: Vec<_> = tenet_counts.iter().collect();
    tenets.sort_by_key(|(k, _)| *k);
    for (tenet, count) in &tenets {
        println!("  {tenet}: {count}");
    }
    println!();
    println!("verdict distribution:");
    let mut verdicts: Vec<_> = verdict_dist.iter().collect();
    verdicts.sort_by_key(|(k, _)| *k);
    for (verdict, count) in &verdicts {
        println!("  {verdict}: {count}");
    }

    // Surface under-covered tenets.
    let mut warnings = Vec::new();
    for t in KNOWN_TENETS {
        let count = tenet_counts.get(t).copied().unwrap_or(0);
        if count < 2 {
            warnings.push(format!("  under-covered tenet: {t} ({count} case(s))"));
        }
    }

    if verdict_dist.get("deny").copied().unwrap_or(0) == 0 {
        warnings.push("  no deny-class cases found".to_string());
    }
    if verdict_dist.get("allow").copied().unwrap_or(0) == 0 {
        warnings.push("  no allow-class cases found".to_string());
    }

    if !warnings.is_empty() {
        println!();
        println!("warnings:");
        for w in &warnings {
            println!("{w}");
        }
    }

    Ok(())
}

fn discover_cases(corpus_dir: &Path) -> anyhow::Result<Vec<Case>> {
    let mut cases = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    // Walk tenet dirs: corpus/<tenet>/<case-id>/
    for tenet_entry in WalkDir::new(corpus_dir)
        .min_depth(1)
        .max_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let tenet_name = tenet_entry.file_name().to_string_lossy().to_string();

        for case_entry in WalkDir::new(tenet_entry.path())
            .min_depth(1)
            .max_depth(1)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
        {
            let case_id = case_entry.file_name().to_string_lossy().to_string();
            let case_path = case_entry.path();
            let display = format!("{tenet_name}/{case_id}");

            match load_case(case_path, &tenet_name, &case_id) {
                Ok(c) => cases.push(c),
                Err(e) => errors.push(format!("{display}: {e:#}")),
            }
        }
    }

    if !errors.is_empty() {
        let msg = errors.join("\n");
        bail!("corpus validation failed:\n{msg}");
    }

    Ok(cases)
}

fn load_case(case_path: &Path, tenet: &str, case_id: &str) -> anyhow::Result<Case> {
    // 1. Load and parse action.json
    let action_path = case_path.join("action.json");
    let action_str = std::fs::read_to_string(&action_path)
        .with_context(|| format!("reading {}", action_path.display()))?;
    let action: serde_json::Value = serde_json::from_str(&action_str)
        .with_context(|| format!("parsing action.json in {tenet}/{case_id}"))?;

    // 2. Validate required fields (structural schema check)
    validate_action_schema(&action, tenet, case_id)?;

    // 3. Load expected.toml
    let expected_path = case_path.join("expected.toml");
    let expected_str = std::fs::read_to_string(&expected_path)
        .with_context(|| format!("reading {}", expected_path.display()))?;
    let expected: ExpectedToml = toml::from_str(&expected_str)
        .with_context(|| format!("parsing expected.toml in {tenet}/{case_id}"))?;

    // Validate verdict value
    if !VALID_VERDICTS.contains(&expected.verdict.as_str()) {
        bail!(
            "expected.toml in {tenet}/{case_id}: invalid verdict '{}' (must be allow|flag|deny)",
            expected.verdict
        );
    }

    // 4. Load provenance.toml
    let provenance_path = case_path.join("provenance.toml");
    let provenance_str = std::fs::read_to_string(&provenance_path)
        .with_context(|| format!("reading {}", provenance_path.display()))?;
    let provenance: ProvenanceToml = toml::from_str(&provenance_str)
        .with_context(|| format!("parsing provenance.toml in {tenet}/{case_id}"))?;

    // 5. Independence checks
    if provenance.author == "ousia-axioms" {
        bail!(
            "provenance.toml in {tenet}/{case_id}: author is 'ousia-axioms' — independence violated"
        );
    }
    if provenance.source.trim().is_empty() {
        bail!(
            "provenance.toml in {tenet}/{case_id}: source is empty — independence check requires a non-empty source"
        );
    }

    Ok(Case {
        tenet: tenet.to_string(),
        expected,
    })
}

/// Hand-rolled schema check: validates required fields exist and are the right JSON types.
/// This validates against the checked-in action.schema.json rules without pulling in
/// the jsonschema crate (which requires rustc > 1.85).
fn validate_action_schema(
    action: &serde_json::Value,
    tenet: &str,
    case_id: &str,
) -> anyhow::Result<()> {
    let obj = match action.as_object() {
        Some(o) => o,
        None => bail!(
            "action.json in {tenet}/{case_id}: root must be a JSON object, found {}",
            type_name(action)
        ),
    };

    for field in REQUIRED_FIELDS {
        if !obj.contains_key(*field) {
            bail!(
                "action.json in {tenet}/{case_id}: missing required field '{field}'"
            );
        }
    }

    // Type checks
    if !obj["agent_id"].is_string() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'agent_id' must be a string"
        );
    }
    if obj["agent_id"].as_str().unwrap_or("").is_empty() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'agent_id' must be non-empty"
        );
    }
    if !obj["action_type"].is_string() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'action_type' must be a string"
        );
    }
    if obj["action_type"].as_str().unwrap_or("").is_empty() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'action_type' must be non-empty"
        );
    }
    if !obj["target"].is_string() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'target' must be a string"
        );
    }
    if obj["target"].as_str().unwrap_or("").is_empty() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'target' must be non-empty"
        );
    }
    if !obj["payload"].is_object() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'payload' must be an object"
        );
    }
    if !obj["context"].is_object() {
        bail!(
            "action.json in {tenet}/{case_id}: field 'context' must be an object"
        );
    }

    Ok(())
}

fn type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn good_action() -> &'static str {
        r#"{
            "agent_id": "test-agent",
            "action_type": "inform",
            "target": "user-123",
            "payload": {"content": "Hello"},
            "context": {}
        }"#
    }

    fn good_expected() -> &'static str {
        r#"verdict = "allow"
rule = "none"
tenet = "primacy_of_sentient_dignity"
rationale = "Sharing information with consent respects dignity."
"#
    }

    fn good_provenance() -> &'static str {
        r#"source = "federation-philosophy"
source_ref = "Notes/federation-utopian-philosophy.md §VI.1"
author = "human-review"
spot_checked_by = ""
"#
    }

    fn write_case(
        tmp: &TempDir,
        tenet: &str,
        case_id: &str,
        action: &str,
        expected: &str,
        prov: &str,
    ) {
        let dir = tmp.path().join(tenet).join(case_id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("action.json"), action).unwrap();
        fs::write(dir.join("expected.toml"), expected).unwrap();
        fs::write(dir.join("provenance.toml"), prov).unwrap();
    }

    #[test]
    fn test_valid_case_passes() {
        let tmp = TempDir::new().unwrap();
        write_case(
            &tmp,
            "primacy_of_sentient_dignity",
            "tc-001",
            good_action(),
            good_expected(),
            good_provenance(),
        );
        let cases = discover_cases(tmp.path()).unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].expected.verdict, "allow");
    }

    #[test]
    fn test_malformed_action_json_fails() {
        let tmp = TempDir::new().unwrap();
        write_case(
            &tmp,
            "justice_nonnegotiable",
            "tc-bad",
            "not-valid-json",
            good_expected(),
            good_provenance(),
        );
        let result = discover_cases(tmp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("justice_nonnegotiable/tc-bad"),
            "error should name the case: {msg}"
        );
    }

    #[test]
    fn test_schema_violation_missing_field_fails() {
        let tmp = TempDir::new().unwrap();
        // Missing required fields
        let bad_action = r#"{"foo": "bar"}"#;
        write_case(
            &tmp,
            "justice_nonnegotiable",
            "tc-bad-schema",
            bad_action,
            good_expected(),
            good_provenance(),
        );
        let result = discover_cases(tmp.path());
        assert!(result.is_err(), "expected schema violation error");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("missing required field"),
            "error should mention missing field: {msg}"
        );
    }

    #[test]
    fn test_ousia_axioms_author_fails() {
        let tmp = TempDir::new().unwrap();
        let bad_prov = r#"source = "federation-philosophy"
source_ref = "§VI"
author = "ousia-axioms"
spot_checked_by = ""
"#;
        write_case(
            &tmp,
            "growth_over_acquisition",
            "tc-prov",
            good_action(),
            good_expected(),
            bad_prov,
        );
        let result = discover_cases(tmp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ousia-axioms"),
            "error should mention ousia-axioms: {msg}"
        );
    }

    #[test]
    fn test_empty_source_fails() {
        let tmp = TempDir::new().unwrap();
        let bad_prov = r#"source = ""
source_ref = "§VI"
author = "human-review"
spot_checked_by = ""
"#;
        write_case(
            &tmp,
            "growth_over_acquisition",
            "tc-empty-src",
            good_action(),
            good_expected(),
            bad_prov,
        );
        let result = discover_cases(tmp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("source is empty"),
            "error should mention empty source: {msg}"
        );
    }

    #[test]
    fn test_shipped_corpus_validates() {
        // Validate the corpus relative to the workspace root when run via cargo test
        let corpus_dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus");
        if !corpus_dir.exists() {
            // Skip if not present (e.g. CI without corpus)
            return;
        }
        let result = run(&corpus_dir);
        assert!(result.is_ok(), "shipped corpus failed: {result:?}");
    }
}
