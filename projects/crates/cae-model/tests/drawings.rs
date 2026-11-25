//! Integration tests for the deterministic SVG engineering drawings.
//!
//! Each drawing kind is checked against analytic section forces or nodal
//! results, so the labels printed in the file are the numbers the solver
//! actually produced, and repeated exports must be byte-identical.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{
    ContourQuantity, DrawingKind, ElementKind, ExportFormat, ExportOptions, Model, OpenCae, OpenCaeOptions, Units,
    export_displacement_svg, export_reactions_svg, solve, solve_internal_forces,
};

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-drawing-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn save_session(model: &Model, directory: &Path, name: &str) -> OpenCae {
    let path = directory.join(format!("{name}.ocae"));
    model.save_atomic(&path).expect("save document");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open document");
    session
}

/// Single 1000 mm beam with a uniform -2 N/mm load on simple supports.
/// Analytic: `V` runs linearly from +1000 N to -1000 N with a zero crossing
/// at midspan, `M` peaks at `qL²/8 = 250000 N·mm` at midspan.
fn uniformly_loaded_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(1.0e6), Some(-2.0)).expect("beam");
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n2", false, true, false, false).expect("roller");
    model
}

/// Two 500 mm beam halves carrying a 1000 N centre point load.
/// Analytic: `|V| = P/2 = 500 N` on both halves, `M` peaks at `PL/4 =
/// 250000 N·mm` under the load.
fn centre_loaded_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 500.0, 0.0).expect("n2");
    model.add_node("n3", 1000.0, 0.0).expect("n3");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(1.0e6), None).expect("b1");
    model.add_element_kind("b2", "n2", "n3", None, ElementKind::Beam, Some(1.0e6), None).expect("b2");
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n3", false, true, false, false).expect("roller");
    model.add_load("n2", None, Some(-1000.0)).expect("centre load");
    model
}

fn forces_options() -> ExportOptions {
    ExportOptions { drawing: Some(DrawingKind::Forces), ..ExportOptions::default() }
}

fn displacement_options() -> ExportOptions {
    ExportOptions { drawing: Some(DrawingKind::Displacement), ..ExportOptions::default() }
}

fn reactions_options() -> ExportOptions {
    ExportOptions { drawing: Some(DrawingKind::Reactions), ..ExportOptions::default() }
}

/// Two 45° truss bars with a 1000 N downward apex load on pinned supports.
/// Analytic: each bar carries -707.1068 N and each support reaction has
/// magnitude `sqrt(500² + 500²) = 707.1 N`.
fn two_bar_truss() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", -500.0, 0.0).expect("n1");
    model.add_node("n2", 500.0, 0.0).expect("n2");
    model.add_node("n3", 0.0, 500.0).expect("n3");
    model.add_element("e1", "n1", "n3", None).expect("e1");
    model.add_element("e2", "n2", "n3", None).expect("e2");
    model.set_material(Some("steel".to_string()), 210_000.0, 100.0);
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n2", true, true, false, false).expect("pin");
    model.add_load("n3", None, Some(-1000.0)).expect("apex load");
    model
}

#[test]
fn reactions_drawing_labels_the_truss_supports() {
    let directory = temporary_dir("reactions-truss");
    let model = two_bar_truss();
    let session = save_session(&model, &directory, "truss");

    let report =
        session.export_with_options(ExportFormat::Svg, &directory, reactions_options()).expect("reactions drawing of a truss");
    let target = directory.join("reactions.svg");
    assert_eq!(report.files, vec![target.display().to_string()]);
    let text = fs::read_to_string(&target).expect("reactions.svg");
    assert!(text.contains("OpenCAE support reaction vectors"), "missing title: {text}");
    // Each pinned support carries |R| = sqrt(500² + 500²) = 707.1 N.
    assert!(text.contains("max |R| = 707.1 N"), "missing the legend maximum: {text}");
    assert_eq!(text.matches(">707.1 N</text>").count(), 2, "one magnitude label per support: {text}");
    assert_eq!(text.matches("marker-end=\"url(#ocae-reaction-arrow)\"").count(), 2, "one arrow per support: {text}");

    session.export_with_options(ExportFormat::Svg, &directory, reactions_options()).expect("repeat export");
    let repeated = fs::read_to_string(&target).expect("repeated reactions.svg");
    assert_eq!(text, repeated, "repeated reaction drawings must be byte-identical");
}

#[test]
fn reactions_drawing_labels_the_cantilever_fixed_end() {
    let directory = temporary_dir("reactions-cantilever");
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(1.0e6), None).expect("beam");
    model.set_material(Some("steel".to_string()), 210_000.0, 100.0);
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.add_load("n2", None, Some(-1000.0)).expect("tip load");
    let session = save_session(&model, &directory, "cantilever");
    let target = directory.join("reactions.svg");

    session.export_with_options(ExportFormat::Svg, &target, reactions_options()).expect("reactions export");
    let text = fs::read_to_string(&target).expect("reactions.svg");
    // Analytic fixed-end reaction: |R| = 1000 N and |Mz| = P·L = 1.000e6 N·mm.
    assert!(text.contains(">1000 N</text>"), "missing the force magnitude label: {text}");
    assert!(text.contains("1.000e6 N·mm"), "missing the reaction moment label: {text}");
    assert_eq!(text.matches("marker-end=\"url(#ocae-reaction-arrow)\"").count(), 1, "one arrow at the fixed end: {text}");
}

