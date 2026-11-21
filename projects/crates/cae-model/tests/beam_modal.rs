use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, ExportFormat, Model, OpenCae, OpenCaeOptions, Units, solve, solve_buckling, solve_modal};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;
const RHO: f64 = 7.85e-9;

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

fn beam_model(elements: usize) -> Model {
    let mut model = Model::new(Units::Millimeter);
    for index in 0..=elements {
        let x = L * index as f64 / elements as f64;
        model.add_node(format!("n{}", index + 1), x, 0.0).expect("node");
    }
    for index in 0..elements {
        model
            .add_element_kind(
                format!("e{}", index + 1),
                format!("n{}", index + 1),
                format!("n{}", index + 2),
                None,
                ElementKind::Beam,
                Some(I),
                None,
            )
            .expect("beam element");
    }
    model.add_support_full("n1", true, true, false, true).expect("fixed support");
    model
}

fn cantilever_tip_load() -> Model {
    let mut model = beam_model(1);
    model.add_load("n2", None, Some(-1000.0)).expect("tip load");
    model
}

fn cantilever_uniform_load() -> Model {
    let mut model = beam_model(1);
    model.elements[0].distributed_load = Some(-2.0);
    model
}

/// Pinned-pinned column loaded by an axial compressive force at the free
/// translation of the far end.
fn pinned_pinned_column(elements: usize) -> Model {
    let mut model = Model::new(Units::Millimeter);
    for index in 0..=elements {
        let x = L * index as f64 / elements as f64;
        model.add_node(format!("n{}", index + 1), x, 0.0).expect("node");
    }
    for index in 0..elements {
        model
            .add_element_kind(
                format!("e{}", index + 1),
                format!("n{}", index + 1),
                format!("n{}", index + 2),
                None,
                ElementKind::Beam,
                Some(I),
                None,
            )
            .expect("beam element");
    }
    model.add_support("n1", true, true).expect("pinned support");
    model.add_support(format!("n{}", elements + 1), false, true).expect("lateral support");
    model.add_load(format!("n{}", elements + 1), Some(-1000.0), None).expect("axial load");
    model
}

fn simply_supported_center_load() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L / 2.0, 0.0).expect("n2");
    model.add_node("n3", L, 0.0).expect("n3");
    model.add_element_kind("e1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("e1");
    model.add_element_kind("e2", "n2", "n3", None, ElementKind::Beam, Some(I), None).expect("e2");
    model.add_support_full("n1", true, true, false, false).expect("pin");
    model.add_support_full("n3", false, true, false, false).expect("roller");
    model.add_load("n2", None, Some(-1000.0)).expect("center load");
    model
}

#[test]
fn cantilever_tip_load_matches_analytic_deflection_and_reaction_moment() {
    let model = cantilever_tip_load();
    let solution = solve(&model).expect("solve");

    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("tip");
    assert_close(tip.uy, -1000.0 * L.powi(3) / (3.0 * E * I), 1e-9);
    assert_close(tip.rz, -1000.0 * L.powi(2) / (2.0 * E * I), 1e-9);

    let reaction = solution.reactions.iter().find(|reaction| reaction.node == "n1").expect("reaction");
    assert_close(reaction.mz, 1000.0 * L, 1e-9);
    assert_close(reaction.fy, 1000.0, 1e-9);
}

#[test]
fn cantilever_uniform_load_matches_analytic_deflection() {
    let model = cantilever_uniform_load();
    let solution = solve(&model).expect("solve");

    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("tip");
    assert_close(tip.uy, -2.0 * L.powi(4) / (8.0 * E * I), 1e-9);
}

#[test]
fn simply_supported_center_load_matches_analytic_deflection() {
    let model = simply_supported_center_load();
    let solution = solve(&model).expect("solve");

    let center = solution.nodes.iter().find(|node| node.id == "n2").expect("center");
    assert_close(center.uy, -1000.0 * L.powi(3) / (48.0 * E * I), 1e-9);
    assert_close(solution.elements[0].max_abs_moment, 1000.0 * L / 4.0, 1e-9);
}

#[test]
fn fixed_free_bar_fundamental_frequency_matches_analytic_within_three_percent() {
    let elements = 10;
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(Some("steel".to_string()), E, A, RHO);
    for index in 0..=elements {
        let x = L * index as f64 / elements as f64;
        model.add_node(format!("n{}", index + 1), x, 0.0).expect("node");
        model.add_support(format!("n{}", index + 1), false, true).expect("lateral guide");
    }
    for index in 0..elements {
        model.add_element(format!("e{}", index + 1), format!("n{}", index + 1), format!("n{}", index + 2), None).expect("bar");
    }
    model.add_support("n1", true, true).expect("fixed end");

    let expected = (1.0 / (4.0 * L)) * (E / RHO).sqrt();
    let modal = solve_modal(&model, 1).expect("modal");
    let frequency = modal.modes.first().expect("first mode").frequency_hz;
    assert_close(frequency, expected, 0.03);
}

#[test]
fn cantilever_beam_first_bending_frequency_matches_analytic_within_five_percent() {
    let mut model = beam_model(40);
    model.set_material_full(Some("steel".to_string()), E, A, RHO);

    let expected = 1.875_104_f64.powi(2) * (E * I / (RHO * A * L.powi(4))).sqrt() / (2.0 * std::f64::consts::PI);
    let modal = solve_modal(&model, 3).expect("modal");
    let frequency = modal.modes.first().expect("first mode").frequency_hz;
    assert_close(frequency, expected, 0.05);
}

