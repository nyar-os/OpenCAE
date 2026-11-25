//! Analytical checks for `beam3d` end releases (spatial hinges).
//!
//! A released `beam3d` end condenses all three local rotations out of the
//! member, so it behaves as a spatial pin that still transfers translations.
//! The closed forms below are those of a propped cantilever and a simply
//! supported span with uniform loads in both bending planes; a consistent
//! Euler-Bernoulli element reproduces them exactly at the element ends.

use cae_model::{EndRelease, Model, Severity, Units, solve};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const I_WEAK: f64 = 5.0e5;
const J: f64 = 2.0e6;
const L: f64 = 1000.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn assert_near_zero(actual: f64, absolute: f64) {
    assert!(actual.abs() <= absolute, "expected ~0, got {actual}");
}

fn frame_model() -> Model {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.material.youngs_modulus = E;
    model.material.area = A;
    model
}

fn propped_cantilever(load_y: Option<f64>, load_z: Option<f64>) -> Model {
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    model
        .add_frame_element("b1", "n1", "n2", None, Some(I), Some(I_WEAK), Some(J), None, None, load_y, load_y, load_z, load_z)
        .expect("b1");
    model.set_element_release("b1", Some(EndRelease { start: false, end: true })).expect("release");
    model.add_support_frame("n1", true, true, true, true, true, true).expect("fixed n1");
    model.add_support_frame("n2", true, true, true, false, false, false).expect("pinned n2");
    model
}

