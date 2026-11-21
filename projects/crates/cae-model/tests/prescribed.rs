//! Analytical checks for prescribed (enforced) nodal displacements.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Model, PrescribedDisplacement, Units, solve};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-prescribed-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

/// Axial bar with a prescribed elongation at the far end.
fn prescribed_bar(delta: f64) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("e1");
    model.add_support("n1", true, true).expect("fixed end");
    model.add_support("n2", false, true).expect("lateral guide");
    model.prescribe_displacement("n2", Some(delta), None, None, None).expect("prescribed displacement");
    model
}

/// Propped cantilever whose roller support settles by `delta`.
fn propped_cantilever_settlement(delta: f64) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam element");
    model.add_support_full("n1", true, true, false, true).expect("fixed support");
    model.prescribe_displacement("n2", None, Some(-delta), None, None).expect("support settlement");
    model
}

#[test]
fn prescribed_bar_matches_analytic_axial_force_stress_and_reactions() {
    let delta = 0.5;
    let model = prescribed_bar(delta);
    assert!(model.validate().iter().all(|issue| issue.code != "support/prescribed-unconstrained"));

    let solution = solve(&model).expect("solve");
    let expected_force = E * A * delta / L;
    let expected_stress = E * delta / L;

    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 result");
    assert_close(tip.ux, delta, 1e-12);

    let element = &solution.elements[0];
    assert_close(element.axial_force, expected_force, 1e-12);
    assert_close(element.stress, expected_stress, 1e-12);

    let fixed = solution.reactions.iter().find(|reaction| reaction.node == "n1").expect("fixed reaction");
    let driven = solution.reactions.iter().find(|reaction| reaction.node == "n2").expect("driven reaction");
    assert_close(fixed.fx, -expected_force, 1e-12);
    assert_close(driven.fx, expected_force, 1e-12);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn propped_cantilever_support_settlement_matches_analytic_reactions_and_moment() {
    let delta = 2.0;
    let model = propped_cantilever_settlement(delta);
    let solution = solve(&model).expect("solve");

    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 result");
    assert_close(tip.uy, -delta, 1e-12);
    assert_close(tip.rz, -1.5 * delta / L, 1e-12);

    // Settling the roller requires a downward force of 3EIδ/L³. The fixed end
    // carries the opposite vertical force and the statically indeterminate
    // moment 3EIδ/L².
    let expected_shear = 3.0 * E * I * delta / L.powi(3);
    let expected_moment = 3.0 * E * I * delta / L.powi(2);

    let prop = solution.reactions.iter().find(|reaction| reaction.node == "n2").expect("prop reaction");
    let fixed = solution.reactions.iter().find(|reaction| reaction.node == "n1").expect("fixed reaction");
    assert_close(prop.fy, -expected_shear, 1e-12);
    assert_close(fixed.fy, expected_shear, 1e-12);
    assert_close(fixed.mz, expected_moment, 1e-12);
    assert_close(solution.elements[0].max_abs_moment, expected_moment, 1e-12);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn prescribed_displacements_reject_invalid_dimensions_and_unknown_nodes() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");

    assert_eq!(
        model.prescribe_displacement("n1", None, None, None, None).expect_err("no component").code,
        "support/no-freedom"
    );
    assert_eq!(
        model.prescribe_displacement("missing", Some(0.1), None, None, None).expect_err("unknown node").code,
        "support/unknown-node"
    );
    assert_eq!(
        model.prescribe_displacement("n1", None, None, Some(0.1), None).expect_err("uz in 2d").code,
        "document/dimension-mismatch"
    );

    let mut three_dimensional = Model::new_with_dimension(Units::Millimeter, 3);
    three_dimensional.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    assert_eq!(
        three_dimensional.prescribe_displacement("n1", None, None, None, Some(0.1)).expect_err("rz in 3d").code,
        "document/dimension-mismatch"
    );
}

#[test]
fn validation_reports_prescribed_values_without_constraints() {
    let mut model = prescribed_bar(0.5);
    let support = model.supports.iter_mut().find(|support| support.node == "n2").expect("n2 support");
    support.dx = false;
    support.prescribed = Some(PrescribedDisplacement { ux: 0.5, ..PrescribedDisplacement::default() });

    assert!(
        model.validate().iter().any(|issue| issue.code == "support/prescribed-unconstrained"),
        "issues: {:?}",
        model.validate()
    );
}

#[test]
fn prescribed_values_round_trip_through_the_document() {
    let directory = temporary_dir("roundtrip");
    let path = directory.join("model.ocae");
    let model = propped_cantilever_settlement(2.0);
    model.save_atomic(&path).expect("save");

    let text = fs::read_to_string(&path).expect("read");
    assert!(text.contains("\"prescribed\""), "document: {text}");

    let loaded = Model::load(&path).expect("load");
    assert_eq!(loaded, model);
    let prescribed = loaded.supports.iter().find_map(|support| support.prescribed.as_ref()).expect("prescribed values");
    assert_eq!(prescribed.uy, -2.0);
}
