use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn world_min_owl() -> PathBuf {
    fixtures_dir().join("world-min.owl")
}

fn world_bad_owl() -> PathBuf {
    fixtures_dir().join("world-bad.owl")
}

fn axioms_toml() -> PathBuf {
    fixtures_dir().join("axioms.toml")
}

#[test]
fn world_min_passes_all_checks() {
    let report = tribunal_conformance::conformance::run(&world_min_owl(), &axioms_toml());

    // All checks should pass
    for check in &report.checks {
        assert_eq!(
            check.status, "pass",
            "check '{}' should pass: {}",
            check.name, check.detail
        );
    }
    assert!(report.ok, "world-min.owl should be fully conformant");
}

#[test]
fn world_bad_fails_at_least_two_checks() {
    // world-bad.owl has: punning (DL fail), multi-inheritance, ABox individual
    let report = tribunal_conformance::conformance::run(&world_bad_owl(), &axioms_toml());

    let failures: Vec<_> = report
        .checks
        .iter()
        .filter(|c| c.status == "fail")
        .collect();

    assert!(
        failures.len() >= 2,
        "world-bad.owl should fail at least 2 checks, got {} failures: {:?}",
        failures.len(),
        failures.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert!(!report.ok, "world-bad.owl should not be conformant");
}

#[test]
fn world_bad_fails_dl_check() {
    let report = tribunal_conformance::conformance::run(&world_bad_owl(), &axioms_toml());
    let dl_check = report
        .checks
        .iter()
        .find(|c| c.name == "owl2-dl-profile")
        .expect("should have owl2-dl-profile check");
    assert_eq!(
        dl_check.status, "fail",
        "world-bad.owl should fail DL check: {}",
        dl_check.detail
    );
}

#[test]
fn world_bad_fails_single_inheritance() {
    let report = tribunal_conformance::conformance::run(&world_bad_owl(), &axioms_toml());
    let check = report
        .checks
        .iter()
        .find(|c| c.name == "single-inheritance")
        .expect("should have single-inheritance check");
    assert_eq!(
        check.status, "fail",
        "world-bad.owl should fail single-inheritance: {}",
        check.detail
    );
}

#[test]
fn world_bad_fails_tbox_only() {
    let report = tribunal_conformance::conformance::run(&world_bad_owl(), &axioms_toml());
    let check = report
        .checks
        .iter()
        .find(|c| c.name == "tbox-only")
        .expect("should have tbox-only check");
    assert_eq!(
        check.status, "fail",
        "world-bad.owl should fail tbox-only: {}",
        check.detail
    );
}

#[test]
fn report_json_roundtrip() {
    let report = tribunal_conformance::conformance::run(&world_min_owl(), &axioms_toml());
    let json = serde_json::to_string(&report).expect("should serialize to JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("should parse as JSON");
    assert!(parsed.get("checks").is_some(), "JSON should have 'checks' field");
    assert!(parsed.get("ok").is_some(), "JSON should have 'ok' field");
}
