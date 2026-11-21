//! Analytical tests for linear elastic spring supports.
//!
//! A spring is a flexible support: the matching degree of freedom stays free
//! and its stiffness is added to the diagonal of the global matrix. The support
//! reaction summary then reports the spring force `−k·u`.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Model, OpenCae, OpenCaeOptions, Severity, Units, solve};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-springs-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() / scale <= relative, "expected {expected}, got {actual} (relative tolerance {relative})");
}

fn axial_bar() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", L, 0.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");
    model.add_support("n1", true, true).expect("support n1");
    model.add_support("n2", false, true).expect("support n2");
    model
}

fn cantilever() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", L, 0.0).expect("node n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam b1");
    model.add_support_full("n1", true, true, false, true).expect("fixed support n1");
    model
}

#[test]
fn axial_bar_with_end_spring_matches_the_analytic_solution() {
    let mut model = axial_bar();
    model.set_spring("n2", Some(5000.0), None, None, None).expect("spring");
    model.add_load("n2", Some(10_000.0), None).expect("load");

    let solution = solve(&model).expect("solve");
    let bar_stiffness = E * A / L;
    let expected = 10_000.0 / (bar_stiffness + 5000.0);
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 displacement");
    assert_close(tip.ux, expected, 1e-12);

    let summary = &solution.reaction_summary;
    assert!(summary.balanced, "summary: {summary:?}");
    let fixed = summary.supports.iter().find(|support| support.node == "n1").expect("n1 reaction");
    let spring = summary.supports.iter().find(|support| support.node == "n2").expect("n2 reaction");
    assert_close(fixed.fx, -bar_stiffness * expected, 1e-12);
    assert_close(spring.fx, -5000.0 * expected, 1e-12);
    assert_close(summary.reaction.fx, -10_000.0, 1e-12);
    assert_close(summary.applied.fx, 10_000.0, 1e-12);
}

#[test]
fn cantilever_tip_spring_matches_the_analytic_solution() {
    let spring_stiffness = 5000.0;
    let mut model = cantilever();
    model.set_spring("n2", None, Some(spring_stiffness), None, None).expect("spring");
    model.add_load("n2", None, Some(-1000.0)).expect("load");

    let solution = solve(&model).expect("solve");
    let beam_stiffness = 3.0 * E * I / (L * L * L);
    let expected = -1000.0 / (beam_stiffness + spring_stiffness);
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 displacement");
    assert_close(tip.uy, expected, 1e-12);

    let summary = &solution.reaction_summary;
    assert!(summary.balanced, "summary: {summary:?}");
    let fixed = summary.supports.iter().find(|support| support.node == "n1").expect("n1 reaction");
    let spring = summary.supports.iter().find(|support| support.node == "n2").expect("n2 reaction");
    assert_close(fixed.fy, 1000.0 + spring_stiffness * expected, 1e-12);
    assert_close(spring.fy, -spring_stiffness * expected, 1e-12);
    assert_close(fixed.mz, fixed.fy * L, 1e-12);
    assert_close(summary.reaction.fy, 1000.0, 1e-12);
}

#[test]
fn cantilever_root_rotational_spring_matches_the_analytic_solution() {
    let rotational = 1.0e8;
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", L, 0.0).expect("node n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam b1");
    model.add_support_full("n1", true, true, false, false).expect("translations only");
    model.set_spring("n1", None, None, None, Some(rotational)).expect("rotational spring");
    model.add_load("n2", None, Some(-1000.0)).expect("load");

    let solution = solve(&model).expect("solve");
    let rotation = -1000.0 * L / rotational;
    let expected = -1000.0 * L * L * L / (3.0 * E * I) + rotation * L;
    let root = solution.nodes.iter().find(|node| node.id == "n1").expect("n1 displacement");
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 displacement");
    assert_close(root.rz, rotation, 1e-12);
    assert_close(tip.uy, expected, 1e-12);

    let support = solution.reaction_summary.supports.iter().find(|support| support.node == "n1").expect("n1 reaction");
    assert_close(support.mz, 1000.0 * L, 1e-12);
    assert!(solution.reaction_summary.balanced, "summary: {:?}", solution.reaction_summary);
}

#[test]
fn spring_validation_rejects_invalid_input() {
    let mut negative = axial_bar();
    assert_eq!(negative.set_spring("n2", Some(-1.0), None, None, None).expect_err("negative").code, "support/invalid-spring");

    let mut zero = axial_bar();
    assert_eq!(zero.set_spring("n2", Some(0.0), None, None, None).expect_err("zero").code, "support/invalid-spring");

    let mut unknown = axial_bar();
    assert_eq!(
        unknown.set_spring("missing", Some(1.0), None, None, None).expect_err("unknown node").code,
        "support/unknown-node"
    );

    let mut wrong_dimension = axial_bar();
    assert_eq!(
        wrong_dimension.set_spring("n2", None, None, Some(1.0), None).expect_err("kz in 2D").code,
        "document/dimension-mismatch"
    );

    let mut truss_rotation = axial_bar();
    truss_rotation.set_spring("n2", None, None, None, Some(1000.0)).expect("stored spring");
    assert!(
        truss_rotation.validate().iter().any(|issue| issue.code == "support/inactive-dof"),
        "a truss node has no rotational degree of freedom"
    );

    let mut conflict = axial_bar();
    conflict.set_spring("n2", None, Some(1000.0), None, None).expect("stored spring");
    assert!(
        conflict.validate().iter().any(|issue| issue.code == "support/spring-constrained"),
        "a spring and a rigid constraint must not share a degree of freedom"
    );
}

#[test]
fn spring_round_trips_through_a_document_and_the_session() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element("e1", "n1", "n2", None, None).expect("e1");
    session.add_support("n1", true, true, false).expect("support n1");
    session.add_support("n2", false, true, false).expect("support n2");
    session.set_spring("n2", Some(5000.0), None, None, None).expect("spring n2");
    session.add_load("n2", Some(10_000.0), None, None).expect("load");

    let text = fs::read_to_string(&path).expect("document");
    assert!(text.contains("\"spring\""), "document must store the spring: {text}");
    assert!(text.contains("\"kx\": 5000.0"), "document must store kx: {text}");

    let mut reopened = OpenCae::new(OpenCaeOptions::default());
    let summary = reopened.open(&path).expect("reopen");
    assert_eq!(summary.supports, 2);
    let solution = reopened.solve(None).expect("solve");
    assert_close(solution.max_displacement.ux, 10_000.0 / (E * A / L + 5000.0), 1e-12);
    assert!(solution.reaction_summary.balanced);

    let reopened_model = Model::load(&path).expect("load model");
    let issues = reopened_model.validate();
    assert!(!issues.iter().any(|issue| issue.severity == Severity::Error), "issues: {issues:?}");
}
