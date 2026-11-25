//! Integration tests for the deterministic SVG export.
//!
//! The drawing is projected onto the global X–Y plane, with the Z axis drawn
//! as an oblique axis in three-dimensional documents, so both model kinds are
//! covered and repeated exports must be byte-identical.

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
    assert!(
        text.contains("stroke=\"#9ca3af\" stroke-width=\"1.2\" stroke-dasharray=\"4 3\""),
        "the undeformed members must be dashed so they stay visible under the deformed shape: {text}"
    );
    assert!(text.contains("OpenCAE model and deformed shape"), "missing title: {text}");
    assert!(text.contains("scale 1.50"), "missing scale: {text}");
    assert!(text.contains("<polygon"), "missing support marker: {text}");
    assert!(text.contains("ocae-load-arrow"), "missing load arrow marker: {text}");
    assert!(text.contains("projection: XY</text>"), "missing projection note: {text}");

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

/// Portal frame with two pure-Z columns, an X beam, a Y arm and a Z tip load.
fn space_frame(path: &Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(3) }).expect("create");
    session.add_node("b1", 0.0, 0.0, Some(0.0)).expect("b1");
    session.add_node("b2", 4000.0, 0.0, Some(0.0)).expect("b2");
    session.add_node("t1", 0.0, 0.0, Some(3000.0)).expect("t1");
    session.add_node("t2", 4000.0, 0.0, Some(3000.0)).expect("t2");
    session.add_node("a1", 4000.0, 2000.0, Some(3000.0)).expect("a1");
    session.set_material(210_000.0, 10_000.0, Some("steel".to_string())).expect("material");
    for (id, from, to) in [("col1", "b1", "t1"), ("col2", "b2", "t2"), ("beam1", "t1", "t2"), ("arm", "t2", "a1")] {
        session
            .add_frame_element(id, from, to, None, None, Some(8.3e7), None, Some(1.6e7), None, None, None, None, None, None)
            .expect(id);
    }
    session.add_support_frame("b1", true, true, true, true, true, true).expect("support b1");
    session.add_support_frame("b2", true, true, true, true, true, true).expect("support b2");
    session.add_load_frame("a1", None, None, Some(-20_000.0), None, None, None).expect("load a1");
    session
}

/// Extracts the endpoint coordinates of every undeformed dashed member line.
fn dashed_member_lines(text: &str) -> Vec<(f64, f64, f64, f64)> {
    let attribute = |line: &str, name: &str| -> f64 {
        let needle = format!("{name}=\"");
        let start = line.find(&needle).unwrap_or_else(|| panic!("missing {name} in {line}")) + needle.len();
        let end = line[start..].find('"').expect("closing quote");
        line[start..start + end].parse().expect("coordinate")
    };

    text.lines()
        .filter(|line| line.starts_with("<line ") && line.contains("stroke-dasharray") && attribute(line, "y1") < 660.0)
        .map(|line| (attribute(line, "x1"), attribute(line, "y1"), attribute(line, "x2"), attribute(line, "y2")))
        .collect()
}

/// Extracts the `x`/`y` anchor of the label drawn for a node id.
fn node_label(text: &str, id: &str) -> (f64, f64) {
    let needle = format!(">{id}</text>");
    let line = text.lines().find(|line| line.contains(&needle)).unwrap_or_else(|| panic!("missing label {id}"));
    let attribute = |name: &str| -> f64 {
        let needle = format!("{name}=\"");
        let start = line.find(&needle).expect(name) + needle.len();
        let end = line[start..].find('"').expect("closing quote");
        line[start..start + end].parse().expect("coordinate")
    };
    (attribute("x"), attribute("y"))
}

#[test]
fn svg_export_projects_three_dimensional_models_obliquely() {
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
    assert!(text.contains("projection: XY, oblique Z (30°, 0.5)"), "missing oblique note: {text}");
}

