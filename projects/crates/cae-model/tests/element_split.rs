//! Parametric element splitting.
//!
//! Splitting is an ordinary atomic edit: it inserts an interior node, keeps the
//! original element id for the first segment and appends the second segment.
//! The structural solution must not change, uniform distributed loads keep the
//! same total action and loads that cannot be divided losslessly are rejected.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, ElementMaterial, EndRelease, Model, OpenCae, OpenCaeOptions, Units, solve};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-split-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn axial_bar() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("e1");
    model.add_support("n1", true, true).expect("pin n1");
    model.add_support("n2", false, true).expect("roller n2");
    model.add_load("n2", Some(1000.0), None).expect("load n2");
    model
}

fn cantilever() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None, None).expect("b1");
    model.add_support_full("n1", true, true, false, true).expect("fixed n1");
    model.add_load("n2", None, Some(-1000.0)).expect("load n2");
    model
}

#[test]
fn split_preserves_the_axial_bar_solution() {
    let mut model = axial_bar();
    let before = solve(&model).expect("solve before");
    let expected = 1000.0 * L / (E * A);
    assert_close(before.nodes[1].ux, expected, 1e-12);

    let split = model.split_element("e1", 0.4).expect("split e1");

    assert_eq!(split.first, "e1");
    assert_eq!(split.second, "e1-b");
    assert_eq!(split.node, "e1-mid");
    let node = model.node(&split.node).expect("interior node");
    assert_eq!(node.x, 400.0);
    assert_eq!(node.y, 0.0);
    assert_eq!(node.z, 0.0);
    model.add_support("e1-mid", false, true).expect("roller interior node");
    let lengths: Vec<f64> = model
        .elements
        .iter()
        .map(|element| {
            let from = model.node(&element.from).expect("from");
            let to = model.node(&element.to).expect("to");
            ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt()
        })
        .collect();
    assert_eq!(lengths, vec![400.0, 600.0]);

    let after = solve(&model).expect("solve after");
    assert_close(after.nodes[1].ux, expected, 1e-12);
    assert_close(after.nodes[0].ux, 0.0, 1e-12);
    for element in &after.elements {
        assert_close(element.axial_force, 1000.0, 1e-12);
    }
}

#[test]
fn split_preserves_the_cantilever_tip_deflection() {
    let mut model = cantilever();
    let expected = -1000.0 * L.powi(3) / (3.0 * E * I);
    let before = solve(&model).expect("solve before");
    assert_close(before.nodes[1].uy, expected, 1e-9);

    model.split_element("b1", 0.25).expect("split b1");

    let after = solve(&model).expect("solve after");
    assert_close(after.nodes[1].uy, expected, 1e-9);
    let fixed = &after.reactions[0];
    assert_close(fixed.fy, 1000.0, 1e-9);
    assert_close(fixed.mz, 1000.0 * L, 1e-9);
}

#[test]
fn split_copies_element_attributes_and_moves_releases_to_the_outer_ends() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    let material = ElementMaterial { youngs_modulus: Some(100_000.0), area: Some(50.0) };
    model
        .add_element_with_loads("b1", "n1", "n2", Some(material.clone()), ElementKind::Beam, Some(I), Some(-10.0), Some(-10.0))
        .expect("b1");
    model.set_element_temperature("b1", Some(30.0)).expect("temperature");
    model.set_element_release("b1", Some(EndRelease { start: true, end: false })).expect("release");

    let split = model.split_element("b1", 0.5).expect("split b1");

    let first = model.elements.iter().find(|element| element.id == split.first).expect("first");
    let second = model.elements.iter().find(|element| element.id == split.second).expect("second");
    for element in [first, second] {
        assert_eq!(element.material.as_ref(), Some(&material));
        assert_eq!(element.kind, ElementKind::Beam);
        assert_eq!(element.inertia, Some(I));
        assert_eq!(element.distributed_load, Some(-10.0));
        assert_eq!(element.distributed_load_end, None, "the uniform load stays uniform");
        assert_eq!(element.temperature, Some(30.0));
    }
    assert_eq!(first.release, Some(EndRelease { start: true, end: false }));
    assert_eq!(second.release, None, "the interior node stays continuous");
}

#[test]
fn split_reuses_an_existing_node_at_the_split_point() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_node("mid", 500.0, 0.0).expect("mid");
    model.add_element("e1", "n1", "n2", None).expect("e1");
    let revision = model.revision;

    let split = model.split_element("e1", 0.5).expect("split e1");

    assert_eq!(split.node, "mid");
    assert!(split.reused_node);
    assert_eq!(model.nodes.len(), 3, "no extra node is created");
    assert_eq!(model.revision, revision + 1);
    let first = model.elements.iter().find(|element| element.id == "e1").expect("first");
    let second = model.elements.iter().find(|element| element.id == split.second).expect("second");
    assert_eq!((first.from.as_str(), first.to.as_str()), ("n1", "mid"));
    assert_eq!((second.from.as_str(), second.to.as_str()), ("mid", "n2"));
}

