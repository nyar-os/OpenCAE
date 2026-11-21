//! Integration tests for the path-backed [`cae_model::OpenCae`] session API.
//!
//! The tests drive the same sequence the `ocae` CLI performs: create a file,
//! edit it through the session, validate, solve, export and report.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, ExportFormat, OpenCae, OpenCaeOptions, Severity, Units};

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-session-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() / scale <= relative, "expected {expected}, got {actual} (relative tolerance {relative})");
}

/// Two-bar symmetric truss identical to the example in the Readme.
fn build_two_bar_truss(session: &mut OpenCae, path: &std::path::Path) {
    session.create(path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");

    session.add_node("n1", -500.0, 0.0, None).expect("node n1");
    session.add_node("n2", 500.0, 0.0, None).expect("node n2");
    session.add_node("n3", 0.0, 500.0, None).expect("node n3");

    session.add_element("e1", "n1", "n3", None, None).expect("element e1");
    session.add_element("e2", "n2", "n3", None, None).expect("element e2");

    session.set_material(210_000.0, 100.0, Some("steel".to_string())).expect("material");

    session.add_support("n1", true, true, false).expect("support n1");
    session.add_support("n2", true, true, false).expect("support n2");

    session.add_load("n3", None, Some(-1000.0), None).expect("load n3");
}

#[test]
fn create_edit_solve_export_matches_the_cli_path() {
    let directory = temporary_dir("full");
    let path = directory.join("demo.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());

    build_two_bar_truss(&mut session, &path);

    let summary = session.summary().expect("summary");
    assert_eq!(summary.nodes, 3);
    assert_eq!(summary.elements, 2);
    assert_eq!(summary.supports, 2);
    assert_eq!(summary.loads, 1);
    assert_eq!(summary.dimension, 2);
    assert_eq!(summary.units, Some(Units::Millimeter));
    assert_eq!(summary.material.youngs_modulus, 210_000.0);
    assert!(summary.revision > 0);

    // Validation reports no blocking finding.
    let validation = session.validate().expect("validate");
    assert!(validation.ok, "unexpected issues: {:?}", validation.issues);
    assert!(!validation.issues.iter().any(|issue| issue.severity == Severity::Error));

    // The analytic solution of the Readme example.
    let solution = session.solve(None).expect("solve");
    assert_eq!(solution.format, "open-cae-truss-report");
    assert_close(solution.max_displacement.uy, -0.033_671_8, 1e-5);
    for element in &solution.elements {
        assert_close(element.axial_force, -707.106_78, 1e-5);
    }

    // `solve --out report.json` writes the same JSON atomically.
    let report_path = directory.join("report.json");
    session.solve(Some(&report_path)).expect("solve with out");
    let written = fs::read_to_string(&report_path).expect("report.json");
    assert!(written.contains("open-cae-truss-report"));

    // `export --format csv --out <dir>` writes node, element and reaction tables.
    let csv_directory = directory.join("csv");
    let csv_report = session.export(ExportFormat::Csv, &csv_directory).expect("csv export");
    assert_eq!(csv_report.format, "csv");
    assert_eq!(csv_report.files.len(), 3);
    for file in &csv_report.files {
        assert!(std::path::Path::new(file).is_file(), "missing export file {file}");
    }
    let nodes = fs::read_to_string(csv_directory.join("nodes.csv")).expect("nodes.csv");
    assert!(nodes.starts_with("id,x,y,z,ux,uy,uz"));
    assert_eq!(nodes.lines().count(), 4);

    // `export --format vtk --out <file.vtk>` writes one VTK file.
    let vtk_path = directory.join("result.vtk");
    let vtk_report = session.export(ExportFormat::Vtk, &vtk_path).expect("vtk export");
    assert_eq!(vtk_report.files, vec![vtk_path.display().to_string()]);
    let vtk = fs::read_to_string(&vtk_path).expect("result.vtk");
    assert!(vtk.contains("UNSTRUCTURED_GRID"));
    assert!(vtk.contains("DISPLACEMENT"));

    // `report` carries the summary, issues and solution.
    let report = session.report().expect("report");
    assert_eq!(report.format, "open-cae-truss-report");
    assert_eq!(report.document.elements, 2);
    assert_eq!(report.solution.nodes.len(), 3);

    // Saving in place keeps the same path, and a fresh session can reopen it.
    let saved = session.save(None).expect("save in place");
    assert_eq!(saved.path, path.display().to_string());

    let mut reopened = OpenCae::new(OpenCaeOptions::default());
    let opened = reopened.open(&path).expect("reopen");
    assert_eq!(opened, saved);
}

#[test]
fn session_solves_buckling_and_writes_the_json_report() {
    let directory = temporary_dir("buckling");
    let path = directory.join("column.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.set_material(210_000.0, 100.0, Some("steel".to_string())).expect("material");

    let elements = 10;
    for index in 0..=elements {
        let x = 1000.0 * index as f64 / elements as f64;
        session.add_node(&format!("n{}", index + 1), x, 0.0, None).expect("node");
    }
    for index in 0..elements {
        session
            .add_element_full(
                &format!("e{}", index + 1),
                &format!("n{}", index + 1),
                &format!("n{}", index + 2),
                None,
                None,
                ElementKind::Beam,
                Some(1.0e6),
                None,
            )
            .expect("beam element");
    }
    session.add_support("n1", true, true, false).expect("pin");
    session.add_support(&format!("n{}", elements + 1), false, true, false).expect("lateral support");
    session.add_load(&format!("n{}", elements + 1), Some(-1000.0), None, None).expect("axial load");

    let out = directory.join("buckling.json");
    let result = session.solve_buckling(1, Some(&out)).expect("buckling");
    assert_eq!(result.format, "open-cae-buckling-report");
    assert_eq!(result.requested, 1);
    assert!(!result.modes.is_empty());
    let written = fs::read_to_string(&out).expect("buckling.json");
    assert!(written.contains("open-cae-buckling-report"));
}

#[test]
fn session_prescribes_a_displacement_and_solves_the_settled_model() {
    let directory = temporary_dir("prescribed");
    let path = directory.join("bar.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", 1000.0, 0.0, None).expect("n2");
    session.add_element("e1", "n1", "n2", None, None).expect("element");
    session.add_support("n1", true, true, false).expect("fixed end");
    session.add_support("n2", false, true, false).expect("lateral guide");
    session.prescribe_displacement("n2", Some(0.5), None, None, None).expect("prescribed displacement");

    let solution = session.solve(None).expect("solve");
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 result");
    assert_close(tip.ux, 0.5, 1e-12);
    let expected_force = 210_000.0 * 100.0 * 0.5 / 1000.0;
    let fixed = solution.reactions.iter().find(|reaction| reaction.node == "n1").expect("fixed reaction");
    assert_close(fixed.fx, -expected_force, 1e-12);

    assert_eq!(
        session.prescribe_displacement("n2", None, None, None, None).expect_err("no component").code,
        "support/no-freedom"
    );
}

#[test]
fn session_reports_stable_codes_for_invalid_operations() {
    let directory = temporary_dir("errors");
    let path = directory.join("demo.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());

    // Editing before create/open is a session error.
    assert_eq!(session.solve(None).expect_err("no document").code, "session/no-document");

    session.create(&path, OpenCaeOptions::default()).expect("create");

    // Creating over an existing file is refused instead of truncated.
    let mut other = OpenCae::new(OpenCaeOptions::default());
    assert_eq!(other.create(&path, OpenCaeOptions::default()).expect_err("exists").code, "file/exists");

    // A support must constrain at least one degree of freedom.
    session.add_node("n1", 0.0, 0.0, None).expect("node");
    assert_eq!(session.add_support("n1", false, false, false).expect_err("no freedom").code, "support/no-freedom");

    // Two-dimensional documents reject a non-zero z.
    assert_eq!(session.add_node("n2", 1.0, 0.0, Some(1.0)).expect_err("z in 2d").code, "document/dimension-mismatch");

    // Unknown export formats keep the CLI contract.
    assert_eq!("obj".parse::<ExportFormat>().expect_err("unknown format").code, "cli/unknown-format");
    assert_eq!("step".parse::<ExportFormat>().expect_err("unsupported format").code, "capability/not-implemented");
}
