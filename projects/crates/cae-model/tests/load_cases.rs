use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Material, Model, OpenCae, OpenCaeOptions, Units, summarize_load_cases};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;
const A: f64 = 100.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-load-cases-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn axial_bar() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("guide");
    model.add_load("n2", Some(1000.0), None).expect("load");
    model
}

#[test]
fn load_case_summary_scales_the_unfactored_case_and_combinations() {
    let mut model = axial_bar();
    model.add_combination("double", 2.0, 0.0).expect("double");
    model.add_combination("half", 0.5, 0.0).expect("half");

    let summary = summarize_load_cases(&model).expect("summary");
    assert_eq!(summary.cases.len(), 3);

    let extension = 1000.0 * L / (E * A);
    let base = &summary.cases[0];
    assert_eq!(base.name, "base");
    assert_close(base.loads, 1.0, 1e-12);
    assert_close(base.gravity, 1.0, 1e-12);
    assert_close(base.applied.fx, 1000.0, 1e-12);
    assert_close(base.max_displacement, extension, 1e-9);
    assert_eq!(base.max_displacement_node, "n2");
    assert_close(base.max_axial_force, 1000.0, 1e-9);
    assert_eq!(base.max_axial_element, "e1");
    assert_close(base.strain_energy, 0.5 * 1000.0 * extension, 1e-9);
    assert!(base.balanced);

    let double = &summary.cases[1];
    assert_eq!(double.name, "double");
    assert_close(double.applied.fx, 2000.0, 1e-12);
    assert_close(double.max_displacement, 2.0 * extension, 1e-9);
    assert_close(double.max_axial_force, 2000.0, 1e-9);
    assert_close(double.strain_energy, 4.0 * base.strain_energy, 1e-9);
    assert!(double.balanced);

    let half = &summary.cases[2];
    assert_eq!(half.name, "half");
    assert_close(half.applied.fx, 500.0, 1e-12);
    assert_close(half.max_displacement, 0.5 * extension, 1e-9);
    assert_close(half.strain_energy, 0.25 * base.strain_energy, 1e-9);
}

#[test]
fn load_case_summary_reports_gravity_dominated_cases() {
    let mut model = Model::new(Units::Millimeter);
    model
        .set_material_data(Material {
            name: "steel".to_string(),
            youngs_modulus: E,
            area: A,
            density: 1.0e-9,
            allowable_stress: None,
            alpha: None,
            conductivity: None,
            shear_modulus: None,
            poissons_ratio: None,
            section: None,
        })
        .expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 0.0, L).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("fixed");
    model.add_support("n2", true, false).expect("guide");
    model.set_gravity(0.0, -10.0, 0.0).expect("gravity");
    model.add_combination("self-weight x2", 0.0, 2.0).expect("combination");

    let summary = summarize_load_cases(&model).expect("summary");
    let base = &summary.cases[0];
    let doubled = &summary.cases[1];
    assert!(base.applied.fy < 0.0);
    assert_close(doubled.applied.fy, 2.0 * base.applied.fy, 1e-9);
    assert_close(doubled.max_displacement, 2.0 * base.max_displacement, 1e-9);
    assert_close(doubled.strain_energy, 4.0 * base.strain_energy, 1e-9);
    assert!(doubled.balanced);
}

#[test]
fn a_model_without_combinations_has_only_the_base_case() {
    let summary = summarize_load_cases(&axial_bar()).expect("summary");
    assert_eq!(summary.cases.len(), 1);
    assert_eq!(summary.cases[0].name, "base");
}

#[test]
fn session_writes_the_load_case_summary() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("e1", "n1", "n2", None, None, ElementKind::Truss, None, None).expect("bar");
    session.add_support("n1", true, true, false).expect("pin");
    session.add_support("n2", false, true, false).expect("guide");
    session.add_load("n2", Some(1000.0), None, None).expect("load");
    session.add_combination("ultimate", 1.5, 1.0).expect("combination");

    let out = directory.join("load-cases.json");
    let result = session.load_cases(Some(&out)).expect("load cases");
    assert_eq!(result.cases.len(), 2);
    assert_eq!(result.cases[0].name, "base");
    assert_eq!(result.cases[1].name, "ultimate");
    let written = fs::read_to_string(&out).expect("load-cases.json");
    assert!(written.contains("\"open-cae-load-case-report\""), "report: {written}");
    assert!(written.contains("\"maxAbsMoment\""));
}
