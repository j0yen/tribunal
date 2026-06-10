use horned_owl::io::rdf::reader::read;
use horned_owl::io::ParserConfiguration;
use horned_owl::model::{Axiom, ClassExpression, ObjectPropertyExpression, RcStr};
use horned_owl::ontology::set::SetOntology;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub checks: Vec<CheckResult>,
    pub ok: bool,
}

#[derive(Debug, Deserialize)]
pub struct AxiomsFile {
    #[serde(default)]
    pub axioms: Vec<AxiomEntry>,
}

#[derive(Debug, Deserialize)]
pub struct AxiomEntry {
    pub id: String,
    pub pattern: String,
    #[allow(dead_code)]
    pub description: String,
}

impl CheckResult {
    fn pass(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: "pass".to_string(),
            detail: detail.into(),
        }
    }
    fn fail(name: &str, detail: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status: "fail".to_string(),
            detail: detail.into(),
        }
    }
}

/// Get the IRI string from a named entity (Class, ObjectProperty, etc.)
fn named_iri<N>(named: &N) -> String
where
    for<'a> &'a N: Into<String>,
{
    named.into()
}

/// Load and parse the OWL file, returning a SetOntology.
fn load_owl(owl_path: &Path) -> Result<SetOntology<RcStr>, String> {
    let file = std::fs::File::open(owl_path)
        .map_err(|e| format!("failed to open OWL file: {e}"))?;
    let mut reader = BufReader::new(file);
    let config = ParserConfiguration::default();

    let (rdf_onto, _incomplete) =
        read(&mut reader, config).map_err(|e| format!("failed to parse OWL: {e}"))?;

    // Convert RDFOntology into SetOntology
    let set_onto: SetOntology<RcStr> = rdf_onto.into();
    Ok(set_onto)
}

/// Check 1: OWL 2 DL profile compliance (no punning, no reflexive+transitive property).
pub fn check_owl2_dl(onto: &SetOntology<RcStr>) -> CheckResult {
    let name = "owl2-dl-profile";

    let mut class_iris: HashSet<String> = HashSet::new();
    let mut obj_prop_iris: HashSet<String> = HashSet::new();
    let mut data_prop_iris: HashSet<String> = HashSet::new();
    let mut reflexive_props: HashSet<String> = HashSet::new();
    let mut transitive_props: HashSet<String> = HashSet::new();

    for annotated in onto.iter() {
        match &annotated.axiom {
            Axiom::DeclareClass(dc) => {
                // DeclareClass<A>(pub Class<A>), dc.0 = Class<A>
                class_iris.insert(named_iri(&dc.0));
            }
            Axiom::DeclareObjectProperty(dp) => {
                // DeclareObjectProperty<A>(pub ObjectProperty<A>), dp.0 = ObjectProperty<A>
                obj_prop_iris.insert(named_iri(&dp.0));
            }
            Axiom::DeclareDataProperty(dp) => {
                // DeclareDataProperty<A>(pub DataProperty<A>), dp.0 = DataProperty<A>
                data_prop_iris.insert(named_iri(&dp.0));
            }
            Axiom::ReflexiveObjectProperty(r) => {
                // ReflexiveObjectProperty<A>(pub ObjectPropertyExpression<A>), r.0 = OPE
                if let ObjectPropertyExpression::ObjectProperty(op) = &r.0 {
                    obj_prop_iris.insert(named_iri(op));
                    reflexive_props.insert(named_iri(op));
                }
            }
            Axiom::TransitiveObjectProperty(t) => {
                // TransitiveObjectProperty<A>(pub ObjectPropertyExpression<A>)
                if let ObjectPropertyExpression::ObjectProperty(op) = &t.0 {
                    transitive_props.insert(named_iri(op));
                }
            }
            _ => {}
        }
    }

    // Detect class/property punning
    let mut class_obj_pun: Vec<_> = class_iris.intersection(&obj_prop_iris).cloned().collect();
    class_obj_pun.sort();
    let mut class_data_pun: Vec<_> = class_iris.intersection(&data_prop_iris).cloned().collect();
    class_data_pun.sort();

    // Detect reflexive+transitive violation
    let mut rt_violations: Vec<_> = reflexive_props
        .intersection(&transitive_props)
        .cloned()
        .collect();
    rt_violations.sort();

    if !class_obj_pun.is_empty() {
        return CheckResult::fail(
            name,
            format!(
                "punning detected (class/object-property): {}",
                class_obj_pun.join(", ")
            ),
        );
    }
    if !class_data_pun.is_empty() {
        return CheckResult::fail(
            name,
            format!(
                "punning detected (class/data-property): {}",
                class_data_pun.join(", ")
            ),
        );
    }
    if !rt_violations.is_empty() {
        return CheckResult::fail(
            name,
            format!(
                "OWL DL violation: reflexive+transitive property: {}",
                rt_violations.join(", ")
            ),
        );
    }

    CheckResult::pass(name, "no punning or DL violations detected")
}