#[test]
fn svg_export_keeps_pure_z_members_visible_in_frames() {
    let directory = temporary_dir("frame3d");
    let path = directory.join("frame.ocae");
    let session = space_frame(&path);
    let target = directory.join("frame.svg");

    session
        .export_with_options(ExportFormat::Svg, &target, ExportOptions { scale: 200.0, ..ExportOptions::default() })
        .expect("svg export");
    let text = fs::read_to_string(&target).expect("frame.svg");
    assert!(text.contains("projection: XY, oblique Z (30°, 0.5)"), "missing oblique note: {text}");

    // One undeformed dashed line per member, and no member collapses to a
    // point: the pure-Z columns must be drawn as inclined lines.
    let lines = dashed_member_lines(&text);
    assert_eq!(lines.len(), 4, "one undeformed line per member");
    for &(x1, y1, x2, y2) in &lines {
        assert!(x1 != x2 || y1 != y2, "a member degenerated to a point: {x1},{y1} -> {x2},{y2}");
    }

    // Nodes at different heights no longer share a screen position, so their
    // labels do not overprint.
    assert_ne!(node_label(&text, "b1"), node_label(&text, "t1"), "b1 and t1 labels overlap");
    assert_ne!(node_label(&text, "b2"), node_label(&text, "t2"), "b2 and t2 labels overlap");

    // A pure Z nodal load gets an arrow: one in the drawing plus the legend.
    assert_eq!(text.matches("marker-end=\"url(#ocae-load-arrow)\"").count(), 2, "missing the Z load arrow");
    assert_eq!(text.matches("<circle").count(), 5, "one dot per node");

    // Every drawn coordinate stays inside the fixed viewBox.
    for line in text.lines().filter(|line| line.starts_with("<line ") || line.starts_with("<circle")) {
        for name in ["x1", "x2", "cx"] {
            let needle = format!("{name}=\"");
            if let Some(value) = line
                .find(&needle)
                .map(|start| &line[start + needle.len()..])
                .and_then(|rest| rest.find('"').map(|end| rest[..end].parse::<f64>().expect("finite x")))
            {
                assert!((0.0..=960.0).contains(&value), "x coordinate {value} outside the viewBox");
            }
        }
        for name in ["y1", "y2", "cy"] {
            let needle = format!("{name}=\"");
            if let Some(value) = line
                .find(&needle)
                .map(|start| &line[start + needle.len()..])
                .and_then(|rest| rest.find('"').map(|end| rest[..end].parse::<f64>().expect("finite y")))
            {
                assert!((0.0..=720.0).contains(&value), "y coordinate {value} outside the viewBox");
            }
        }
    }

    // Contour exports of the same frame stay deterministic.
    let contour = directory.join("frame-disp.svg");
    let options = ExportOptions { scale: 200.0, contour: Some(ContourQuantity::Displacement), ..ExportOptions::default() };
    session.export_with_options(ExportFormat::Svg, &contour, options).expect("contour export");
    let first = fs::read_to_string(&contour).expect("frame-disp.svg");
    session.export_with_options(ExportFormat::Svg, &contour, options).expect("repeat contour export");
    let repeated = fs::read_to_string(&contour).expect("repeated frame-disp.svg");
    assert_eq!(first, repeated, "repeated 3D contour exports must be byte-identical");
    assert!(first.contains("contour: displacement (mm)"), "missing displacement legend: {first}");
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
    // P·L = 1e6 N·mm; the legend prints the sampled extreme with four
    // significant digits in scientific notation.
    assert!(text.contains("-1.000e6"), "missing P·L extreme: {text}");
    // A varying field keeps the gradient bar: the white page background plus
    // twelve colour steps.
    assert_eq!(text.matches("<rect").count(), 13, "a varying contour keeps the twelve-step bar: {text}");
    assert!(text.contains("font-size=\"11\">-1.000e6</text>"), "missing sampled minimum label: {text}");
    assert!(text.contains("text-anchor=\"end\">0</text>"), "missing sampled maximum label: {text}");
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
    assert!(text.contains("uniform 1000</text>"), "a constant field must be labelled uniform: {text}");
    assert_eq!(text.matches("uniform 1000").count(), 1, "the uniform value must be printed once: {text}");
    // The page background plus one solid swatch; a uniform field has no gradient.
    assert_eq!(text.matches("<rect").count(), 2, "a uniform contour needs a single swatch: {text}");
    assert!(text.contains("stroke=\"#b91c1c\""), "the maximum must be drawn red: {text}");
}

#[test]
fn svg_uniform_axial_contour_labels_the_compression_once() {
    let directory = temporary_dir("contour-uniform");
    let path = directory.join("truss.ocae");
    let session = two_bar_truss(&path);
    let target = directory.join("axial.svg");
    let options = ExportOptions { contour: Some(ContourQuantity::Axial), ..ExportOptions::default() };

    session.export_with_options(ExportFormat::Svg, &target, options).expect("contour export");
    let text = fs::read_to_string(&target).expect("axial.svg");
    assert!(text.contains("uniform -707.1</text>"), "missing uniform compression label: {text}");
    assert_eq!(text.matches("-707.1").count(), 1, "the two endpoints must not repeat the same value: {text}");
    assert_eq!(text.matches("<rect").count(), 2, "a uniform contour needs a single swatch: {text}");
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
