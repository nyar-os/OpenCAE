//! Bending-moment release (hinge) tests for two-dimensional beams.
//!
//! A released end carries no moment, which turns a fixed-ended beam into a
//! propped cantilever and, with both outer ends released, into a simply
//! supported beam. The analytical results below are independent of the mesh
//! because consistent beam loads give exact nodal displacements.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, EndRelease, Model, OpenCae, OpenCaeOptions, Severity, Units};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;
const W: f64 = -10.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-release-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn beam(nodes: &[(&str, f64)]) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material(Some("steel".to_string()), E, A);
    for (id, x) in nodes {
        model.add_node(*id, *x, 0.0).expect("node");
    }
    model
}

#[test]
fn releasing_the_end_removes_the_end_moment_of_a_fixed_beam() {
    // Continuous fixed-fixed beam: both end moments are w·L²/12.
    let mut continuous = beam(&[("n1", 0.0), ("n2", L)]);
    continuous.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(W)).expect("beam");
    continuous.add_support_full("n1", true, true, false, true).expect("fixed n1");
    continuous.add_support_full("n2", true, true, false, true).expect("fixed n2");
    let reference = cae_model::solve(&continuous).expect("solve continuous");
    let fixed_moment = (W * L * L / 12.0).abs();
    assert_close(reference.elements[0].end_forces.m1.abs(), fixed_moment, 1e-9);
    assert_close(reference.elements[0].end_forces.m2.abs(), fixed_moment, 1e-9);

    // Release the end rotation at n2: the beam becomes a propped cantilever,
    // so its end moment vanishes and the fixed moment grows to w·L²/8.
    let mut released = beam(&[("n1", 0.0), ("n2", L)]);
    released.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(W)).expect("beam");
    released.set_element_release("b1", Some(EndRelease { start: false, end: true })).expect("release");
    released.add_support_full("n1", true, true, false, true).expect("fixed n1");
    released.add_support("n2", false, true).expect("roller n2");

    let solution = cae_model::solve(&released).expect("solve released");
    let forces = &solution.elements[0].end_forces;
    assert!(forces.m2.abs() < 1e-6, "released end moment should vanish, got {}", forces.m2);
    assert_close(forces.m1.abs(), (W * L * L / 8.0).abs(), 1e-9);

    let reaction = solution.reactions.iter().find(|reaction| reaction.node == "n2").expect("n2 reaction");
    assert_close(reaction.fy.abs(), (3.0 * W * L / 8.0).abs(), 1e-9);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn both_outer_releases_form_a_simply_supported_beam() {
    let mut model = beam(&[("n1", 0.0), ("n2", L / 2.0), ("n3", L)]);
    model.add_element_kind("e1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(W)).expect("first beam");
    model.add_element_kind("e2", "n2", "n3", None, ElementKind::Beam, Some(I), Some(W)).expect("second beam");
    model.set_element_release("e1", Some(EndRelease { start: true, end: false })).expect("release start");
    model.set_element_release("e2", Some(EndRelease { start: false, end: true })).expect("release end");
    model.add_support("n1", true, true).expect("pin n1");
    model.add_support("n3", false, true).expect("pin n3");

    let solution = cae_model::solve(&model).expect("solve");
    let midspan = solution.nodes.iter().find(|node| node.id == "n2").expect("n2");
    assert_close(midspan.uy, 5.0 * W * L.powi(4) / (384.0 * E * I), 1e-9);

    for result in &solution.elements {
        let released_moment =
            if result.end_forces.m1.abs() < result.end_forces.m2.abs() { result.end_forces.m1 } else { result.end_forces.m2 };
        assert!(released_moment.abs() < 1e-6, "released end moment should vanish, got {released_moment}");
    }

    let forces = cae_model::solve_internal_forces(&model, 11).expect("forces");
    let max_moment = forces
        .elements
        .iter()
        .flat_map(|element| element.stations.iter())
        .fold(0.0_f64, |largest, station| largest.max(station.moment.abs()));
    assert_close(max_moment, (W * L * L / 8.0).abs(), 1e-9);
}

#[test]
fn release_validation_rejects_invalid_input() {
    let mut truss = beam(&[("n1", 0.0), ("n2", L)]);
    truss.add_element_kind("t1", "n1", "n2", None, ElementKind::Truss, None, None).expect("truss");
    truss.add_support("n1", true, true).expect("pin");
    truss.add_support("n2", false, true).expect("guide");
    let error = truss.set_element_release("t1", Some(EndRelease { start: true, end: false })).expect_err("truss release");
    assert_eq!(error.code, "element/release-requires-beam");
    truss.elements[0].release = Some(EndRelease { start: true, end: false });
    assert!(
        truss.validate().iter().any(|issue| issue.code == "element/release-requires-beam" && issue.severity == Severity::Error)
    );

    let mut model = beam(&[("n1", 0.0), ("n2", L)]);
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    assert_eq!(
        model.set_element_release("b1", Some(EndRelease::default())).expect_err("empty release").code,
        "element/invalid-release"
    );
    assert_eq!(
        model.set_element_release("missing", Some(EndRelease { start: true, end: false })).expect_err("unknown").code,
        "element/unknown-id"
    );

    // Constraining rz at a node whose only beam end is released is a conflict.
    model.add_support_full("n1", true, true, false, true).expect("fixed n1");
    model.add_support("n2", false, true).expect("roller n2");
    model.set_element_release("b1", Some(EndRelease { start: true, end: false })).expect("release");
    assert!(model.validate().iter().any(|issue| issue.code == "support/inactive-dof"));
}

#[test]
fn release_round_trips_through_a_document() {
    let directory = temporary_dir("roundtrip");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.set_material(E, A, Some("steel".to_string())).expect("material");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, Some(I), None).expect("beam");
    session.set_element_release("b1", Some(EndRelease { start: false, end: true })).expect("release");
    session.add_support("n1", true, true, false).expect("pin n1");
    session.add_support("n2", false, true, false).expect("roller n2");

    let text = fs::read_to_string(&path).expect("document");
    assert!(text.contains("\"release\": {"), "document: {text}");
    assert!(text.contains("\"end\": true"), "document: {text}");

    let summary = session.summary().expect("summary");
    assert!(summary.revision > 0);
}
