use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Model, OpenCae, OpenCaeOptions, Units, solve_deflection};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;
const I: f64 = 1.0e6;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-deflection-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn beam_model() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model
}

#[test]
fn simply_supported_uniform_load_matches_the_analytic_deflection() {
    let mut model = beam_model();
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model.elements[0].distributed_load = Some(-10.0);

    let result = solve_deflection(&model, 11).expect("deflection");
    let element = &result.elements[0];
    assert_eq!(element.stations.len(), 11);
    assert_close(element.length, L, 1e-12);

    let mid = &element.stations[5];
    assert_close(mid.position, L / 2.0, 1e-12);
    let expected = 5.0 * -10.0 * L.powi(4) / (384.0 * E * I);
    assert_close(mid.transverse, expected, 1e-9);
    assert!(mid.rotation.abs() < 1e-9, "mid-span rotation: {}", mid.rotation);

    // Both supports stay on their original positions.
    assert!(element.stations[0].transverse.abs() < 1e-12);
    assert!(element.stations[10].transverse.abs() < 1e-12);
    assert!(element.stations[0].x.abs() < 1e-12);
    assert_close(element.stations[10].x, L, 1e-12);

    assert_eq!(result.governing.element, "b1");
    assert_close(result.governing.position, L / 2.0, 1e-12);
    assert_close(result.governing.transverse, expected, 1e-9);
    assert_close(element.max_abs_transverse, expected.abs(), 1e-9);
}

#[test]
fn cantilever_tip_load_recovers_the_tip_displacement_and_rotation() {
    let mut model = beam_model();
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.add_load("n2", None, Some(-1000.0)).expect("tip load");

    let result = solve_deflection(&model, 5).expect("deflection");
    let tip = result.elements[0].stations.last().expect("tip station");

    assert_close(tip.transverse, -1000.0 * L.powi(3) / (3.0 * E * I), 1e-9);
    assert_close(tip.rotation, -1000.0 * L.powi(2) / (2.0 * E * I), 1e-9);
    assert_close(tip.y, tip.transverse, 1e-12);
    assert_close(tip.x, L, 1e-12);
}

#[test]
fn cantilever_uniform_load_matches_the_analytic_deflection() {
    let mut model = beam_model();
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.elements[0].distributed_load = Some(-10.0);

    let result = solve_deflection(&model, 9).expect("deflection");
    let tip = result.elements[0].stations.last().expect("tip station");

    assert_close(tip.transverse, -10.0 * L.powi(4) / (8.0 * E * I), 1e-9);
    assert_close(tip.rotation, -10.0 * L.powi(3) / (6.0 * E * I), 1e-9);
}

#[test]
fn linearly_varying_load_uses_the_exact_particular_solution() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(0.0), Some(-10.0)).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");

    let result = solve_deflection(&model, 9).expect("deflection");
    let tip = result.elements[0].stations.last().expect("tip station");

    // Cantilever with a triangular load growing to w at the free end:
    // delta = 11 w L^4 / (120 EI).
    assert_close(tip.transverse, 11.0 * -10.0 * L.powi(4) / (120.0 * E * I), 1e-9);
}

#[test]
fn truss_elements_are_sampled_as_straight_members() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("guide");
    model.add_load("n2", Some(1000.0), None).expect("load");

    let result = solve_deflection(&model, 3).expect("deflection");
    let element = &result.elements[0];
    for station in &element.stations {
        assert!(station.transverse.abs() < 1e-12);
        assert!(station.rotation.abs() < 1e-12);
        assert!(station.y.abs() < 1e-12);
    }
    let extension = 1000.0 * L / (E * 100.0);
    assert_close(element.stations[2].axial, extension, 1e-9);
    assert_close(element.stations[2].x, L + extension, 1e-9);
}

#[test]
fn deflection_sampling_requires_at_least_two_stations() {
    let model = beam_model();
    let error = solve_deflection(&model, 1).expect_err("one station");
    assert_eq!(error.code, "analysis/invalid-stations");
}

#[test]
fn session_writes_the_deflection_report() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, Some(I), Some(-10.0)).expect("beam");
    session.add_support_full("n1", true, true, false, true).expect("fixed");

    let out = directory.join("deflections.json");
    let result = session.deflections(7, Some(&out)).expect("deflections");
    assert_eq!(result.stations, 7);
    assert_eq!(result.governing.element, "b1");
    let written = fs::read_to_string(&out).expect("deflections.json");
    assert!(written.contains("\"open-cae-deflection-report\""), "report: {written}");
    assert!(written.contains("\"maxAbsTransverse\""));
}