#[test]
fn releasing_a_beam3d_end_turns_fixed_ends_into_a_propped_cantilever() {
    let w = -10.0;
    // Continuous fixed-fixed member: both end moments are w·L²/12.
    let mut fixed = frame_model();
    fixed.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    fixed.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    fixed
        .add_frame_element("b1", "n1", "n2", None, Some(I), Some(I), Some(J), None, None, Some(w), Some(w), None, None)
        .expect("b1");
    fixed.add_support_frame("n1", true, true, true, true, true, true).expect("fixed n1");
    fixed.add_support_frame("n2", true, true, true, true, true, true).expect("fixed n2");
    let continuous = solve(&fixed).expect("solve continuous");
    let continuous_moment = (w * L * L / 12.0).abs();
    let spatial = continuous.elements[0].spatial.as_ref().expect("spatial end forces");
    assert_close(spatial.mz1.abs(), continuous_moment, 1e-9);
    assert_close(spatial.mz2.abs(), continuous_moment, 1e-9);

    // Releasing the end rotation removes the end moment and raises the fixed
    // moment to the propped-cantilever value w·L²/8.
    let released = propped_cantilever(Some(w), None);
    let solution = solve(&released).expect("solve released");
    let spatial = solution.elements[0].spatial.as_ref().expect("spatial end forces");
    assert_close(spatial.mz1.abs(), (w * L * L / 8.0).abs(), 1e-9);
    assert_near_zero(spatial.mz2, 1e-9 * (w * L * L / 8.0).abs());
    assert_near_zero(spatial.t2, 1e-9 * (w * L * L / 8.0).abs());
    assert_close(spatial.vy1.abs(), (5.0 * w * L / 8.0).abs(), 1e-9);
    let reaction = solution.reactions.iter().find(|reaction| reaction.node == "n2").expect("n2 reaction");
    assert_close(reaction.fy.abs(), (3.0 * w * L / 8.0).abs(), 1e-9);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn a_beam3d_hinge_carries_biaxial_bending_independently() {
    let wy = -10.0;
    let wz = -6.0;
    let model = propped_cantilever(Some(wy), Some(wz));
    let solution = solve(&model).expect("solve");
    let spatial = solution.elements[0].spatial.as_ref().expect("spatial end forces");
    // Local +y loading bends about z; local +z loading bends about y.
    assert_close(spatial.mz1.abs(), (wy * L * L / 8.0).abs(), 1e-9);
    assert_close(spatial.my1.abs(), (wz * L * L / 8.0).abs(), 1e-9);
    assert_close(spatial.vy1.abs(), (5.0 * wy * L / 8.0).abs(), 1e-9);
    assert_close(spatial.vz1.abs(), (5.0 * wz * L / 8.0).abs(), 1e-9);
    let moment_scale = (wy * L * L / 8.0).abs().max((wz * L * L / 8.0).abs());
    assert_near_zero(spatial.my2, 1e-9 * moment_scale);
    assert_near_zero(spatial.mz2, 1e-9 * moment_scale);
    assert_near_zero(spatial.t2, 1e-9 * moment_scale);
    let reaction = solution.reactions.iter().find(|reaction| reaction.node == "n2").expect("n2 reaction");
    assert_close(reaction.fy.abs(), (3.0 * wy * L / 8.0).abs(), 1e-9);
    assert_close(reaction.fz.abs(), (3.0 * wz * L / 8.0).abs(), 1e-9);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn releasing_the_outer_beam3d_ends_forms_a_simply_supported_span() {
    let wy = -10.0;
    let wz = -6.0;
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L / 2.0, 0.0, 0.0).expect("n2");
    model.add_node_3d("n3", L, 0.0, 0.0).expect("n3");
    for (id, from, to) in [("e1", "n1", "n2"), ("e2", "n2", "n3")] {
        model
            .add_frame_element(
                id,
                from,
                to,
                None,
                Some(I),
                Some(I_WEAK),
                Some(J),
                None,
                None,
                Some(wy),
                Some(wy),
                Some(wz),
                Some(wz),
            )
            .expect(id);
    }
    model.set_element_release("e1", Some(EndRelease { start: true, end: false })).expect("release e1");
    model.set_element_release("e2", Some(EndRelease { start: false, end: true })).expect("release e2");
    model.add_support_frame("n1", true, true, true, false, false, false).expect("pin n1");
    model.add_support_frame("n2", false, false, false, true, false, false).expect("torsion n2");
    model.add_support_frame("n3", false, true, true, false, false, false).expect("roller n3");

    let solution = solve(&model).expect("solve");
    let mid = solution.nodes.iter().find(|node| node.id == "n2").expect("n2");
    assert_close(mid.uy, 5.0 * wy * L.powi(4) / (384.0 * E * I), 1e-9);
    assert_close(mid.uz, 5.0 * wz * L.powi(4) / (384.0 * E * I_WEAK), 1e-9);

    let first = solution.elements.iter().find(|element| element.id == "e1").expect("e1");
    let last = solution.elements.iter().find(|element| element.id == "e2").expect("e2");
    let first = first.spatial.as_ref().expect("e1 spatial");
    let last = last.spatial.as_ref().expect("e2 spatial");
    assert_near_zero(first.mz1, 1e-9 * (wy * L * L / 8.0).abs());
    assert_near_zero(last.mz2, 1e-9 * (wy * L * L / 8.0).abs());
    assert_near_zero(first.my1, 1e-9 * (wz * L * L / 8.0).abs());
    assert_near_zero(last.my2, 1e-9 * (wz * L * L / 8.0).abs());
    assert_close(first.mz2.abs(), (wy * L * L / 8.0).abs(), 1e-9);
    assert_close(first.my2.abs(), (wz * L * L / 8.0).abs(), 1e-9);

    for reaction in &solution.reactions {
        if reaction.node == "n1" || reaction.node == "n3" {
            assert_close(reaction.fy.abs(), (wy * L / 2.0).abs(), 1e-9);
            assert_close(reaction.fz.abs(), (wz * L / 2.0).abs(), 1e-9);
        }
    }
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn beam3d_release_validation_rejects_inactive_rotations() {
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    model.add_frame_element("b1", "n1", "n2", None, Some(I), None, Some(J), None, None, None, None, None, None).expect("b1");
    model.set_element_release("b1", Some(EndRelease { start: false, end: true })).expect("release");
    assert!(!model.validate().iter().any(|issue| issue.code == "element/release-requires-beam"));

    assert_eq!(
        model.set_element_release("b1", Some(EndRelease::default())).expect_err("empty release").code,
        "element/invalid-release"
    );

    model.add_support_frame("n1", true, true, true, true, true, true).expect("fixed n1");
    model.add_support_frame("n2", true, true, true, false, false, false).expect("pinned n2");
    model.add_support_frame("n2", false, false, false, true, false, false).expect("rx");
    assert!(model.validate().iter().any(|issue| issue.code == "support/inactive-dof" && issue.severity == Severity::Error));

    model.add_load_frame("n2", None, None, None, Some(1.0), None, None).expect("mx");
    assert!(model.validate().iter().any(|issue| issue.code == "load/inactive-dof" && issue.severity == Severity::Error));

    let mut truss = frame_model();
    truss.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    truss.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    truss.add_element("t1", "n1", "n2", None).expect("truss");
    assert_eq!(
        truss.set_element_release("t1", Some(EndRelease { start: true, end: false })).expect_err("truss release").code,
        "element/release-requires-beam"
    );
}

#[test]
fn beam3d_release_round_trips_through_json() {
    let model = propped_cantilever(Some(-10.0), None);
    let text = model.to_json_pretty().expect("serialize");
    assert!(text.contains("\"release\""), "document: {text}");
    let parsed = Model::parse_json(&text).expect("parse");
    assert_eq!(parsed, model);
}
