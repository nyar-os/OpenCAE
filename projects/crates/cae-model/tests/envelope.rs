use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{LoadCombination, Model, OpenCae, OpenCaeOptions, PatternFactor, Units, solve_envelope};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;
const LOAD: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

/// Horizontal bar with a tension load and a roller at the free end.
fn single_bar_model() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("element");
    model.add_support("n1", true, true).expect("fixed n1");
    model.add_support("n2", false, true).expect("roller n2");
    model.add_load("n2", Some(LOAD), None).expect("load");
    model
}

/// Vertical bar in a three-dimensional document with a downward tip load.
fn vertical_bar_3d_model() -> Model {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", 0.0, 0.0, L).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("element");
    model.add_support_3d("n1", true, true, true).expect("fixed n1");
    model.add_support_3d("n2", true, true, false).expect("guided n2");
    model.add_load_3d("n2", None, None, Some(-LOAD)).expect("load");
    model
}

fn combination(name: &str, loads: f64, gravity: f64) -> LoadCombination {
    LoadCombination { name: name.to_string(), loads, gravity, thermal: 1.0, patterns: Vec::new() }
}

#[test]
fn two_dimensional_extremes_name_the_governing_combination() {
    let mut model = single_bar_model();
    model.combinations.push(combination("tension", 1.0, 0.0));
    model.combinations.push(combination("compression", -1.0, 0.0));

    let report = solve_envelope(&model).expect("envelope");
    assert_eq!(report.format, "open-cae-envelope");
    assert_eq!(report.version, 1);
    assert_eq!(report.units, "mm");
    assert_eq!(report.combination_count, 2);
    assert_eq!(report.cases, vec!["base", "tension", "compression"]);
    assert!(report.ok);
    assert_eq!(report.nodes.len(), 2);
    assert_eq!(report.elements.len(), 1);
    assert_eq!(report.reactions.len(), 2);

    let displacement = LOAD * L / (E * A);
    let tip = report.nodes.iter().find(|node| node.id == "n2").expect("tip node");
    assert!(tip.uz.is_none(), "two-dimensional nodes omit uz");
    assert_close(tip.ux.max, displacement, 1e-12);
    assert_close(tip.ux.min, -displacement, 1e-12);
    assert_eq!(tip.ux.max_combination, "base", "ties keep the earlier case");
    assert_eq!(tip.ux.min_combination, "compression");
    assert_close(tip.uy.min, 0.0, 1e-12);
    assert_close(tip.uy.max, 0.0, 1e-12);
    assert_eq!(tip.uy.min_combination, "base");

    let element = &report.elements[0];
    assert_eq!(element.id, "e1");
    assert_close(element.axial.max, LOAD, 1e-12);
    assert_close(element.axial.min, -LOAD, 1e-12);
    assert_eq!(element.axial.max_combination, "base");
    assert_eq!(element.axial.min_combination, "compression");
    assert_close(element.shear.min, 0.0, 1e-12);
    assert_close(element.shear.max, 0.0, 1e-12);
    assert_close(element.moment.min, 0.0, 1e-12);
    assert_close(element.moment.max, 0.0, 1e-12);

    let support = report.reactions.iter().find(|reaction| reaction.node == "n1").expect("fixed reaction");
    assert!(support.fz.is_none(), "two-dimensional reactions omit fz");
    assert_close(support.fx.min, -LOAD, 1e-12);
    assert_close(support.fx.max, LOAD, 1e-12);
    assert_eq!(support.fx.min_combination, "base");
    assert_eq!(support.fx.max_combination, "compression");
}

#[test]
fn three_dimensional_documents_report_uz_and_fz_rows() {
    let mut model = vertical_bar_3d_model();
    model.combinations.push(combination("up", -1.0, 0.0));

    let report = solve_envelope(&model).expect("envelope");
    assert_eq!(report.cases, vec!["base", "up"]);

    let displacement = LOAD * L / (E * A);
    let tip = report.nodes.iter().find(|node| node.id == "n2").expect("tip node");
    let uz = tip.uz.as_ref().expect("three-dimensional nodes report uz");
    assert_close(uz.min, -displacement, 1e-12);
    assert_close(uz.max, displacement, 1e-12);
    assert_eq!(uz.min_combination, "base");
    assert_eq!(uz.max_combination, "up");

    let support = report.reactions.iter().find(|reaction| reaction.node == "n1").expect("fixed reaction");
    let fz = support.fz.as_ref().expect("three-dimensional reactions report fz");
    assert_close(fz.min, -LOAD, 1e-12);
    assert_close(fz.max, LOAD, 1e-12);
    assert_eq!(fz.min_combination, "up");
    assert_eq!(fz.max_combination, "base");
}

#[test]
fn a_document_without_combinations_reports_the_base_case() {
    let model = single_bar_model();
    let report = solve_envelope(&model).expect("base envelope");

    assert_eq!(report.combination_count, 0);
    assert_eq!(report.cases, vec!["base"]);
    assert!(report.ok);

    let displacement = LOAD * L / (E * A);
    let tip = report.nodes.iter().find(|node| node.id == "n2").expect("tip node");
    assert_close(tip.ux.min, displacement, 1e-12);
    assert_close(tip.ux.max, displacement, 1e-12);
    assert_eq!(tip.ux.min_combination, "base");
    assert_eq!(tip.ux.max_combination, "base");

    let element = &report.elements[0];
    assert_close(element.axial.min, LOAD, 1e-12);
    assert_close(element.axial.max, LOAD, 1e-12);
}

#[test]
fn envelope_json_is_deterministic_and_never_writes_the_document() {
    let directory = temporary_dir("envelope-determinism");
    let source = directory.join("model.ocae");
    let mut model = single_bar_model();
    model.combinations.push(combination("tension", 1.0, 0.0));
    model.combinations.push(combination("compression", -1.0, 0.0));
    model.save_atomic(&source).expect("save");
    let before = fs::read(&source).expect("document bytes");

    let first = directory.join("first.json");
    let second = directory.join("second.json");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&source).expect("open");
    session.envelope(Some(&first)).expect("first envelope");
    session.envelope(Some(&second)).expect("second envelope");

    assert_eq!(fs::read(&first).expect("first report"), fs::read(&second).expect("second report"));
    assert_eq!(fs::read(&source).expect("document bytes"), before, "the report must not write the document");
}

#[test]
fn a_failing_combination_propagates_the_solver_code() {
    let mut model = single_bar_model();
    model.combinations.push(LoadCombination {
        name: "missing".to_string(),
        loads: 1.0,
        gravity: 0.0,
        thermal: 1.0,
        patterns: vec![PatternFactor { pattern: "absent".to_string(), factor: 1.0 }],
    });

    let error = solve_envelope(&model).expect_err("an unknown pattern must fail the envelope");
    assert_eq!(error.code, "combination/unknown-pattern");
}