#[test]
fn reactions_drawing_rejects_an_empty_model() {
    let directory = temporary_dir("reactions-empty");
    let model = two_bar_truss();
    let solution = solve(&model).expect("solve");
    let empty = Model::new(Units::Millimeter);
    let error = export_reactions_svg(&empty, &solution, &directory.join("reactions.svg")).expect_err("empty model");
    assert_eq!(error.code, "export/no-solution");
}

#[test]
fn reactions_drawing_rejects_a_model_without_supports() {
    let directory = temporary_dir("reactions-unsupported");
    let model = two_bar_truss();
    let solution = solve(&model).expect("solve");
    let mut unsupported = Model::new(Units::Millimeter);
    unsupported.add_node("n1", -500.0, 0.0).expect("n1");
    unsupported.add_node("n2", 500.0, 0.0).expect("n2");
    unsupported.add_node("n3", 0.0, 500.0).expect("n3");
    unsupported.add_element("e1", "n1", "n3", None).expect("e1");
    unsupported.add_element("e2", "n2", "n3", None).expect("e2");
    unsupported.set_material(Some("steel".to_string()), 210_000.0, 100.0);
    let error = export_reactions_svg(&unsupported, &solution, &directory.join("reactions.svg")).expect_err("no supports");
    assert_eq!(error.code, "export/no-reactions");
}

#[test]
fn displacement_drawing_labels_the_tension_bar_tip() {
    let directory = temporary_dir("displacement-bar");
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.set_material(Some("steel".to_string()), 210_000.0, 100.0);
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n2", false, true, false, false).expect("guide");
    model.add_load("n2", Some(10_000.0), None).expect("tension");
    let session = save_session(&model, &directory, "bar");
    let target = directory.join("displacements.svg");

    let report = session.export_with_options(ExportFormat::Svg, &target, displacement_options()).expect("displacement export");
    assert_eq!(report.files, vec![target.display().to_string()]);
    let text = fs::read_to_string(&target).expect("displacements.svg");
    assert!(text.contains("OpenCAE nodal displacement vectors"), "missing title: {text}");
    // Analytic tip displacement u = FL/EA = 10000·1000/(210000·100) = 0.4762 mm.
    assert!(text.contains("0.4762"), "missing the tip magnitude label: {text}");
    assert!(text.contains("max |u| = 0.4762"), "missing the legend maximum: {text}");
    assert!(text.contains("mm"), "missing the displacement unit: {text}");
    assert!(text.contains("deformation × 1.00"), "missing the scale note: {text}");

    session.export_with_options(ExportFormat::Svg, &target, displacement_options()).expect("repeat export");
    let repeated = fs::read_to_string(&target).expect("repeated displacements.svg");
    assert_eq!(text, repeated, "repeated displacement drawings must be byte-identical");
}

#[test]
fn displacement_drawing_labels_the_cantilever_tip() {
    let directory = temporary_dir("displacement-cantilever");
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(1.0e6), None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.add_load("n2", None, Some(-1000.0)).expect("tip load");
    let session = save_session(&model, &directory, "cantilever");
    let target = directory.join("displacements.svg");

    session.export_with_options(ExportFormat::Svg, &target, displacement_options()).expect("displacement export");
    let text = fs::read_to_string(&target).expect("displacements.svg");
    // Analytic tip deflection u = PL³/(3EI) = 1000·10⁹/(3·210000·10⁶) = 1.587 mm.
    assert!(text.contains("max |u| = 1.587"), "missing the tip magnitude label: {text}");
    // The fixed support has no arrow.
    assert_eq!(text.matches("marker-end=\"url(#ocae-displacement-arrow)\"").count(), 1, "one arrow for the free tip: {text}");
}

#[test]
fn displacement_drawing_rejects_an_empty_model() {
    let directory = temporary_dir("displacement-empty");
    let model = uniformly_loaded_beam();
    let solution = solve(&model).expect("solve");
    let empty = Model::new(Units::Millimeter);
    let error = export_displacement_svg(&empty, &solution, 1.0, &directory.join("displacements.svg")).expect_err("empty model");
    assert_eq!(error.code, "export/no-solution");
}

#[test]
fn displacement_drawing_rejects_invalid_scale() {
    let directory = temporary_dir("displacement-scale");
    let model = uniformly_loaded_beam();
    let session = save_session(&model, &directory, "beam");
    let error = session
        .export_with_options(
            ExportFormat::Svg,
            &directory.join("displacements.svg"),
            ExportOptions { drawing: Some(DrawingKind::Displacement), scale: 0.0, ..ExportOptions::default() },
        )
        .expect_err("zero scale must be rejected");
    assert_eq!(error.code, "export/invalid-scale");
}

