//! Integration tests for in-place node movement and element endpoint
//! reconnection.
//!
//! Both operations keep identifiers and references so a solved model can be
//! edited without rebuilding its topology.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, ElementMaterial, EndRelease, Model, OpenCae, OpenCaeOptions, Units};

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-update-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn bar_model() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", 1000.0, 0.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");
    model
}

#[test]
fn node_move_changes_only_the_supplied_coordinates_and_bumps_the_revision() {
    let mut model = bar_model();
    let revision = model.revision;

    model.update_node("n2", None, Some(250.0), None).expect("move n2");

    let node = model.node("n2").expect("node n2");
    assert_eq!((node.x, node.y, node.z), (1000.0, 250.0, 0.0));
    assert_eq!(model.revision, revision + 1);
}

#[test]
fn node_move_supports_three_dimensional_partial_updates() {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("node n1");
    model.add_node_3d("n2", 0.0, 0.0, 1000.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");

    model.update_node("n2", Some(25.0), Some(-15.0), Some(750.0)).expect("move n2");

    let node = model.node("n2").expect("node n2");
    assert_eq!((node.x, node.y, node.z), (25.0, -15.0, 750.0));
}

#[test]
fn node_move_rejects_unknown_ids_empty_requests_and_two_dimensional_z() {
    let mut model = bar_model();
    let revision = model.revision;

    assert_eq!(model.update_node("missing", Some(1.0), None, None).expect_err("unknown id").code, "node/unknown-id");
    assert_eq!(model.update_node("n2", None, None, None).expect_err("empty request").code, "node/invalid-coordinate");
    assert_eq!(model.update_node("n2", None, None, Some(1.0)).expect_err("z in 2d").code, "document/dimension-mismatch");

    assert_eq!(model.revision, revision);
    let node = model.node("n2").expect("node n2");
    assert_eq!((node.x, node.y, node.z), (1000.0, 0.0, 0.0));
}

#[test]
fn node_move_that_would_collapse_an_element_is_rejected_without_mutating() {
    let mut model = bar_model();
    let revision = model.revision;

    assert_eq!(model.update_node("n2", Some(0.0), Some(0.0), None).expect_err("zero length").code, "element/zero-length");

    assert_eq!(model.revision, revision);
    let node = model.node("n2").expect("node n2");
    assert_eq!((node.x, node.y, node.z), (1000.0, 0.0, 0.0));
}

#[test]
fn element_reconnect_preserves_material_loads_temperature_and_release() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", 1000.0, 0.0).expect("node n2");
    model.add_node("n3", 1000.0, 500.0).expect("node n3");
    let material = ElementMaterial { youngs_modulus: Some(100_000.0), area: Some(50.0) };
    model
        .add_element_with_loads(
            "b1",
            "n1",
            "n2",
            Some(material.clone()),
            ElementKind::Beam,
            Some(1.0e6),
            Some(-2.0),
            Some(-4.0),
        )
        .expect("beam b1");
    model.set_element_temperature("b1", Some(30.0)).expect("temperature");
    model.set_element_release("b1", Some(EndRelease { start: true, end: false })).expect("release");
    let revision = model.revision;

    model.update_element_endpoints("b1", None, Some("n3")).expect("reconnect b1");

    let element = model.elements.iter().find(|element| element.id == "b1").expect("element b1");
    assert_eq!((element.from.as_str(), element.to.as_str()), ("n1", "n3"));
    assert_eq!(element.material.as_ref(), Some(&material));
    assert_eq!(element.kind, ElementKind::Beam);
    assert_eq!(element.inertia, Some(1.0e6));
    assert_eq!(element.distributed_load, Some(-2.0));
    assert_eq!(element.distributed_load_end, Some(-4.0));
    assert_eq!(element.temperature, Some(30.0));
    assert_eq!(element.release, Some(EndRelease { start: true, end: false }));
    assert_eq!(model.revision, revision + 1);
}

#[test]
fn element_reconnect_rejects_invalid_endpoints() {
    let mut model = bar_model();
    model.add_node("n3", 0.0, 0.0).expect("node n3");
    let revision = model.revision;

    assert_eq!(model.update_element_endpoints("missing", None, Some("n2")).expect_err("unknown id").code, "element/unknown-id");
    assert_eq!(
        model.update_element_endpoints("e1", None, Some("missing")).expect_err("unknown node").code,
        "element/unknown-node"
    );
    assert_eq!(model.update_element_endpoints("e1", None, None).expect_err("empty request").code, "element/degenerate");
    assert_eq!(
        model.update_element_endpoints("e1", Some("n2"), Some("n2")).expect_err("same endpoint").code,
        "element/degenerate"
    );
    assert_eq!(
        model.update_element_endpoints("e1", None, Some("n3")).expect_err("coincident nodes").code,
        "element/zero-length"
    );

    assert_eq!(model.revision, revision);
    let element = model.elements.iter().find(|element| element.id == "e1").expect("element e1");
    assert_eq!((element.from.as_str(), element.to.as_str()), ("n1", "n2"));
}

#[test]
fn session_updates_persist_and_increment_the_revision() {
    let directory = temporary_dir("session");
    let path = directory.join("demo.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("node n1");
    session.add_node("n2", 1000.0, 0.0, None).expect("node n2");
    session.add_node("n3", 1000.0, 500.0, None).expect("node n3");
    session.add_element("e1", "n1", "n2", None, None).expect("element e1");

    let before = session.summary().expect("summary").revision;
    let moved = session.update_node("n2", Some(1200.0), Some(50.0), None).expect("move n2");
    assert_eq!(moved.revision, before + 1);
    let reconnected = session.update_element("e1", None, Some("n3")).expect("reconnect e1");
    assert_eq!(reconnected.revision, moved.revision + 1);

    let loaded = Model::load(&path).expect("reload");
    let node = loaded.node("n2").expect("node n2");
    assert_eq!((node.x, node.y), (1200.0, 50.0));
    let element = loaded.elements.iter().find(|element| element.id == "e1").expect("element e1");
    assert_eq!((element.from.as_str(), element.to.as_str()), ("n1", "n3"));
}
