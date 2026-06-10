use std::path::PathBuf;
use tribunal_bench::bench;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn corpus_dir() -> PathBuf {
    fixtures_dir().join("corpus")
}

fn guard_stub() -> PathBuf {
    fixtures_dir().join("guard-stub")
}

fn guard_stub_wrong_rule() -> PathBuf {
    fixtures_dir().join("guard-stub-wrong-rule")
}

fn guard_stub_bad_json() -> PathBuf {
    fixtures_dir().join("guard-stub-bad-json")
}

fn guard_stub_false_allow() -> PathBuf {
    fixtures_dir().join("guard-stub-false-allow")
}

#[test]
fn guard_stub_produces_positive_accuracy() {
    let result = bench::run(&guard_stub(), &corpus_dir(), None);
    let report = result.expect("guard-stub should not error");

    assert!(
        report.overall.ran > 0,
        "should have run at least 1 case, got {}",
        report.overall.ran
    );
    assert!(
        report.overall.accuracy > 0.0,
        "accuracy should be > 0, got {}",
        report.overall.accuracy
    );
    assert_eq!(
        report.overall.errored, 0,
        "guard-stub should produce no errors"
    );
}

#[test]
fn guard_stub_no_false_allow() {
    let result = bench::run(&guard_stub(), &corpus_dir(), None);
    let report = result.expect("guard-stub should not error");

    assert_eq!(
        report.false_allow.len(),
        0,
        "guard-stub should not produce false_allow, got: {:?}",
        report.false_allow
    );
}

#[test]
fn guard_stub_false_allow_detection() {
    // The false-allow stub always returns allow, so all deny cases become false_allow
    let result = bench::run(&guard_stub_false_allow(), &corpus_dir(), None);
    // Even with errors it may return Err; extract the report either way
    let report = match result {
        Ok(r) => r,
        Err(r) => r,
    };

    assert!(
        !report.false_allow.is_empty(),
        "false-allow stub should produce at least 1 false_allow (deny case expected)"
    );
    // All false_allow should be deny cases (their directory prefix is deny-*)
    for id in &report.false_allow {
        let case_name = id.split('/').last().unwrap_or("");
        assert!(
            case_name.starts_with("deny-"),
            "false_allow case should start with 'deny-': {id}"
        );
    }
}

#[test]
fn guard_stub_wrong_rule_counts_correctly() {
    let result = bench::run(&guard_stub_wrong_rule(), &corpus_dir(), None);
    let report = result.expect("wrong-rule stub should not error");

    // Every case where verdict matched should contribute to wrong_rule
    // The wrong-rule stub always returns "wrong-rule" so any passed case = wrong_rule
    assert!(
        report.wrong_rule > 0,
        "wrong-rule stub should produce wrong_rule > 0, got {}",
        report.wrong_rule
    );
}

#[test]
fn guard_stub_bad_json_causes_error_exit() {
    let result = bench::run(&guard_stub_bad_json(), &corpus_dir(), None);
    assert!(
        result.is_err(),
        "bad-json stub should cause bench to return Err"
    );
    let report = result.unwrap_err();
    assert!(
        report.overall.errored > 0,
        "should have at least 1 errored case"
    );
    assert!(
        !report.errored_cases.is_empty(),
        "errored_cases should be non-empty"
    );
}

#[test]
fn per_tenet_breakdown_populated() {
    let result = bench::run(&guard_stub(), &corpus_dir(), None);
    let report = result.expect("guard-stub should not error");

    assert!(
        !report.per_tenet.is_empty(),
        "per_tenet should have at least one entry"
    );
    // tenet_a and tenet_b should both appear
    let tenet_names: Vec<_> = report.per_tenet.iter().map(|t| t.tenet.as_str()).collect();
    assert!(
        tenet_names.contains(&"tenet_a"),
        "expected tenet_a in per_tenet: {tenet_names:?}"
    );
}

#[test]
fn report_serializes_to_json() {
    let result = bench::run(&guard_stub(), &corpus_dir(), None);
    let report = result.expect("should succeed");
    let json = serde_json::to_string(&report).expect("should serialize");
    let val: serde_json::Value = serde_json::from_str(&json).expect("should parse");
    assert!(val.get("overall").is_some(), "JSON should have 'overall'");
    assert!(val.get("confusion").is_some(), "JSON should have 'confusion'");
}
