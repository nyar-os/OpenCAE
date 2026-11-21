use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Material, Model, OpenCae, OpenCaeOptions, Section, Units, solve_stress};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-stress-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn cantilever(section: Option<Section>, tip_axial: Option<f64>) -> Model {
    let area = section.and_then(|section| section.properties().ok()).map(|properties| properties.area).unwrap_or(100.0);
    let mut model = Model::new(Units::Millimeter);
    model
        .set_material_data(Material {
            name: "steel".to_string(),
            youngs_modulus: E,
            area,
            density: 0.0,
            allowable_stress: Some(235.0),
            alpha: None,
            section,
        })
        .expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    let inertia = if section.is_some() { None } else { Some(1.0e6) };
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, inertia, None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.add_load("n2", tip_axial, Some(-1000.0)).expect("tip load");
    model
}

#[test]
fn cantilever_tip_load_recovers_the_extreme_fibre_stress() {
    let section = Section::rectangle(10.0, 20.0).expect("section");
    let properties = section.properties().expect("properties");
    let model = cantilever(Some(section), None);
    let result = solve_stress(&model, 5).expect("stress");

    let element = &result.elements[0];
    assert_close(element.section_modulus, properties.section_modulus(), 1e-12);
    assert_close(element.area, 200.0, 1e-12);

    // Fixed-end moment is -P·L, so |σ| = P·L/S.
    let expected = 1000.0 * L / properties.section_modulus();
    assert_close(element.max_abs_stress, expected, 1e-9);
    assert_close(element.stations[0].max_stress, expected, 1e-9);
    assert_close(element.stations[0].min_stress, -expected, 1e-9);
    assert_close(element.stations[0].axial_stress, 0.0, 1e-12);
    assert_eq!(element.governing_station, 0);
    assert_close(element.governing_position, 0.0, 1e-12);
    assert_close(element.utilization.expect("utilization"), expected / 235.0, 1e-9);

    assert_eq!(result.governing.element, "b1");
    assert_close(result.governing.max_abs_stress, expected, 1e-9);
    assert_eq!(result.allowable_stress, Some(235.0));

    // The free end carries no bending moment.
    let free = element.stations.last().expect("free end");
    assert!(free.max_stress.abs() < 1e-9, "free-end max stress: {}", free.max_stress);
    assert!(free.min_stress.abs() < 1e-9, "free-end min stress: {}", free.min_stress);
}

#[test]
fn axial_force_shifts_the_extreme_fibre_stresses() {
    let section = Section::rectangle(10.0, 20.0).expect("section");
    let properties = section.properties().expect("properties");
    let model = cantilever(Some(section), Some(10_000.0));
    let result = solve_stress(&model, 3).expect("stress");

    let element = &result.elements[0];
    let axial = 10_000.0 / 200.0;
    let bending = 1000.0 * L / properties.section_modulus();
    assert_close(element.stations[0].axial_stress, axial, 1e-9);
    assert_close(element.stations[0].min_stress, axial - bending, 1e-9);
    assert_close(element.stations[0].max_stress, axial + bending, 1e-9);
    assert_close(element.max_abs_stress, axial + bending, 1e-9);
}

#[test]
fn truss_stress_recovery_needs_no_section() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("guide");
    model.add_load("n2", Some(1000.0), None).expect("load");

    let result = solve_stress(&model, 4).expect("stress");
    let element = &result.elements[0];
    assert_close(element.section_modulus, 0.0, 1e-12);
    for station in &element.stations {
        assert_close(station.axial_stress, 10.0, 1e-9);
        assert_close(station.min_stress, 10.0, 1e-12);
        assert_close(station.max_stress, 10.0, 1e-12);
    }
    assert!(element.utilization.is_none());
}

#[test]
fn beam_stress_recovery_requires_a_section() {
    let model = cantilever(None, None);
    let error = solve_stress(&model, 3).expect_err("missing section");
    assert_eq!(error.code, "material/missing-section");
}

#[test]
fn stress_recovery_requires_at_least_two_stations() {
    let model = cantilever(Some(Section::rectangle(10.0, 20.0).expect("section")), None);
    let error = solve_stress(&model, 1).expect_err("one station");
    assert_eq!(error.code, "analysis/invalid-stations");
}

#[test]
fn session_writes_the_stress_report() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.apply_material_preset("steel", None, Some(Section::rectangle(10.0, 20.0).expect("section")), None).expect("preset");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, None, None).expect("beam");
    session.add_support_full("n1", true, true, false, true).expect("fixed");
    session.add_load("n2", None, Some(-1000.0), None).expect("load");

    let out = directory.join("stress.json");
    let result = session.stresses(7, Some(&out)).expect("stresses");
    assert_eq!(result.stations, 7);
    assert_eq!(result.governing.element, "b1");
    let written = fs::read_to_string(&out).expect("stress.json");
    assert!(written.contains("\"open-cae-stress-report\""), "report: {written}");
    assert!(written.contains("\"maxAbsStress\""));
}