#[test]
fn forces_drawing_reports_the_uniform_load_diagram() {
    let directory = temporary_dir("forces-uniform");
    let model = uniformly_loaded_beam();
    let session = save_session(&model, &directory, "uniform");
    let target = directory.join("forces.svg");

    let report = session.export_with_options(ExportFormat::Svg, &target, forces_options()).expect("forces drawing export");
    assert_eq!(report.files, vec![target.display().to_string()]);
    let text = fs::read_to_string(&target).expect("forces.svg");
    assert!(text.contains("OpenCAE shear and moment diagrams"), "missing title: {text}");
    // Analytic extremes: |V|max = qL/2 = 1000 N at the left support,
    // |M|max = qL²/8 = 250000 N·mm at midspan.
    assert!(text.contains("max |V| = 1000 @ x=0"), "missing shear extreme: {text}");
    assert!(text.contains("max |M| = 250000 @ x=500"), "missing moment extreme: {text}");
    // The zero-shear point sits under the midspan maximum moment.
    assert!(text.contains("V=0 @ x=500"), "missing zero-shear marker: {text}");
    assert!(text.contains("sagging positive"), "missing sign convention: {text}");
    assert!(text.contains("M(x) [N·mm]"), "missing moment axis unit: {text}");

    session.export_with_options(ExportFormat::Svg, &target, forces_options()).expect("repeat export");
    let repeated = fs::read_to_string(&target).expect("repeated forces.svg");
    assert_eq!(text, repeated, "repeated forces drawings must be byte-identical");
}

#[test]
fn forces_drawing_reports_the_centre_load_peak_at_pl_over_four() {
    let directory = temporary_dir("forces-centre");
    let model = centre_loaded_beam();
    let session = save_session(&model, &directory, "centre");
    let target = directory.join("forces.svg");

    session.export_with_options(ExportFormat::Svg, &target, forces_options()).expect("forces drawing export");
    let text = fs::read_to_string(&target).expect("forces.svg");
    // One panel per beam element, both peaking at PL/4 = 250000 N·mm.
    assert_eq!(text.matches("max |M| = 250000 @ x=").count(), 2, "PL/4 must appear on both panels: {text}");
    assert_eq!(text.matches("max |V| = 500 @ x=").count(), 2, "P/2 must appear on both panels: {text}");
    assert_eq!(text.matches("<svg").count(), 1, "one multi-panel file: {text}");
}

#[test]
fn forces_drawing_extremes_match_the_sampled_diagram() {
    let model = uniformly_loaded_beam();
    let diagrams = solve_internal_forces(&model, 11).expect("internal forces");
    let element = &diagrams.elements[0];
    assert!(element.stations.iter().any(|station| station.shear > 999.0 && station.shear < 1001.0));
    assert!(element.max_abs_moment > 249_000.0, "sampled moment peak: {}", element.max_abs_moment);
    let solution = solve(&model).expect("solve");
    assert!(solution.reaction_summary.balanced, "the reactions must close equilibrium");
}

#[test]
fn forces_drawing_rejects_models_without_beam_members() {
    let directory = temporary_dir("forces-truss");
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("support");
    model.add_support("n2", false, true).expect("guide");
    model.add_load("n2", Some(1000.0), None).expect("load");
    let session = save_session(&model, &directory, "truss");
    let error = session
        .export_with_options(ExportFormat::Svg, &directory.join("forces.svg"), forces_options())
        .expect_err("a truss has no diagram");
    assert_eq!(error.code, "export/no-beam-diagram");
    assert!(!directory.join("forces.svg").exists());
}

#[test]
fn drawing_option_reports_stable_errors() {
    let error = "pressure".parse::<DrawingKind>().expect_err("unknown kind");
    assert_eq!(error.code, "export/invalid-drawing");

    let directory = temporary_dir("drawing-errors");
    let model = uniformly_loaded_beam();
    let session = save_session(&model, &directory, "beam");

    let error = session
        .export_with_options(ExportFormat::Csv, &directory.join("csv"), forces_options())
        .expect_err("drawing on CSV must fail");
    assert_eq!(error.code, "export/invalid-drawing");

    let error = session
        .export_with_options(
            ExportFormat::Svg,
            &directory.join("both.svg"),
            ExportOptions {
                drawing: Some(DrawingKind::Forces),
                contour: Some(ContourQuantity::Moment),
                ..ExportOptions::default()
            },
        )
        .expect_err("drawing and contour cannot combine");
    assert_eq!(error.code, "export/invalid-drawing");
    assert!(!directory.join("both.svg").exists());

    // A directory target picks the documented default file name.
    let report = session.export_with_options(ExportFormat::Svg, &directory, forces_options()).expect("directory export");
    assert!(report.files[0].ends_with("forces.svg"), "unexpected default name: {}", report.files[0]);
}
