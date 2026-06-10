use std::path::PathBuf;
use tribunal_gate::gate::{BlockReason, GateInput, Policy, run_gate};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tribunal-bench/fixtures")
}

fn corpus_dir() -> PathBuf {
    fixtures_dir().join("corpus")
}

fn guard_stub() -> PathBuf {
    fixtures_dir().join("guard-stub")
}

fn owl_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tribunal-conformance/fixtures/world-min.owl")
}

fn axioms_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tribunal-conformance/fixtures/axioms.toml")
}

/// Find the built tribunal binary.
fn find_binary(name: &str) -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
            // In debug, test binaries are in deps/ — go up one level
            if let Some(parent) = dir.parent() {
                let candidate = parent.join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    // Try PATH
    if let Ok(output) = std::process::Command::new("which").arg(name).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

fn skip_if_no_binaries() -> Option<(PathBuf, PathBuf)> {
    let tribunal = find_binary("tribunal")?;
    let bench = find_binary("tribunal-bench")?;
    Some((tribunal, bench))
}

/// Build a passing gate input using real binaries if available.
#[test]
fn publish_allowed_with_all_pass_fixtures() {
    let (tribunal_bin, bench_bin) = match skip_if_no_binaries() {
        Some(b) => b,
        None => {
            eprintln!("skipping: tribunal/tribunal-bench binaries not found");
            return;
        }
    };

    // bench corpus has 4 cases across 2 tenets (tenet_a: 3, tenet_b: 1)
    // tenet_b has only 1 case — use per_tenet_min_n = 1 to pass
    let policy = Policy {
        min_accuracy: 1.0,
        per_tenet_min_n: 1,
    };

    let owl = owl_path();
    let corpus = corpus_dir();
    let guard = guard_stub();
    let ax = axioms_path();

    let input = GateInput {
        owl: &owl,
        guard: &guard,
        corpus: &corpus,
        policy: &policy,
        tribunal_bin: &tribunal_bin,
        bench_bin: &bench_bin,
        axioms: Some(&ax),
    };

    let result = run_gate(&input);
    match &result {
        Err(reason) => panic!("expected PUBLISH ALLOWED, got BLOCKED: {}", reason.message()),
        Ok(()) => {}
    }
}

#[test]
fn policy_low_accuracy_blocks() {
    let (tribunal_bin, bench_bin) = match skip_if_no_binaries() {
        Some(b) => b,
        None => {
            eprintln!("skipping: tribunal/tribunal-bench binaries not found");
            return;
        }
    };

    let false_allow_stub = fixtures_dir().join("guard-stub-false-allow");
    let policy = Policy {
        min_accuracy: 0.99,
        per_tenet_min_n: 1,
    };

    let owl = owl_path();
    let corpus = corpus_dir();
    let ax = axioms_path();

    let input = GateInput {
        owl: &owl,
        guard: &false_allow_stub,
        corpus: &corpus,
        policy: &policy,
        tribunal_bin: &tribunal_bin,
        bench_bin: &bench_bin,
        axioms: Some(&ax),
    };

    let result = run_gate(&input);
    assert!(result.is_err(), "should be blocked with false-allow stub");
    // Should block on false_allow (non-overridable) before accuracy
    match result.unwrap_err() {
        BlockReason::FalseAllowNonZero(_) => {}
        other => panic!("expected FalseAllowNonZero, got: {}", other.message()),
    }
}

#[test]
fn conformance_failure_blocks() {
    let (tribunal_bin, bench_bin) = match skip_if_no_binaries() {
        Some(b) => b,
        None => {
            eprintln!("skipping: tribunal/tribunal-bench binaries not found");
            return;
        }
    };

    let bad_owl = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tribunal-conformance/fixtures/world-bad.owl");

    let policy = Policy {
        min_accuracy: 0.0,
        per_tenet_min_n: 0,
    };

    let corpus = corpus_dir();
    let guard = guard_stub();
    let ax = axioms_path();

    let input = GateInput {
        owl: &bad_owl,
        guard: &guard,
        corpus: &corpus,
        policy: &policy,
        tribunal_bin: &tribunal_bin,
        bench_bin: &bench_bin,
        axioms: Some(&ax),
    };

    let result = run_gate(&input);
    assert!(result.is_err(), "bad OWL should block gate");
    match result.unwrap_err() {
        BlockReason::ConformanceFailed(_) => {}
        other => panic!("expected ConformanceFailed, got: {}", other.message()),
    }
}

#[test]
fn false_allow_zero_is_non_negotiable() {
    let (tribunal_bin, bench_bin) = match skip_if_no_binaries() {
        Some(b) => b,
        None => {
            eprintln!("skipping: tribunal/tribunal-bench binaries not found");
            return;
        }
    };

    let false_allow_stub = fixtures_dir().join("guard-stub-false-allow");
    // Policy with 0% min accuracy — even so, false_allow should block
    let policy = Policy {
        min_accuracy: 0.0,
        per_tenet_min_n: 0,
    };

    let owl = owl_path();
    let corpus = corpus_dir();
    let ax = axioms_path();

    let input = GateInput {
        owl: &owl,
        guard: &false_allow_stub,
        corpus: &corpus,
        policy: &policy,
        tribunal_bin: &tribunal_bin,
        bench_bin: &bench_bin,
        axioms: Some(&ax),
    };

    let result = run_gate(&input);
    assert!(result.is_err(), "false_allow > 0 should always block");
    match result.unwrap_err() {
        BlockReason::FalseAllowNonZero(_) => {}
        other => panic!("expected FalseAllowNonZero, got: {}", other.message()),
    }
}

#[test]
fn policy_loads_from_toml() {
    let policy_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("gate-policy.toml");
    if !policy_path.exists() {
        return;
    }
    let policy = tribunal_gate::gate::load_policy(&policy_path).expect("should load policy");
    assert!(policy.min_accuracy > 0.0, "min_accuracy should be positive");
    assert!(policy.per_tenet_min_n > 0, "per_tenet_min_n should be positive");
}
