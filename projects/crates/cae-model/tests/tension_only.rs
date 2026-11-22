//! Tension-only truss members: compressed cables leave the active set, the
//! remaining structure is re-solved, and losing all stiffness is reported as a
//! mechanism.

use cae_model::{CaeError, ElementKind, Model, Units, solve_tension_only};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

/// Two collinear bars with a vertical roller at the loaded node.
///
/// `cable` (`e1`) is tension-only, `strut` (`e2`) is an ordinary bar. A positive
/// load pulls the node away from the cable, so the cable is in tension; a
/// negative load pushes the node towards it and the cable goes slack.
fn collinear_model(load: f64) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("a", 0.0, 0.0).expect("a");
    model.add_node("b", L, 0.0).expect("b");
    model.add_node("c", 2.0 * L, 0.0).expect("c");
    model.add_element("e1", "a", "b", None).expect("cable");
    model.add_element("e2", "b", "c", None).expect("strut");
    model.set_element_tension_only("e1", true).expect("tension-only");
    model.add_support("a", true, true).expect("fixed a");
    model.add_support("c", true, true).expect("fixed c");
    model.add_support("b", false, true).expect("vertical roller b");
    model.add_load("b", Some(load), None).expect("load");
    model
}

#[test]
fn tensioned_cable_stays_active() {
    let model = collinear_model(1000.0);
    let result = solve_tension_only(&model, 20, 1e-9).expect("solve");
    assert_eq!(result.iterations, 1);
    assert!(result.converged);
    assert!(result.elements[0].active);
    assert_close(result.solution.elements[0].axial_force, 500.0, 1e-12);
    assert_close(result.solution.elements[1].axial_force, -500.0, 1e-12);
    let node = result.solution.nodes.iter().find(|node| node.id == "b").expect("b");
    assert_close(node.ux, 1000.0 / (2.0 * E * A / L), 1e-12);
}

#[test]
fn compressed_cable_leaves_the_active_set() {
    let model = collinear_model(-1000.0);
    let result = solve_tension_only(&model, 20, 1e-9).expect("solve");
    assert_eq!(result.iterations, 2);
    assert!(result.converged);
    assert!(!result.elements[0].active);
    assert_close(result.elements[0].axial_force, 0.0, 1e-12);
    assert_close(result.solution.elements[0].axial_force, 0.0, 1e-12);
    assert_close(result.solution.elements[1].axial_force, 1000.0, 1e-12);
    let node = result.solution.nodes.iter().find(|node| node.id == "b").expect("b");
    assert_close(node.ux, -1000.0 / (E * A / L), 1e-12);
}

#[test]
fn reversed_load_reactivates_the_cable() {
    let mut model = collinear_model(-1000.0);
    let slack = solve_tension_only(&model, 20, 1e-9).expect("slack solve");
    assert!(!slack.elements[0].active);

    model.add_load("b", Some(1000.0), None).expect("reverse load");
    let restored = solve_tension_only(&model, 20, 1e-9).expect("restored solve");
    assert_eq!(restored.iterations, 1);
    assert!(restored.elements[0].active);
    assert_close(restored.solution.elements[0].axial_force, 500.0, 1e-12);
    assert_close(restored.solution.elements[1].axial_force, -500.0, 1e-12);
}

#[test]
fn slack_cable_that_removes_stability_reports_a_mechanism() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("a", 0.0, 0.0).expect("a");
    model.add_node("b", L, L).expect("b");
    model.add_node("c", 2.0 * L, 0.0).expect("c");
    model.add_element("e1", "a", "b", None).expect("strut");
    model.add_element("e2", "b", "c", None).expect("cable");
    model.set_element_tension_only("e2", true).expect("tension-only");
    model.add_support("a", true, true).expect("fixed a");
    model.add_support("c", true, true).expect("fixed c");
    model.add_load("b", None, Some(-1000.0)).expect("downward load");

    let error = solve_tension_only(&model, 20, 1e-9).expect_err("mechanism");
    assert_eq!(error.code, "model/mechanism");
    assert!(error.message.contains("singular") || error.message.contains("mechanism"));
}

#[test]
fn beams_cannot_be_tension_only() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("a", 0.0, 0.0).expect("a");
    model.add_node("b", L, 0.0).expect("b");
    model.add_element_kind("e1", "a", "b", None, ElementKind::Beam, Some(1.0e6), None).expect("beam");
    let error: CaeError = model.set_element_tension_only("e1", true).expect_err("beam rejection");
    assert_eq!(error.code, "element/tension-only-requires-truss");

    let mut document = model.clone();
    document.elements[0].tension_only = true;
    let issues = document.validate();
    assert!(issues.iter().any(|issue| issue.code == "element/tension-only-requires-truss"));
}
