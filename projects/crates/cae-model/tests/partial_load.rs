//! Partial-span distributed load tests.
//!
//! A uniform load of intensity `q` acting between `L/4` and `3L/4` of a simply
//! supported beam is statically determinate, so the reactions, shear diagram,
//! bending moments and midspan deflection have closed forms.

use cae_model::{ElementKind, Model, Units, solve, solve_deflection, solve_internal_forces};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;
const I: f64 = 1.0e6;
const Q: f64 = -10.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn simply_supported_partial() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(Q), Some(Q)).expect("beam");
    model.set_element_load_span("b1", Some((0.25, 0.75))).expect("span");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

#[test]
fn partial_uniform_load_reactions_and_moments_match_the_analytic_solution() {
    let model = simply_supported_partial();
    let solution = solve(&model).expect("solve");

    let reaction = Q * L / 4.0;
    for node in ["n1", "n2"] {
        let entry = solution.reactions.iter().find(|entry| entry.node == node).expect("reaction");
        assert_close(entry.fy, -reaction, 1e-12);
        assert_close(entry.fx, 0.0, 1e-12);
    }
    assert!(solution.reaction_summary.balanced);

    let diagram = solve_internal_forces(&model, 5).expect("diagram");
    let element = &diagram.elements[0];
    let station = |index: usize| &element.stations[index];
    assert_close(station(0).shear, -reaction, 1e-12);
    assert_close(station(1).shear, -reaction, 1e-12);
    assert_close(station(2).shear, 0.0, 1e-12);
    assert_close(station(3).shear, reaction, 1e-12);
    assert_close(station(4).shear, reaction, 1e-12);

    assert_close(station(0).moment, 0.0, 1e-12);
    assert_close(station(1).moment, -Q * L * L / 16.0, 1e-12);
    assert_close(station(2).moment, -3.0 * Q * L * L / 32.0, 1e-12);
    assert_close(station(3).moment, -Q * L * L / 16.0, 1e-12);
    assert_close(station(4).moment, 0.0, 1e-12);
    assert_close(element.max_abs_moment, 3.0 * Q.abs() * L * L / 32.0, 1e-12);

    // The sampled ends reproduce the element end forces.
    assert_close(station(0).shear, solution.elements[0].end_forces.v1, 1e-12);
    assert_close(station(4).shear, -solution.elements[0].end_forces.v2, 1e-12);
}

#[test]
fn partial_uniform_load_deflection_matches_the_analytic_midspan_value() {
    let model = simply_supported_partial();
    let deflection = solve_deflection(&model, 5).expect("deflection");
    let element = &deflection.elements[0];

    let expected = 19.0 * Q * L.powi(4) / (2048.0 * E * I);
    assert_close(element.stations[2].transverse, expected, 1e-9);
    assert!(element.stations[2].rotation.abs() < 1e-9);
    assert!(element.stations[0].transverse.abs() < 1e-9);
    assert!(element.stations[4].transverse.abs() < 1e-9);
    assert_close(deflection.governing.transverse, expected, 1e-9);
    assert_close(deflection.governing.position, L / 2.0, 1e-9);
}

#[test]
fn a_full_span_load_span_is_equivalent_to_the_default() {
    let mut model = simply_supported_partial();
    model.set_element_load_span("b1", Some((0.0, 1.0))).expect("full span");
    let solution = solve(&model).expect("solve");
    let reaction = Q * L / 2.0;
    for node in solution.reactions.iter().filter(|entry| entry.fy != 0.0) {
        assert_close(node.fy, -reaction, 1e-9);
    }
}

#[test]
fn load_span_edits_reject_invalid_input() {
    let mut model = simply_supported_partial();
    assert_eq!(
        model.set_element_load_span("b1", Some((0.75, 0.25))).expect_err("reversed span").code,
        "element/invalid-load-span"
    );
    assert_eq!(model.set_element_load_span("b1", Some((0.0, 1.5))).expect_err("outside").code, "element/invalid-load-span");
    assert_eq!(model.set_element_load_span("missing", Some((0.25, 0.75))).expect_err("unknown").code, "element/unknown-id");
    model.set_element_load_span("b1", None).expect("clear");

    let mut truss = Model::new(Units::Millimeter);
    truss.add_node("n1", 0.0, 0.0).expect("n1");
    truss.add_node("n2", L, 0.0).expect("n2");
    truss.add_element("e1", "n1", "n2", None).expect("bar");
    assert_eq!(
        truss.set_element_load_span("e1", Some((0.25, 0.75))).expect_err("truss").code,
        "element/distributed-load-requires-beam"
    );

    let mut undeclared = Model::new(Units::Millimeter);
    undeclared.add_node("n1", 0.0, 0.0).expect("n1");
    undeclared.add_node("n2", L, 0.0).expect("n2");
    undeclared.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    assert_eq!(
        undeclared.set_element_load_span("b1", Some((0.25, 0.75))).expect_err("no load").code,
        "element/missing-distributed-load"
    );
}
