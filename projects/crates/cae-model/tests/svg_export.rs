//! Integration tests for the deterministic SVG export.
//!
//! The drawing is projected onto the global X–Y plane, so both two- and
//! three-dimensional models are covered, and repeated exports must be
//! byte-identical.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{CaeError, ExportFormat, ExportOptions, Model, OpenCae, OpenCaeOptions, Units, export_svg, solve};

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-svg-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn two_bar_truss(path: &Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.add_node("n1", -500.0, 0.0, None).expect("n1");
    session.add_node("n2", 500.0, 0.0, None).expect("n2");
    session.add_node("n3", 0.0, 500.0, None).expect("n3");
    session.add_element("e1", "n1", "n3", None, None).expect("e1");
    session.add_element("e2", "n2", "n3", None, None).expect("e2");
    session.set_material(210_000.0, 100.0, Some("steel".to_string())).expect("material");
    session.add_support("n1", true, true, false).expect("support n1");
    session.add_support("n2", true, true, false).expect("support n2");
    session.add_load("n3", None, Some(-1000.0), None).expect("load n3");
    session
}

fn three_dimensional_bar(path: &Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(3) }).expect("create");
    session.add_node("a", 0.0, 0.0, Some(0.0)).expect("a");
    session.add_node("b", 1000.0, 800.0, Some(500.0)).expect("b");
    session.add_element("bar", "a", "b", None, None).expect("bar");
    session.set_material(210_000.0, 100.0, Some("steel".to_string())).expect("material");
    session.add_support("a", true, true, true).expect("support a");
    session.add_support("b", false, true, true).expect("support b");
    session.add_load("b", Some(1000.0), None, None).expect("load b");
    session
}

#[test]
fn svg_export_draws_the_model_and_is_byte_identical() {
    let directory = temporary_dir("deterministic");
    let path = directory.join("demo.ocae");
    let session = two_bar_truss(&path);
    let target = directory.join("model.svg");

    let first = session
        .export_with_options(ExportFormat::Svg, &target, ExportOptions { scale: 1.5, ..ExportOptions::default() })
        .expect("first export");
    assert_eq!(first.format, "svg");
    assert_eq!(first.files, vec![target.display().to_string()]);
    let text = fs::read_to_string(&target).expect("model.svg");
    assert!(text.contains("<svg"), "missing <svg>: {text}");
    assert!(text.contains("<line "), "missing undeformed <line>: {text}");
    assert!(text.contains("<polyline"), "missing deformed <polyline>: {text}");
    assert!(text.contains("OpenCAE model and deformed shape"), "missing title: {text}");
    assert!(text.contains("scale 1.50"), "missing scale: {text}");
    assert!(text.contains("<polygon"), "missing support marker: {text}");
    assert!(text.contains("ocae-load-arrow"), "missing load arrow marker: {text}");
    assert!(text.contains("projection: XY (Z ignored)"), "missing projection note: {text}");

    session
        .export_with_options(ExportFormat::Svg, &target, ExportOptions { scale: 1.5, ..ExportOptions::default() })
        .expect("second export");
    let repeated = fs::read_to_string(&target).expect("model.svg");
    assert_eq!(text, repeated, "repeated SVG exports must be byte-identical");

    let report = session.export(ExportFormat::Svg, &directory).expect("directory export");
    assert!(report.files[0].ends_with("model.svg"));
}

#[test]
fn svg_export_projects_three_dimensional_models_onto_xy() {
    let directory = temporary_dir("projection");
    let path = directory.join("space.ocae");
    let session = three_dimensional_bar(&path);
    let target = directory.join("space.svg");

    session.export(ExportFormat::Svg, &target).expect("svg export");
    let text = fs::read_to_string(&target).expect("space.svg");
    assert!(text.contains("<polyline"));
    assert!(text.contains("<polygon"));
    assert!(text.contains("scale 1.00"));
    assert!(text.contains("OpenCAE model and deformed shape"));
}

#[test]
fn svg_export_reports_stable_errors() {
    let directory = temporary_dir("errors");
    let path = directory.join("demo.ocae");
    let session = two_bar_truss(&path);
    let model = Model::load(&path).expect("reload");
    let solution = solve(&model).expect("solve");

    let empty = Model::new(Units::Millimeter);
    let error: CaeError = export_svg(&empty, &solution, 1.0, &directory.join("empty.svg")).expect_err("empty model");
    assert_eq!(error.code, "export/no-solution");

    let error = export_svg(&model, &solution, 0.0, &directory.join("zero.svg")).expect_err("zero scale");
    assert_eq!(error.code, "export/invalid-scale");

    let error = export_svg(&model, &solution, f64::NAN, &directory.join("nan.svg")).expect_err("NaN scale");
    assert_eq!(error.code, "export/invalid-scale");

    let mut mismatched = model.clone();
    mismatched.add_node("extra", 1000.0, 1000.0).expect("extra node");
    let error = export_svg(&mismatched, &solution, 1.0, &directory.join("mismatch.svg")).expect_err("mismatched solution");
    assert_eq!(error.code, "export/mismatch");

    assert!(!directory.join("zero.svg").exists());
    let _ = session;
}
