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

use cae_model::{
    ElementKind, ElementMaterial, ElementUpdate, EndRelease, Material, Model, OpenCae, OpenCaeOptions, Section, Units, solve,
    solve_deflection,
};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;
const I: f64 = 1.0e6;
const ALPHA: f64 = 12.0e-6;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn material(alpha: Option<f64>, section: Option<Section>) -> Material {
    Material {
        name: "steel".to_string(),
        youngs_modulus: E,
        area: A,
        density: 0.0,
        allowable_stress: None,
        alpha,
        conductivity: None,
        section,
    }
}

fn simply_supported_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_data(material(Some(ALPHA), None)).expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

fn cantilever(alpha: Option<f64>, section: Option<Section>) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_data(material(alpha, section)).expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model
}

fn beam_element<'a>(model: &'a Model, id: &str) -> &'a cae_model::Element {
    model.elements.iter().find(|element| element.id == id).expect("element")
}

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

#[test]
fn element_update_sets_a_uniform_load_and_matches_the_analytic_deflection() {
    let mut model = simply_supported_beam();
    let revision = model.revision;

    model
        .update_element_with("b1", &ElementUpdate { distributed_load: Some(-10.0), ..ElementUpdate::default() })
        .expect("uniform load");

    let element = beam_element(&model, "b1");
    assert_eq!(element.distributed_load, Some(-10.0));
    assert_eq!(element.distributed_load_end, None);
    assert_eq!(element.distributed_load_from, None);
    assert_eq!(element.distributed_load_to, None);
    assert_eq!(model.revision, revision + 1);

    let expected = 5.0 * -10.0 * L.powi(4) / (384.0 * E * I);
    let deflection = solve_deflection(&model, 11).expect("deflection");
    assert_close(deflection.elements[0].stations[5].transverse, expected, 1e-9);

    let solution = solve(&model).expect("solve");
    let total: f64 = solution.reactions.iter().map(|reaction| reaction.fy).sum();
    assert_close(total, 10.0 * L, 1e-9);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn element_update_with_only_w2_changes_the_end_intensity_to_a_triangular_load() {
    let mut model = simply_supported_beam();
    model
        .update_element_with("b1", &ElementUpdate { distributed_load: Some(-10.0), ..ElementUpdate::default() })
        .expect("uniform load");
    let revision = model.revision;

    model
        .update_element_with("b1", &ElementUpdate { distributed_load_end: Some(-20.0), ..ElementUpdate::default() })
        .expect("triangular load");

    let element = beam_element(&model, "b1");
    assert_eq!(element.distributed_load, Some(-10.0));
    assert_eq!(element.distributed_load_end, Some(-20.0));
    assert_eq!(model.revision, revision + 1);

    let solution = solve(&model).expect("solve");
    let reaction = |node: &str| solution.reactions.iter().find(|entry| entry.node == node).expect("reaction").fy;
    let (q1, q2) = (-10.0, -20.0);
    assert_close(reaction("n1"), -L * (q1 / 3.0 + q2 / 6.0), 1e-9);
    assert_close(reaction("n2"), -L * (q1 / 6.0 + q2 / 3.0), 1e-9);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn element_update_applies_and_clears_a_partial_span_load() {
    let mut model = simply_supported_beam();
    model
        .update_element_with("b1", &ElementUpdate { distributed_load: Some(-10.0), ..ElementUpdate::default() })
        .expect("uniform load");
    model
        .update_element_with(
            "b1",
            &ElementUpdate { distributed_load_from: Some(0.25), distributed_load_to: Some(0.75), ..ElementUpdate::default() },
        )
        .expect("partial span");

    let element = beam_element(&model, "b1");
    assert_eq!(element.distributed_load_from, Some(0.25));
    assert_eq!(element.distributed_load_to, Some(0.75));
    let solution = solve(&model).expect("partial solve");
    let total: f64 = solution.reactions.iter().map(|reaction| reaction.fy).sum();
    assert_close(total, 10.0 * L / 2.0, 1e-9);

    model.update_element_with("b1", &ElementUpdate { clear_load: true, ..ElementUpdate::default() }).expect("clear load");
    let element = beam_element(&model, "b1");
    assert_eq!(element.distributed_load, None);
    assert_eq!(element.distributed_load_end, None);
    assert_eq!(element.distributed_load_from, None);
    assert_eq!(element.distributed_load_to, None);

    let cleared = solve(&model).expect("cleared solve");
    for reaction in &cleared.reactions {
        assert_eq!(reaction.fy, 0.0);
    }
    let deflection = solve_deflection(&model, 11).expect("cleared deflection");
    assert!(deflection.elements[0].stations.iter().all(|station| station.transverse == 0.0));
}

#[test]
fn element_update_sets_and_clears_a_uniform_temperature_and_restores_the_solution() {
    let mut model = cantilever(Some(ALPHA), None);
    let baseline = solve(&model).expect("baseline");

    model
        .update_element_with("b1", &ElementUpdate { temperature: Some(50.0), ..ElementUpdate::default() })
        .expect("temperature");
    let element = beam_element(&model, "b1");
    assert_eq!(element.temperature, Some(50.0));
    assert_eq!(element.temperature_gradient, None);

    let heated = solve(&model).expect("heated");
    let tip = heated.nodes.iter().find(|node| node.id == "n2").expect("n2");
    assert_close(tip.ux, ALPHA * 50.0 * L, 1e-12);

    model
        .update_element_with("b1", &ElementUpdate { clear_temperature: true, ..ElementUpdate::default() })
        .expect("clear temperature");
    let element = beam_element(&model, "b1");
    assert_eq!(element.temperature, None);
    assert_eq!(element.temperature_gradient, None);
    assert_eq!(solve(&model).expect("restored"), baseline);
}

#[test]
fn element_update_sets_and_clears_a_gradient_and_restores_the_solution() {
    let section = Some(Section::Rectangle { width: 100.0, height: 200.0 });
    let mut model = cantilever(Some(ALPHA), section);
    let baseline = solve(&model).expect("baseline");

    model
        .update_element_with("b1", &ElementUpdate { temperature_gradient: Some(40.0), ..ElementUpdate::default() })
        .expect("gradient");
    assert_eq!(beam_element(&model, "b1").temperature_gradient, Some(40.0));
    let bent = solve(&model).expect("bent");
    let tip = bent.nodes.iter().find(|node| node.id == "n2").expect("n2");
    assert_close(tip.rz, -ALPHA * 40.0 / 200.0 * L, 1e-12);

    model
        .update_element_with("b1", &ElementUpdate { clear_temperature: true, ..ElementUpdate::default() })
        .expect("clear gradient");
    assert_eq!(beam_element(&model, "b1").temperature_gradient, None);
    assert_eq!(solve(&model).expect("restored"), baseline);
}

#[test]
fn element_update_rejects_conflicts_empty_requests_and_unknown_elements() {
    let mut model = simply_supported_beam();
    let revision = model.revision;

    let load_conflicts = [
        ElementUpdate { clear_load: true, distributed_load: Some(-1.0), ..ElementUpdate::default() },
        ElementUpdate { clear_load: true, distributed_load_end: Some(-1.0), ..ElementUpdate::default() },
        ElementUpdate {
            clear_load: true,
            distributed_load_from: Some(0.2),
            distributed_load_to: Some(0.8),
            ..ElementUpdate::default()
        },
        ElementUpdate { clear_load: true, distributed_load_from: Some(0.2), ..ElementUpdate::default() },
        ElementUpdate { clear_load: true, distributed_load_to: Some(0.8), ..ElementUpdate::default() },
    ];
    for update in load_conflicts {
        assert_eq!(model.update_element_with("b1", &update).expect_err("load conflict").code, "session/argument-invalid");
    }
    let temperature_conflicts = [
        ElementUpdate { clear_temperature: true, temperature: Some(10.0), ..ElementUpdate::default() },
        ElementUpdate { clear_temperature: true, temperature_gradient: Some(10.0), ..ElementUpdate::default() },
    ];
    for update in temperature_conflicts {
        assert_eq!(
            model.update_element_with("b1", &update).expect_err("temperature conflict").code,
            "session/argument-invalid"
        );
    }
    assert_eq!(
        model.update_element_with("b1", &ElementUpdate::default()).expect_err("empty request").code,
        "session/argument-invalid"
    );
    assert_eq!(
        model
            .update_element_with("b1", &ElementUpdate { distributed_load_from: Some(0.25), ..ElementUpdate::default() })
            .expect_err("lone start fraction")
            .code,
        "element/invalid-load-span"
    );
    assert_eq!(
        model
            .update_element_with("b1", &ElementUpdate { distributed_load_to: Some(0.75), ..ElementUpdate::default() })
            .expect_err("lone end fraction")
            .code,
        "element/invalid-load-span"
    );
    assert_eq!(
        model
            .update_element_with(
                "b1",
                &ElementUpdate {
                    distributed_load: Some(-1.0),
                    distributed_load_from: Some(0.75),
                    distributed_load_to: Some(0.25),
                    ..ElementUpdate::default()
                },
            )
            .expect_err("reversed span")
            .code,
        "element/invalid-load-span"
    );
    assert_eq!(
        model
            .update_element_with("missing", &ElementUpdate { temperature: Some(1.0), ..ElementUpdate::default() })
            .expect_err("unknown element")
            .code,
        "element/unknown-id"
    );

    assert_eq!(model.revision, revision);
    let element = beam_element(&model, "b1");
    assert_eq!(element.distributed_load, None);
    assert_eq!(element.temperature, None);
}

#[test]
fn element_update_changes_one_revision_and_preserves_unnamed_values() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_node("n3", L, 500.0).expect("n3");
    let material_override = ElementMaterial { youngs_modulus: Some(100_000.0), area: Some(50.0) };
    model
        .add_element_with_loads(
            "b1",
            "n1",
            "n2",
            Some(material_override.clone()),
            ElementKind::Beam,
            Some(I),
            Some(-2.0),
            Some(-4.0),
        )
        .expect("beam");
    model.set_element_temperature("b1", Some(30.0)).expect("temperature");
    model.set_element_release("b1", Some(EndRelease { start: true, end: false })).expect("release");
    let revision = model.revision;

    model
        .update_element_with(
            "b1",
            &ElementUpdate {
                to: Some("n3".to_string()),
                distributed_load: Some(-5.0),
                temperature: Some(40.0),
                temperature_gradient: Some(10.0),
                convection: Some(15.0),
                perimeter: Some(2.0),
                ..ElementUpdate::default()
            },
        )
        .expect("combined update");

    let element = beam_element(&model, "b1");
    assert_eq!(model.revision, revision + 1);
    assert_eq!((element.from.as_str(), element.to.as_str()), ("n1", "n3"));
    assert_eq!(element.material.as_ref(), Some(&material_override));
    assert_eq!(element.inertia, Some(I));
    assert_eq!(element.kind, ElementKind::Beam);
    assert_eq!(element.release, Some(EndRelease { start: true, end: false }));
    assert!(!element.tension_only);
    assert_eq!(element.distributed_load, Some(-5.0));
    assert_eq!(element.distributed_load_end, None);
    assert_eq!(element.distributed_load_from, None);
    assert_eq!(element.distributed_load_to, None);
    assert_eq!(element.temperature, Some(40.0));
    assert_eq!(element.temperature_gradient, Some(10.0));
    assert_eq!(element.convection, Some(15.0));
    assert_eq!(element.perimeter, Some(2.0));
}

#[test]
fn session_element_update_round_trips_load_and_temperature() {
    let directory = temporary_dir("element-actions");
    let path = directory.join("demo.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.set_material(E, A, Some("steel".to_string())).expect("material");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, Some(I), Some(-2.0)).expect("beam");
    let before = session.summary().expect("summary").revision;

    let summary = session
        .update_element_with(
            "b1",
            &ElementUpdate {
                distributed_load: Some(-4.0),
                distributed_load_end: Some(-6.0),
                distributed_load_from: Some(0.2),
                distributed_load_to: Some(0.8),
                temperature: Some(20.0),
                temperature_gradient: Some(5.0),
                ..ElementUpdate::default()
            },
        )
        .expect("update");
    assert_eq!(summary.revision, before + 1);

    let loaded = Model::load(&path).expect("reload");
    let element = beam_element(&loaded, "b1");
    assert_eq!(element.distributed_load, Some(-4.0));
    assert_eq!(element.distributed_load_end, Some(-6.0));
    assert_eq!(element.distributed_load_from, Some(0.2));
    assert_eq!(element.distributed_load_to, Some(0.8));
    assert_eq!(element.temperature, Some(20.0));
    assert_eq!(element.temperature_gradient, Some(5.0));
}
