use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{OpenCae, OpenCaeOptions};

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-query-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn session_with_bar(path: &std::path::Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", 1000.0, 0.0, None).expect("n2");
    session.add_element("e1", "n1", "n2", None, None).expect("e1");
    session.add_support("n1", true, true, false).expect("support");
    session.add_support("n2", false, true, false).expect("support");
    session.add_load("n2", Some(1000.0), None, None).expect("load");
    session
}

#[test]
fn node_displacement_query_matches_the_analytic_solution() {
    let directory = temporary_dir("node");
    let path = directory.join("model.ocae");
    let session = session_with_bar(&path);
    let displacement = session.node_displacement("n2").expect("n2 displacement");
    let expected = 1000.0 * 1000.0 / (210_000.0 * 100.0);

    assert_eq!(displacement.id, "n2");
    assert_close(displacement.ux, expected, 1e-9);
    assert!(displacement.uy.abs() < 1e-12);
    assert_close(displacement.magnitude, expected, 1e-9);
}

#[test]
fn element_result_query_reports_axial_force_and_end_forces() {
    let directory = temporary_dir("element");
    let path = directory.join("model.ocae");
    let session = session_with_bar(&path);
    let element = session.element_result("e1").expect("e1 result");

    assert_eq!(element.id, "e1");
    assert_eq!(element.from, "n1");
    assert_eq!(element.to, "n2");
    assert_close(element.axial_force, 1000.0, 1e-9);
    assert_close(element.end_forces.n1, -1000.0, 1e-9);
    assert_close(element.end_forces.n2, 1000.0, 1e-9);
    assert_close(element.max_abs_moment, 0.0, 1e-12);

    let json = serde_json::to_value(&element).expect("serialize element result");
    assert_eq!(json["endForces"]["n1"], -1000.0);
    assert_eq!(json["axialForce"], 1000.0);
}

#[test]
fn node_reaction_query_returns_the_support_reaction() {
    let directory = temporary_dir("reaction");
    let path = directory.join("model.ocae");
    let session = session_with_bar(&path);
    let reaction = session.node_reaction("n1").expect("n1 reaction");

    assert_eq!(reaction.node, "n1");
    assert_close(reaction.fx, -1000.0, 1e-9);
    assert!(reaction.fz.abs() < 1e-12);

    let second = session.node_reaction("n2").expect("n2 reaction");
    assert_eq!(second.node, "n2");
    assert_close(second.fx, 0.0, 1e-12);

    let error = session.node_reaction("missing").expect_err("no reaction");
    assert_eq!(error.code, "result/no-reaction");
}

#[test]
fn unknown_identifiers_report_stable_codes() {
    let directory = temporary_dir("unknown");
    let path = directory.join("model.ocae");
    let session = session_with_bar(&path);

    assert_eq!(session.node_displacement("missing").expect_err("unknown node").code, "result/unknown-node");
    assert_eq!(session.element_result("missing").expect_err("unknown element").code, "result/unknown-element");
    assert_eq!(session.node_reaction("missing").expect_err("unknown reaction").code, "result/no-reaction");
}
