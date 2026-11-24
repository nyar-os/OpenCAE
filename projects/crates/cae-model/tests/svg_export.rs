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

use cae_model::{
    CaeError, ContourQuantity, ElementKind, ExportFormat, ExportOptions, Model, OpenCae, OpenCaeOptions, Units, export_svg,
    solve,
};

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

/// Cantilever beam fixed at `n1` with a 1000 N downward tip load at `n2`.
fn cantilever(path: &Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", 1000.0, 0.0, None).expect("n2");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, Some(1.0e6), None).expect("beam");
    session.set_material(210_000.0, 100.0, Some("steel".to_string())).expect("material");
    session.add_support_full("n1", true, true, false, true).expect("fixed support");
    session.add_load("n2", None, Some(-1000.0), None).expect("tip load");
    session
}

/// Single 1000 mm bar in 1000 N tension.
fn tension_bar(path: &Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", 1000.0, 0.0, None).expect("n2");
    session.add_element("e1", "n1", "n2", None, None).expect("bar");
    session.set_material(210_000.0, 100.0, Some("steel".to_string())).expect("material");
    session.add_support("n1", true, true, false).expect("support n1");
    session.add_support("n2", false, true, false).expect("support n2");
    session.add_load("n2", Some(1000.0), None, None).expect("tension");
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
fn svg_export_annotates_each_node_id() {
    let directory = temporary_dir("node-labels");
    let path = directory.join("demo.ocae");
    let session = two_bar_truss(&path);
    let target = directory.join("labels.svg");

    session.export(ExportFormat::Svg, &target).expect("svg export");
    let text = fs::read_to_string(&target).expect("labels.svg");
    assert!(text.contains(">n1</text>"), "missing node label n1: {text}");
    assert!(text.contains(">n2</text>"), "missing node label n2: {text}");
    assert!(text.contains(">n3</text>"), "missing node label n3: {text}");

    session.export(ExportFormat::Svg, &target).expect("repeat export");
    let repeated = fs::read_to_string(&target).expect("repeated labels.svg");
    assert_eq!(text, repeated, "repeated exports with node labels must be byte-identical");
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

#[test]
fn svg_moment_contour_reports_the_fixed_end_moment() {
    let directory = temporary_dir("contour-moment");
    let path = directory.join("cantilever.ocae");
    let session = cantilever(&path);
    let target = directory.join("moment.svg");
    let options = ExportOptions { contour: Some(ContourQuantity::Moment), stations: Some(11), ..ExportOptions::default() };

    session.export_with_options(ExportFormat::Svg, &target, options).expect("contour export");
    let text = fs::read_to_string(&target).expect("moment.svg");
    assert!(text.contains("OpenCAE model and contour"), "missing contour title: {text}");
    assert!(text.contains("contour: bending moment (N·mm)"), "missing legend title: {text}");
    // The fixed-end moment of a 1000 N tip load on a 1000 mm cantilever is
    // P·L = 1e6 N·mm; the legend prints the sampled extreme.
    assert!(text.contains("1000000.000"), "missing P·L extreme: {text}");
    assert!(text.contains("<rect"), "missing colour bar: {text}");
    assert!(text.contains("stroke=\"#1d4ed8\"") || text.contains("stroke=\"#b91c1c\""), "missing contour colour: {text}");
    assert!(!text.contains("<polyline"), "a contour drawing replaces the blue deformed polyline");

    session.export_with_options(ExportFormat::Svg, &target, options).expect("repeat contour export");
    let repeated = fs::read_to_string(&target).expect("moment.svg");
    assert_eq!(text, repeated, "repeated contour exports must be byte-identical");
}

#[test]
fn svg_axial_contour_of_a_tension_bar_is_uniform_red() {
    let directory = temporary_dir("contour-axial");
    let path = directory.join("bar.ocae");
    let session = tension_bar(&path);
    let target = directory.join("axial.svg");
    let options = ExportOptions { contour: Some(ContourQuantity::Axial), ..ExportOptions::default() };

    session.export_with_options(ExportFormat::Svg, &target, options).expect("contour export");
    let text = fs::read_to_string(&target).expect("axial.svg");
    assert!(text.contains("contour: axial force (N)"), "missing axial legend: {text}");
    assert!(text.contains("1000.000"), "missing 1000 N axial force: {text}");
    assert!(text.contains("stroke=\"#b91c1c\""), "the maximum must be drawn red: {text}");
}

#[test]
fn svg_displacement_contour_reports_the_bar_extension() {
    let directory = temporary_dir("contour-displacement");
    let path = directory.join("bar.ocae");
    let session = tension_bar(&path);
    let target = directory.join("displacement.svg");
    let options = ExportOptions { contour: Some(ContourQuantity::Displacement), ..ExportOptions::default() };

    session.export_with_options(ExportFormat::Svg, &target, options).expect("contour export");
    let text = fs::read_to_string(&target).expect("displacement.svg");
    assert!(text.contains("contour: displacement (mm)"), "missing displacement legend: {text}");
    // FL/(EA) = 1000·1000/(210000·100) = 0.047619 mm.
    assert!(text.contains("0.0476"), "missing bar extension: {text}");
}

#[test]
fn svg_contour_rejects_unknown_quantities_and_non_svg_formats() {
    let error = "pressure".parse::<ContourQuantity>().expect_err("unknown quantity");
    assert_eq!(error.code, "export/invalid-contour");

    let directory = temporary_dir("contour-errors");
    let path = directory.join("bar.ocae");
    let session = tension_bar(&path);
    let error = session
        .export_with_options(
            ExportFormat::Csv,
            &directory.join("tables"),
            ExportOptions { contour: Some(ContourQuantity::Axial), ..ExportOptions::default() },
        )
        .expect_err("contour on CSV must fail");
    assert_eq!(error.code, "export/invalid-contour");
}