#[test]
fn self_weight_reactions_sum_to_total_weight() {
    let elements = 10;
    let gravity = 9.81;
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(Some("steel".to_string()), E, A, RHO);
    for index in 0..=elements {
        let y = L * index as f64 / elements as f64;
        model.add_node(format!("n{}", index + 1), 0.0, y).expect("node");
        model.add_support(format!("n{}", index + 1), true, false).expect("lateral guide");
    }
    for index in 0..elements {
        model.add_element(format!("e{}", index + 1), format!("n{}", index + 1), format!("n{}", index + 2), None).expect("bar");
    }
    model.add_support("n1", true, true).expect("base");
    model.set_gravity(0.0, -gravity, 0.0).expect("gravity");

    let solution = solve(&model).expect("solve");
    let reaction_sum: f64 = solution.reactions.iter().map(|reaction| reaction.fy).sum();
    let weight = RHO * A * L * gravity;
    assert_close(reaction_sum, weight, 1e-9);
}

#[test]
fn pinned_pinned_column_buckling_matches_euler_load() {
    let model = pinned_pinned_column(20);
    let reference = solve(&model).expect("static reference");
    for element in &reference.elements {
        assert_close(element.axial_force, -1000.0, 1e-9);
    }

    let expected = std::f64::consts::PI.powi(2) * E * I / (L.powi(2) * 1000.0);
    let buckling = solve_buckling(&model, 2).expect("buckling");
    let first = buckling.modes.first().expect("first buckling mode");
    assert_close(first.factor, expected, 1e-3);
    assert!(
        first.shape.iter().any(|node| node.uy.abs() > 0.1),
        "the first buckling mode must bend laterally: {:?}",
        first.shape
    );
}

/// Three equally spaced legs from a fixed triangular base to one loaded apex.
fn tetrahedron_column() -> Model {
    let radius = 500.0;
    let offset = 0.866_025_403_784_438_6 * radius;
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, radius, 0.0).expect("n1");
    model.add_node_3d("n2", -offset, -0.5 * radius, 0.0).expect("n2");
    model.add_node_3d("n3", offset, -0.5 * radius, 0.0).expect("n3");
    model.add_node_3d("n4", 0.0, 0.0, 500.0).expect("apex");
    model.add_element("e1", "n1", "n4", None).expect("e1");
    model.add_element("e2", "n2", "n4", None).expect("e2");
    model.add_element("e3", "n3", "n4", None).expect("e3");
    model.add_support_3d("n1", true, true, true).expect("base n1");
    model.add_support_3d("n2", true, true, true).expect("base n2");
    model.add_support_3d("n3", true, true, true).expect("base n3");
    model.add_load_3d("n4", None, None, Some(-1000.0)).expect("apex load");
    model
}

#[test]
fn three_dimensional_truss_buckling_matches_analytic_tetrahedron() {
    let model = tetrahedron_column();
    let radius: f64 = 500.0;
    let height: f64 = 500.0;
    let leg_length = radius.hypot(height);
    let axial_force = 1000.0 * leg_length / (3.0 * height);
    let expected = E * A * radius.powi(2) / (axial_force * (radius.powi(2) + 2.0 * height.powi(2)));

    let buckling = solve_buckling(&model, 2).expect("3D truss buckling");
    let first = buckling.modes.first().expect("first buckling mode");
    let second = buckling.modes.get(1).expect("second buckling mode");
    assert_close(first.factor, expected, 1e-8);
    assert_close(second.factor, expected, 1e-8);
    assert!(
        first.shape.iter().any(|node| node.ux.abs() + node.uy.abs() > 0.1),
        "the critical tetrahedron mode must be lateral: {:?}",
        first.shape
    );
}

#[test]
fn beam_in_three_dimensions_is_rejected() {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", 1000.0, 0.0, 0.0).expect("n2");
    let error =
        model.add_element_kind("e1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect_err("3D beam must be rejected");
    assert_eq!(error.code, "capability/not-implemented");
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn distributed_load_on_truss_is_rejected() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    let error = model
        .add_element_kind("e1", "n1", "n2", None, ElementKind::Truss, None, Some(-1.0))
        .expect_err("truss distributed load must be rejected");
    assert_eq!(error.code, "element/distributed-load-requires-beam");
}

#[test]
fn modal_requires_positive_density() {
    let model = beam_model(2);
    let error = solve_modal(&model, 1).expect_err("density zero must be rejected");
    assert_eq!(error.code, "material/invalid-density");
}

#[test]
fn markdown_report_is_deterministic_and_contains_all_sections() {
    let directory = temporary_dir("md");
    let path = directory.join("model.ocae");
    let mut model = cantilever_tip_load();
    model.set_material_full(Some("steel".to_string()), E, A, RHO);
    model.save_atomic(&path).expect("save");

    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open");
    let first = session.export(ExportFormat::Md, &directory).expect("first export");
    let first_report = fs::read_to_string(&first.files[0]).expect("report");
    let second = session.export(ExportFormat::Md, &directory).expect("second export");
    let second_report = fs::read_to_string(&second.files[0]).expect("report");
    assert_eq!(first_report, second_report);
    for section in ["## Model Summary", "## Validation", "## Solution Summary", "## Nodal Displacements", "## Modal Results"] {
        assert!(first_report.contains(section), "missing {section}");
    }
    assert!(first_report.contains("beam"));
    assert!(first_report.contains("Frequency (Hz)"));
}