#[test]
fn split_rejects_invalid_fractions_unknown_ids_and_unsupported_loads() {
    let mut model = axial_bar();
    let revision = model.revision;
    for at in [f64::NAN, f64::INFINITY, 0.0, 1.0, -0.1, 1.5] {
        assert_eq!(model.split_element("e1", at).expect_err("invalid fraction").code, "element/invalid-fraction");
    }
    assert_eq!(model.split_element("missing", 0.5).expect_err("unknown element").code, "element/unknown-id");
    assert_eq!(model.revision, revision);
    assert_eq!(model.elements.len(), 1);
    assert_eq!(model.nodes.len(), 2);

    let mut linear = Model::new(Units::Millimeter);
    linear.add_node("n1", 0.0, 0.0).expect("n1");
    linear.add_node("n2", L, 0.0).expect("n2");
    linear.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(-2.0), Some(-4.0)).expect("b1");
    let revision = linear.revision;
    assert_eq!(linear.split_element("b1", 0.5).expect_err("linear load").code, "element/split-unsupported-load");
    assert_eq!(linear.revision, revision);
    assert_eq!(linear.elements.len(), 1);

    let mut partial = Model::new(Units::Millimeter);
    partial.add_node("n1", 0.0, 0.0).expect("n1");
    partial.add_node("n2", L, 0.0).expect("n2");
    partial.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(-2.0), Some(-2.0)).expect("b1");
    partial.set_element_load_span("b1", Some((0.25, 0.75))).expect("span");
    assert_eq!(partial.split_element("b1", 0.5).expect_err("partial span").code, "element/split-unsupported-load");

    let mut gradient = Model::new(Units::Millimeter);
    gradient.add_node("n1", 0.0, 0.0).expect("n1");
    gradient.add_node("n2", L, 0.0).expect("n2");
    gradient.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None, None).expect("b1");
    gradient.set_element_temperature_gradient("b1", Some(5.0)).expect("gradient");
    assert_eq!(gradient.split_element("b1", 0.5).expect_err("gradient").code, "element/split-unsupported-load");
}

#[test]
fn split_keeps_the_total_reaction_of_a_uniform_load() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(-10.0), None).expect("b1");
    model.add_support("n1", true, true).expect("pin n1");
    model.add_support("n2", false, true).expect("roller n2");
    let before = solve(&model).expect("solve before");

    model.split_element("b1", 0.3).expect("split b1");

    for element in &model.elements {
        assert_eq!(element.distributed_load, Some(-10.0));
        assert_eq!(element.distributed_load_end, None);
    }
    let after = solve(&model).expect("solve after");
    for index in [0, 1] {
        assert_close(after.reactions[index].fy, before.reactions[index].fy, 1e-12);
        assert_close(after.reactions[index].fy, 10.0 * L / 2.0, 1e-12);
    }
    assert!(after.reaction_summary.balanced);
}

#[test]
fn session_split_persists_the_revision_and_supports_undo_and_redo() {
    let directory = temporary_dir("session");
    let path = directory.join("split.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element("e1", "n1", "n2", None, None).expect("e1");
    let before = session.summary().expect("summary").revision;

    let split = session.split_element("e1", 0.5).expect("split");
    assert_eq!(split.summary.revision, before + 1);
    assert_eq!(split.summary.nodes, 3);
    assert_eq!(split.summary.elements, 2);
    assert_eq!(split.split.node, "e1-mid");
    assert_eq!(split.split.first, "e1");
    assert_eq!(split.split.second, "e1-b");

    let reopened = Model::load(&path).expect("reload");
    assert_eq!(reopened.nodes.len(), 3);
    assert_eq!(reopened.elements.len(), 2);
    assert_eq!(reopened.node("e1-mid").expect("mid").x, 500.0);

    let undone = session.update_element("e1", None, Some("n2")).expect("undo endpoints");
    assert_eq!(undone.revision, split.summary.revision + 1);
    session.remove_element("e1-b").expect("undo segment");
    let undone = session.remove_node("e1-mid").expect("undo node");
    assert_eq!(undone.nodes, 2);
    assert_eq!(undone.elements, 1);
    let restored = Model::load(&path).expect("reload undone");
    let element = restored.elements.iter().find(|element| element.id == "e1").expect("e1");
    assert_eq!((element.from.as_str(), element.to.as_str()), ("n1", "n2"));

    let redone = session.split_element("e1", 0.5).expect("redo");
    assert_eq!(redone.split.node, "e1-mid");
    assert_eq!(redone.split.second, "e1-b");
    assert_eq!(redone.summary.revision, undone.revision + 1);
}

#[test]
fn split_does_not_move_nodal_loads_or_supports() {
    let mut model = axial_bar();
    model.add_support("n2", false, true).expect("roller n2");

    let split = model.split_element("e1", 0.5).expect("split e1");

    assert_eq!(model.loads.len(), 1);
    assert_eq!(model.loads[0].node, "n2");
    assert_eq!(model.supports.len(), 2);
    assert!(model.supports.iter().all(|support| support.node != split.node));
}
