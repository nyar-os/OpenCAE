use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, OpenCae, OpenCaeOptions, Section};

const I: f64 = 1.0e6;
const L: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-reactions-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn session_with_cantilever(path: &std::path::Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions::default()).expect("create");
    session.apply_material_preset("steel", None, Some(Section::rectangle(10.0, 20.0).expect("section")), None).expect("preset");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, Some(I), None).expect("beam");
    session.add_support_full("n1", true, true, false, true).expect("fixed");
    session.add_load("n2", None, Some(-1000.0), None).expect("load");
    session
}

#[test]
fn reaction_summary_reports_support_reactions_and_equilibrium() {
    let directory = temporary_dir("summary");
    let path = directory.join("model.ocae");
    let session = session_with_cantilever(&path);
    let summary = session.reactions(None).expect("reactions");

    assert_eq!(summary.supports.len(), 1);
    let support = &summary.supports[0];
    assert_eq!(support.node, "n1");
    assert_close(support.fy, 1000.0, 1e-9);
    assert_close(support.mz, 1000.0 * L, 1e-9);
    assert_close(support.fx, 0.0, 1e-12);

    assert_close(summary.applied.fx, 0.0, 1e-12);
    assert_close(summary.applied.fy, -1000.0, 1e-9);
    assert_close(summary.unbalanced.fx, 0.0, 1e-12);
    assert_close(summary.unbalanced.fy, 0.0, 1e-9);
    assert!(summary.balanced, "summary: {summary:?}");
    assert!(summary.tolerance > 0.0);
}

#[test]
fn session_writes_the_reaction_report() {
    let directory = temporary_dir("report");
    let path = directory.join("model.ocae");
    let session = session_with_cantilever(&path);
    let out = directory.join("reactions.json");
    let summary = session.reactions(Some(&out)).expect("reactions");
    assert!(summary.balanced);

    let written = fs::read_to_string(&out).expect("reactions.json");
    assert!(written.contains("\"balanced\": true"), "report: {written}");
    assert!(written.contains("\"node\": \"n1\""), "report: {written}");
}

#[test]
fn csv_export_includes_one_row_per_support() {
    let directory = temporary_dir("csv");
    let path = directory.join("model.ocae");
    let session = session_with_cantilever(&path);
    let csv_directory = directory.join("csv");
    let report = session.export(cae_model::ExportFormat::Csv, &csv_directory).expect("export");
    assert_eq!(report.files.len(), 4);

    let reactions = fs::read_to_string(csv_directory.join("reactions.csv")).expect("reactions.csv");
    let mut lines = reactions.lines();
    assert_eq!(lines.next(), Some("node,fx,fy,fz,mz"));
    let row = lines.next().expect("support row");
    let fields: Vec<&str> = row.split(',').collect();
    assert_eq!(fields[0], "n1");
    assert_close(fields[2].parse::<f64>().expect("fy"), 1000.0, 1e-9);
    assert_close(fields[4].parse::<f64>().expect("mz"), 1000.0 * L, 1e-9);
    assert_eq!(lines.count(), 0);
}

#[test]
fn solution_keeps_the_reaction_summary() {
    let directory = temporary_dir("solution");
    let path = directory.join("model.ocae");
    let session = session_with_cantilever(&path);
    let solution = session.solve(None).expect("solve");
    assert_eq!(solution.reaction_summary.supports.len(), 1);
    assert!(solution.reaction_summary.balanced);
}