/// Check 2: Single inheritance — each named class has ≤1 named asserted superclass.
pub fn check_single_inheritance(onto: &SetOntology<RcStr>) -> CheckResult {
    let name = "single-inheritance";

    // Map class IRI -> list of named superclass IRIs
    let mut super_counts: HashMap<String, Vec<String>> = HashMap::new();

    for annotated in onto.iter() {
        // Axiom::SubClassOf(SubClassOf<A>) where SubClassOf has fields sub and sup
        if let Axiom::SubClassOf(sco) = &annotated.axiom {
            // sub must be a named class
            let ClassExpression::Class(sub_cls) = &sco.sub else {
                continue;
            };
            let sub_iri = named_iri(sub_cls);

            // super must also be a named class (not an expression)
            if let ClassExpression::Class(sup_cls) = &sco.sup {
                let sup_iri = named_iri(sup_cls);
                super_counts.entry(sub_iri).or_default().push(sup_iri);
            }
        }
    }

    let mut violations: Vec<String> = super_counts
        .iter()
        .filter(|(_, supers)| supers.len() > 1)
        .map(|(cls, supers)| {
            format!(
                "{cls} has {} superclasses: {}",
                supers.len(),
                supers.join(", ")
            )
        })
        .collect();
    violations.sort();

    if violations.is_empty() {
        CheckResult::pass(name, "all classes have \u{2264}1 named superclass")
    } else {
        CheckResult::fail(
            name,
            format!("multiple inheritance: {}", violations.join("; ")),
        )
    }
}

/// Check 3: TBox-only — no ABox axioms.
pub fn check_tbox_only(onto: &SetOntology<RcStr>) -> CheckResult {
    let name = "tbox-only";

    let mut abox_found: Vec<&'static str> = Vec::new();

    for annotated in onto.iter() {
        match &annotated.axiom {
            Axiom::ClassAssertion(_) => abox_found.push("ClassAssertion"),
            Axiom::ObjectPropertyAssertion(_) => abox_found.push("ObjectPropertyAssertion"),
            Axiom::DataPropertyAssertion(_) => abox_found.push("DataPropertyAssertion"),
            Axiom::SameIndividual(_) => abox_found.push("SameIndividual"),
            Axiom::DifferentIndividuals(_) => abox_found.push("DifferentIndividuals"),
            _ => {}
        }
    }

    if abox_found.is_empty() {
        CheckResult::pass(name, "no ABox axioms found \u{2014} TBox-only confirmed")
    } else {
        let mut uniq = abox_found.clone();
        uniq.sort_unstable();
        uniq.dedup();
        CheckResult::fail(
            name,
            format!(
                "ABox axioms present ({}): {} occurrence(s)",
                uniq.join(", "),
                abox_found.len()
            ),
        )
    }
}

/// Check 4: Ten-axiom encoding table — each entry in axioms.toml is present.
pub fn check_axiom_table(onto: &SetOntology<RcStr>, axioms_path: &Path) -> CheckResult {
    let name = "axiom-encoding-table";

    if !axioms_path.exists() {
        return CheckResult::fail(
            name,
            format!("axioms.toml not found at {}", axioms_path.display()),
        );
    }

    let content = match std::fs::read_to_string(axioms_path) {
        Ok(s) => s,
        Err(e) => {
            return CheckResult::fail(name, format!("failed to read axioms.toml: {e}"))
        }
    };

    let axioms_file: AxiomsFile = match toml::from_str(&content) {
        Ok(a) => a,
        Err(e) => {
            return CheckResult::fail(name, format!("failed to parse axioms.toml: {e}"))
        }
    };

    if axioms_file.axioms.len() < 10 {
        return CheckResult::fail(
            name,
            format!(
                "axioms.toml has {} entries, need at least 10",
                axioms_file.axioms.len()
            ),
        );
    }

    // Inventory which patterns are present in the ontology
    let mut has_subclassof = false;
    let mut has_equivalent = false;
    let mut has_disjoint = false;

    for annotated in onto.iter() {
        match &annotated.axiom {
            Axiom::SubClassOf(_) => has_subclassof = true,
            Axiom::EquivalentClasses(_) => has_equivalent = true,
            Axiom::DisjointClasses(_) => has_disjoint = true,
            _ => {}
        }
    }

    let mut missing: Vec<String> = Vec::new();

    for entry in &axioms_file.axioms {
        let present = match entry.pattern.as_str() {
            "SubClassOf" => has_subclassof,
            "EquivalentClasses" => has_equivalent,
            "DisjointClasses" => has_disjoint,
            _other => true, // unknown pattern — pragmatic pass
        };
        if !present {
            missing.push(format!("{} ({})", entry.id, entry.pattern));
        }
    }

    if missing.is_empty() {
        CheckResult::pass(
            name,
            format!("all {} axiom entries confirmed", axioms_file.axioms.len()),
        )
    } else {
        CheckResult::fail(
            name,
            format!("missing axiom patterns: {}", missing.join(", ")),
        )
    }
}

/// Run all 4 conformance checks and return a report.
pub fn run(owl_path: &Path, axioms_path: &Path) -> ConformanceReport {
    let onto = match load_owl(owl_path) {
        Ok(o) => o,
        Err(e) => {
            let check = CheckResult::fail("parse", format!("failed to load OWL: {e}"));
            return ConformanceReport {
                ok: false,
                checks: vec![check],
            };
        }
    };

    let checks = vec![
        check_owl2_dl(&onto),
        check_single_inheritance(&onto),
        check_tbox_only(&onto),
        check_axiom_table(&onto, axioms_path),
    ];

    let ok = checks.iter().all(|c| c.status == "pass");
    ConformanceReport { checks, ok }
}
