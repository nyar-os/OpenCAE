use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Material, Model, OpenCae, OpenCaeOptions, Section, Units, solve_nodal_stress};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-nodal-stress-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn material(section: Section) -> Material {
    Material {
        name: "steel".to_string(),
        youngs_modulus: E,
        area: section.properties().expect("section").area,
        density: 0.0,
        allowable_stress: Some(235.0),
        alpha: None,
        section: Some(section),
    }
}

fn cantilever(section: Section) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_data(material(section)).expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, None, None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.add_load("n2", None, Some(-1000.0)).expect("load");
    model
}

#[test]
fn cantilever_end_stresses_are_averaged_per_node() {
    let section = Section::rectangle(10.0, 20.0).expect("section");
    let modulus = section.properties().expect("properties").section_modulus();
    let model = cantilever(section);
    let result = solve_nodal_stress(&model, 5).expect("nodal stress");

    let stress = 1000.0 * L / modulus;
    let fixed = &result.nodes[0];
    assert_eq!(fixed.node, "n1");
    assert_eq!(fixed.connected_elements, 1);
    assert_close(fixed.mean_axial_stress, 0.0, 1e-12);
    assert_close(fixed.min_stress, -stress, 1e-9);
    assert_close(fixed.max_stress, stress, 1e-9);
    assert_close(fixed.max_abs_stress, stress, 1e-9);
    assert_eq!(fixed.governing_element, "b1");

    let free = &result.nodes[1];
    assert_eq!(free.connected_elements, 1);
    assert!(free.max_abs_stress.abs() < 1e-9);

    assert_eq!(result.governing.node, "n1");
    assert_eq!(result.governing.element, "b1");
    assert_close(result.governing.max_abs_stress, stress, 1e-9);
    assert_close(result.governing.utilization.expect("utilization"), stress / 235.0, 1e-9);
}

#[test]
fn shared_node_combines_the_connected_element_ends() {
    let section = Section::rectangle(10.0, 20.0).expect("section");
    let modulus = section.properties().expect("properties").section_modulus();
    let mut model = Model::new(Units::Millimeter);
    model.set_material_data(material(section)).expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L / 2.0, 0.0).expect("n2");
    model.add_node("n3", L, 0.0).expect("n3");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, None, None).expect("b1");
    model.add_element_kind("b2", "n2", "n3", None, ElementKind::Beam, None, None).expect("b2");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.add_load("n3", None, Some(-1000.0)).expect("load");

    let result = solve_nodal_stress(&model, 3).expect("nodal stress");
    let shared = &result.nodes[1];
    let stress = 1000.0 * (L / 2.0) / modulus;
    assert_eq!(shared.node, "n2");
    assert_eq!(shared.connected_elements, 2);
    assert_close(shared.mean_axial_stress, 0.0, 1e-12);
    assert_close(shared.min_stress, -stress, 1e-9);
    assert_close(shared.max_stress, stress, 1e-9);
    assert_close(shared.max_abs_stress, stress, 1e-9);
    assert!(["b1", "b2"].contains(&shared.governing_element.as_str()));

    // The fixed end carries the full cantilever moment and governs the report.
    assert_eq!(result.governing.node, "n1");
    assert_close(result.governing.max_abs_stress, 1000.0 * L / modulus, 1e-9);
}

#[test]
fn truss_nodes_report_the_axial_stress_without_utilisation() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("guide");
    model.add_load("n2", Some(1000.0), None).expect("load");

    let result = solve_nodal_stress(&model, 4).expect("nodal stress");
    for node in &result.nodes {
        assert_eq!(node.connected_elements, 1);
        assert_close(node.mean_axial_stress, 10.0, 1e-9);
        assert_close(node.min_stress, 10.0, 1e-9);
        assert_close(node.max_stress, 10.0, 1e-9);
        assert_close(node.max_abs_stress, 10.0, 1e-9);
    }
    assert!(result.allowable_stress.is_none());
    assert!(result.governing.utilization.is_none());
    assert_eq!(result.governing.node, "n1");
}

#[test]
fn nodal_stress_requires_at_least_two_stations() {
    let model = cantilever(Section::rectangle(10.0, 20.0).expect("section"));
    let error = solve_nodal_stress(&model, 1).expect_err("one station");
    assert_eq!(error.code, "analysis/invalid-stations");
}

#[test]
fn session_writes_the_nodal_stress_report() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session
        .apply_material_preset("steel", None, Some(Section::rectangle(10.0, 20.0).expect("section")), Some(235.0))
        .expect("preset");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, None, None).expect("beam");
    session.add_support_full("n1", true, true, false, true).expect("fixed");
    session.add_load("n2", None, Some(-1000.0), None).expect("load");

    let out = directory.join("nodal-stress.json");
    let result = session.nodal_stresses(5, Some(&out)).expect("nodal stresses");
    assert_eq!(result.governing.node, "n1");
    let written = fs::read_to_string(&out).expect("nodal-stress.json");
    assert!(written.contains("\"open-cae-nodal-stress-report\""), "report: {written}");
    assert!(written.contains("\"meanAxialStress\""));
}
