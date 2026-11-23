use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{
    EnvelopeExtreme, EnvelopeNode, EnvelopeReport, LoadCombination, Model, OpenCae, OpenCaeOptions, PatternFactor, Units,
    envelope_csv, solve_envelope,
};

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
    session.envelope(Some(&first), None).expect("first envelope");
    session.envelope(Some(&second), None).expect("second envelope");

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

const CSV_HEADER: &str = "section,id,component,min,max,minCombination,maxCombination";

#[test]
fn csv_reports_the_header_and_the_json_report_order() {
    let mut model = single_bar_model();
    model.combinations.push(combination("tension", 1.0, 0.0));
    model.combinations.push(combination("compression", -1.0, 0.0));

    let report = solve_envelope(&model).expect("envelope");
    let csv = envelope_csv(&report);
    let rows: Vec<&str> = csv.lines().collect();

    assert_eq!(rows[0], CSV_HEADER);
    assert_eq!(rows.len(), 1 + 2 * 2 + 3 + 2 * 2, "nodes, elements and reactions rows in report order");
    let sections: Vec<Vec<&str>> = rows[1..].iter().map(|row| row.split(',').collect()).collect();
    assert_eq!(sections[0][..3], ["nodes", "n1", "ux"]);
    assert_eq!(sections[1][..3], ["nodes", "n1", "uy"]);
    assert_eq!(sections[2][..3], ["nodes", "n2", "ux"]);
    assert_eq!(sections[3][..3], ["nodes", "n2", "uy"]);
    assert_eq!(sections[4][..3], ["elements", "e1", "axial"]);
    assert_eq!(sections[5][..3], ["elements", "e1", "shear"]);
    assert_eq!(sections[6][..3], ["elements", "e1", "moment"]);
    assert_eq!(sections[7][..3], ["reactions", "n1", "fx"]);
    assert_eq!(sections[8][..3], ["reactions", "n1", "fy"]);
    assert_eq!(sections[9][..3], ["reactions", "n2", "fx"]);
    assert_eq!(sections[10][..3], ["reactions", "n2", "fy"]);
    assert!(!csv.contains(",uz,"), "a two-dimensional document has no uz column");
    assert!(!csv.contains(",fz,"), "a two-dimensional document has no fz column");

    let tip = report.nodes.iter().find(|node| node.id == "n2").expect("tip node");
    assert_eq!(
        rows[3],
        format!(
            "nodes,n2,ux,{},{},{},{}",
            serde_json::to_string(&tip.ux.min).expect("min"),
            serde_json::to_string(&tip.ux.max).expect("max"),
            tip.ux.min_combination,
            tip.ux.max_combination
        )
    );
}

#[test]
fn csv_reports_uz_and_fz_rows_in_three_dimensions() {
    let model = vertical_bar_3d_model();
    let report = solve_envelope(&model).expect("envelope");
    let csv = envelope_csv(&report);
    let rows: Vec<&str> = csv.lines().collect();

    assert_eq!(rows[0], CSV_HEADER);
    assert!(rows.iter().any(|row| row.starts_with("nodes,n1,uz,")), "3D nodes report uz");
    assert!(rows.iter().any(|row| row.starts_with("nodes,n2,uz,")), "3D nodes report uz");
    assert!(rows.iter().any(|row| row.starts_with("reactions,n1,fz,")), "3D reactions report fz");
    assert_eq!(rows.len(), 1 + 2 * 3 + 3 + 2 * 3);
}

#[test]
fn csv_quotes_identifiers_and_combination_names_with_commas_or_quotes() {
    let extreme =
        EnvelopeExtreme { min: -1.0, max: 2.5, min_combination: "a,b".to_string(), max_combination: "say \"hi\"".to_string() };
    let report = EnvelopeReport {
        format: "open-cae-envelope".to_string(),
        version: 1,
        units: "mm".to_string(),
        combination_count: 1,
        cases: vec!["base".to_string(), "a,b".to_string()],
        nodes: vec![EnvelopeNode { id: "n,1".to_string(), ux: extreme.clone(), uy: extreme, uz: None }],
        elements: Vec::new(),
        reactions: Vec::new(),
        ok: true,
    };

    let csv = envelope_csv(&report);
    assert_eq!(
        csv,
        "section,id,component,min,max,minCombination,maxCombination\n\
         nodes,\"n,1\",ux,-1.0,2.5,\"a,b\",\"say \"\"hi\"\"\"\n\
         nodes,\"n,1\",uy,-1.0,2.5,\"a,b\",\"say \"\"hi\"\"\"\n"
    );
    assert!(!csv.contains('\r'), "CSV rows end with LF only");
    assert_eq!(csv.matches('\n').count(), 3, "one LF per header or row");
    assert!(csv.ends_with('\n'));
}

#[test]
fn csv_is_deterministic_and_matches_the_session_output() {
    let directory = temporary_dir("envelope-csv");
    let source = directory.join("model.ocae");
    let mut model = single_bar_model();
    model.combinations.push(combination("tension", 1.0, 0.0));
    model.combinations.push(combination("compression", -1.0, 0.0));
    model.save_atomic(&source).expect("save");

    let first = directory.join("first.csv");
    let second = directory.join("second.csv");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&source).expect("open");
    let report = session.envelope(None, Some(&first)).expect("first csv");
    session.envelope(None, Some(&second)).expect("second csv");

    let first_bytes = fs::read(&first).expect("first csv bytes");
    assert_eq!(first_bytes, fs::read(&second).expect("second csv bytes"));
    assert_eq!(first_bytes, envelope_csv(&report).into_bytes());
}

#[test]
fn a_failing_combination_writes_neither_the_json_report_nor_the_csv() {
    let directory = temporary_dir("envelope-failing");
    let source = directory.join("model.ocae");
    let mut model = single_bar_model();
    model.combinations.push(LoadCombination {
        name: "missing".to_string(),
        loads: 1.0,
        gravity: 0.0,
        thermal: 1.0,
        patterns: vec![PatternFactor { pattern: "absent".to_string(), factor: 1.0 }],
    });
    model.save_atomic(&source).expect("save");

    let json = directory.join("envelope.json");
    let csv = directory.join("envelope.csv");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&source).expect("open");
    let error = session.envelope(Some(&json), Some(&csv)).expect_err("a failing case must fail");

    assert_eq!(error.code, "combination/unknown-pattern");
    assert!(!json.exists(), "a failed solve must not write the JSON report");
    assert!(!csv.exists(), "a failed solve must not write the CSV report");
}
