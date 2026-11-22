use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{Model, OpenCae, OpenCaeOptions, Units};

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-removal-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn bar_model() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1000.0, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("e1");
    model.add_support("n1", true, true).expect("support n1");
    model.add_load("n2", Some(1000.0), None).expect("load n2");
    model
}

#[test]
fn remove_node_rejects_an_element_reference() {
    let mut model = bar_model();
    let error = model.remove_node("n1").expect_err("node is used by an element");
    assert_eq!(error.code, "node/referenced-by-element");
    assert!(error.message.contains("e1"), "message: {}", error.message);
    assert!(model.node("n1").is_some());

    model.remove_element("e1").expect("remove element");
    model.remove_support("n1");
    model.remove_load("n1");
    model.remove_node("n1").expect("node is now unreferenced");
    assert!(model.node("n1").is_none());
}

#[test]
fn remove_node_reports_support_and_load_references() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_support("n1", true, true).expect("support");
    model.add_load("n1", Some(1.0), None).expect("load");

    let error = model.remove_node("n1").expect_err("support still references the node");
    assert_eq!(error.code, "node/referenced-by-support");

    assert!(model.remove_support("n1"));
    let error = model.remove_node("n1").expect_err("load still references the node");
    assert_eq!(error.code, "node/referenced-by-load");

    assert!(model.remove_load("n1"));
    model.remove_node("n1").expect("node is unreferenced");
    assert!(model.nodes.is_empty());
}

#[test]
fn unknown_identifiers_are_rejected() {
    let mut model = bar_model();
    assert_eq!(model.remove_node("missing").expect_err("unknown node").code, "node/unknown-id");
    assert_eq!(model.remove_element("missing").expect_err("unknown element").code, "element/unknown-id");
    assert!(!model.remove_support("missing"));
    assert!(!model.remove_load("missing"));
}

#[test]
fn removal_bumps_the_revision_and_keeps_other_entities() {
    let mut model = bar_model();
    let revision = model.revision;
    assert!(model.remove_load("n2"));
    assert_eq!(model.revision, revision + 1);
    assert!(model.loads.is_empty());
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.elements.len(), 1);
    assert_eq!(model.supports.len(), 1);

    model.remove_element("e1").expect("remove element");
    assert!(model.elements.is_empty());
    assert_eq!(model.supports.len(), 1);
}

#[test]
fn session_persists_ordered_removal_to_the_document() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", 1000.0, 0.0, None).expect("n2");
    session.add_element("e1", "n1", "n2", None, None).expect("e1");
    session.add_support("n1", true, true, false).expect("support");
    session.add_load("n2", Some(1000.0), None, None).expect("load");

    let summary = session.remove_load("n2").expect("remove load");
    assert_eq!(summary.loads, 0);
    assert_eq!(summary.nodes, 2);
    let summary = session.remove_support("n1").expect("remove support");
    assert_eq!(summary.supports, 0);
    let summary = session.remove_element("e1").expect("remove element");
    assert_eq!(summary.elements, 0);
    let summary = session.remove_node("n1").expect("remove n1");
    assert_eq!(summary.nodes, 1);
    session.remove_node("n2").expect("remove n2");

    let written = fs::read_to_string(&path).expect("read document");
    assert!(written.contains("\"nodes\": []"), "document: {written}");
    assert!(written.contains("\"elements\": []"), "document: {written}");
    assert!(written.contains("\"supports\": []"), "document: {written}");
    assert!(written.contains("\"loads\": []"), "document: {written}");
}

#[test]
fn session_reports_missing_entities() {
    let directory = temporary_dir("missing");
    let path = directory.join("model.ocae");
    let mut session = build_session(&path);
    assert_eq!(session.remove_node("n2").expect_err("referenced node").code, "node/referenced-by-element");
    assert_eq!(session.remove_element("missing").expect_err("unknown element").code, "element/unknown-id");
    assert_eq!(session.remove_support("n2").expect_err("no support").code, "support/unknown-support");
    assert_eq!(session.remove_load("n1").expect_err("no load").code, "load/unknown-load");
}

fn build_session(path: &Path) -> OpenCae {
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", 1000.0, 0.0, None).expect("n2");
    session.add_element("e1", "n1", "n2", None, None).expect("e1");
    session.add_support("n1", true, true, false).expect("support");
    session.add_load("n2", Some(1000.0), None, None).expect("load");
    session
}
