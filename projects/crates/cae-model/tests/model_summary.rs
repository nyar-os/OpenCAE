//! Model-quantity summary tests.
//!
//! Lengths, volumes and masses are statically determinate for a straight bar,
//! and the degree-of-freedom counts follow the solver layout exactly.

use cae_model::{ElementKind, EndRelease, Model, Units, summarize_model};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const DENSITY: f64 = 7.85e-9;
const L: f64 = 1000.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn bar_model() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("e1");
    model.set_material_full(Some("steel".to_string()), E, A, DENSITY);
    model.set_node_mass("n1", 0.001).expect("mass n1");
    model.set_node_mass("n2", 0.002).expect("mass n2");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

#[test]
fn truss_summary_matches_hand_calculated_length_volume_mass_and_dofs() {
    let summary = summarize_model(&bar_model()).expect("summary");

    assert_eq!(summary.format, "open-cae-model-summary");
    assert_eq!(summary.units, "mm");
    assert_eq!(summary.dimension, 2);
    assert_eq!((summary.nodes, summary.elements), (2, 1));
    assert_eq!((summary.truss_elements, summary.beam_elements), (1, 0));
    assert_eq!(summary.supports, 2);
    assert_eq!(summary.patterns, 0);
    assert_eq!(summary.combinations, 0);

    assert_close(summary.total_length, L, 1e-15);
    assert_close(summary.truss_length, L, 1e-15);
    assert_close(summary.beam_length, 0.0, 1e-15);
    assert_close(summary.volume, A * L, 1e-15);
    assert_close(summary.member_mass, DENSITY * A * L, 1e-15);
    assert_close(summary.nodal_mass, 0.003, 1e-15);
    assert_close(summary.total_mass, DENSITY * A * L + 0.003, 1e-15);

    let centroid = summary.center_of_mass.expect("mass centroid");
    let expected_x = (DENSITY * A * L * L / 2.0 + 0.002 * L) / (DENSITY * A * L + 0.003);
    assert_close(centroid[0], expected_x, 1e-12);
    assert_close(centroid[1], 0.0, 1e-15);
    assert_close(centroid[2], 0.0, 1e-15);

    // 2D truss: two translations per node; the pin fixes two and the roller one.
    assert_eq!(summary.translational_dofs, 4);
    assert_eq!(summary.rotational_dofs, 0);
    assert_eq!(summary.total_dofs, 4);
    assert_eq!(summary.constrained_dofs, 3);
    assert_eq!(summary.free_dofs, 1);
}

#[test]
fn beam_rotations_and_end_releases_change_the_degree_of_freedom_count() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(1.0e6), None).expect("beam");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");

    let full = summarize_model(&model).expect("summary");
    assert_eq!(full.translational_dofs, 4);
    assert_eq!(full.rotational_dofs, 2);
    assert_eq!(full.total_dofs, 6);
    assert_eq!(full.constrained_dofs, 3);
    assert_eq!(full.free_dofs, 3);

    // A released start end removes the rotation degree of freedom at n1.
    model.set_element_release("b1", Some(EndRelease { start: true, end: false })).expect("release start");
    let start_released = summarize_model(&model).expect("summary");
    assert_eq!(start_released.rotational_dofs, 1);
    assert_eq!(start_released.total_dofs, 5);
    assert_eq!(start_released.free_dofs, 2);

    // Releasing both ends leaves a simply supported member with translations only.
    model.set_element_release("b1", Some(EndRelease { start: true, end: true })).expect("release both");
    let both_released = summarize_model(&model).expect("summary");
    assert_eq!(both_released.rotational_dofs, 0);
    assert_eq!(both_released.total_dofs, 4);
    assert_eq!(both_released.free_dofs, 1);
}

#[test]
fn three_dimensional_truss_has_three_translations_per_node() {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("e1");
    model.add_support_3d("n1", true, true, true).expect("fixed");
    model.add_support_3d("n2", false, true, true).expect("guided");

    let summary = summarize_model(&model).expect("summary");
    assert_eq!(summary.dimension, 3);
    assert_eq!(summary.translational_dofs, 6);
    assert_eq!(summary.rotational_dofs, 0);
    assert_eq!(summary.total_dofs, 6);
    assert_eq!(summary.constrained_dofs, 5);
    assert_eq!(summary.free_dofs, 1);
    assert!(summary.center_of_mass.is_none(), "a zero density and no nodal mass has no centroid");
}

#[test]
fn summary_reports_stable_errors_instead_of_guessing() {
    let mut model = bar_model();
    model.material.density = f64::NAN;
    assert_eq!(summarize_model(&model).expect_err("NaN density").code, "material/invalid-density");

    let mut model = bar_model();
    model.material.density = -1.0;
    assert_eq!(summarize_model(&model).expect_err("negative density").code, "material/invalid-density");

    let mut model = bar_model();
    model.nodes[1].mass = -0.5;
    assert_eq!(summarize_model(&model).expect_err("negative mass").code, "node/invalid-mass");

    let mut model = bar_model();
    model.nodes[0].x = f64::INFINITY;
    assert_eq!(summarize_model(&model).expect_err("non-finite coordinate").code, "node/invalid-coordinate");

    let mut model = bar_model();
    model.material.area = 0.0;
    assert_eq!(summarize_model(&model).expect_err("zero area").code, "material/invalid");

    let mut model = bar_model();
    model.elements[0].from = "missing".to_string();
    assert_eq!(summarize_model(&model).expect_err("unknown node").code, "element/unknown-node");
}
