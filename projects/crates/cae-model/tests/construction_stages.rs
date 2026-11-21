use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ConstructionStage, ElementKind, Model, OpenCae, OpenCaeOptions, Units, solve_construction_stages};

const E: f64 = 210_000.0;
const I: f64 = 1e6;
const L: f64 = 1000.0;
const P: f64 = 1000.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-stages-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

/// Two-element cantilever `n1-n2-n3` with a load at the shared node `n2`.
///
/// `n1` is fully fixed and `n3` is pinned in translation, so activating only
/// `e1` is a cantilever (`PL³/3EI`) and activating both members is a propped
/// cantilever with a midspan load (`7PL³/768EI`).
fn two_element_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, 100.0, 0.0);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_node("n3", 2.0 * L, 0.0).expect("n3");
    model.add_element_kind("e1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("e1");
    model.add_element_kind("e2", "n2", "n3", None, ElementKind::Beam, Some(I), None).expect("e2");
    model.add_support("n1", true, true).expect("fixed translation");
    model.add_support("n3", true, true).expect("pinned translation");
    model.add_support_full("n1", true, true, false, true).expect("fixed rotation");
    model.add_load("n2", None, Some(-P)).expect("load");
    model
}

fn stages(names: &[&str], elements: &[Option<Vec<&str>>], factors: &[f64]) -> Vec<ConstructionStage> {
    names
        .iter()
        .zip(elements)
        .zip(factors)
        .map(|((name, elements), factor)| ConstructionStage {
            name: (*name).to_string(),
            elements: elements.as_ref().map(|ids| ids.iter().map(|id| (*id).to_string()).collect()),
            loads: *factor,
            gravity: 0.0,
        })
        .collect()
}

#[test]
fn stage_activation_changes_the_active_stiffness_path() {
    let model = two_element_beam();
    let request =
        stages(&["erection", "completed", "completed-x2"], &[Some(vec!["e1"]), Some(vec!["e1", "e2"]), None], &[1.0, 1.0, 2.0]);
    let result = solve_construction_stages(&model, &request).expect("stages");
    assert_eq!(result.stages.len(), 3);
    assert_eq!(result.stages[0].name, "erection");
    assert_eq!(result.stages[0].active_elements, 1);
    assert_eq!(result.stages[1].active_elements, 2);
    assert_eq!(result.stages[2].active_elements, 2);

    let cantilever = P * L.powi(3) / (3.0 * E * I);
    // The textbook propped-cantilever formula uses the total span, which is 2L.
    let propped = 7.0 * P * (2.0 * L).powi(3) / (768.0 * E * I);
    assert_close(result.stages[0].max_displacement, cantilever, 1e-9);
    assert_eq!(result.stages[0].max_displacement_node, "n2");
    assert_close(result.stages[0].max_axial_force, 0.0, 1e-12);
    assert_close(result.stages[0].applied.fy, -P, 1e-12);
    assert!(result.stages[0].balanced);

    assert_close(result.stages[1].max_displacement, propped, 1e-9);
    assert_eq!(result.stages[1].max_displacement_node, "n2");
    assert_close(result.stages[1].applied.fy, -P, 1e-12);
    assert!(result.stages[1].balanced);

    assert_close(result.stages[2].max_displacement, 2.0 * propped, 1e-9);
    assert_close(result.stages[2].applied.fy, -2.0 * P, 1e-12);
    assert_close(result.stages[2].strain_energy, 4.0 * result.stages[1].strain_energy, 1e-9);
}

#[test]
fn invalid_stage_definitions_are_rejected() {
    let model = two_element_beam();
    let empty: Vec<ConstructionStage> = Vec::new();
    assert_eq!(solve_construction_stages(&model, &empty).expect_err("empty").code, "stage/none");

    let mut unnamed = stages(&[""], &[None], &[1.0]);
    assert_eq!(solve_construction_stages(&model, &unnamed).expect_err("empty name").code, "stage/invalid-name");
    unnamed[0].name = "ok".to_string();
    unnamed[0].loads = f64::NAN;
    assert_eq!(solve_construction_stages(&model, &unnamed).expect_err("nan factor").code, "stage/invalid-factor");

    let duplicate = stages(&["same", "same"], &[None, None], &[1.0, 1.0]);
    assert_eq!(solve_construction_stages(&model, &duplicate).expect_err("duplicate").code, "stage/duplicate-name");

    let unknown = stages(&["bad"], &[Some(vec!["missing"])], &[1.0]);
    assert_eq!(solve_construction_stages(&model, &unknown).expect_err("unknown element").code, "stage/unknown-element");

    let none_active = stages(&["nothing"], &[Some(vec![])], &[1.0]);
    assert_eq!(solve_construction_stages(&model, &none_active).expect_err("no element").code, "stage/no-elements");
}

#[test]
fn session_writes_a_construction_stage_report() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.set_material_full(E, 100.0, 0.0, None).expect("material");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_full("e1", "n1", "n2", None, None, ElementKind::Beam, Some(I), None).expect("e1");
    session.add_support_full("n1", true, true, false, true).expect("fixed");
    session.add_load("n2", None, Some(-P), None).expect("load");

    let out = directory.join("stages.json");
    let request = stages(&["single"], &[None], &[1.0]);
    let result = session.construction_stages(&request, Some(&out)).expect("stages");
    assert_eq!(result.format, "open-cae-construction-stage-report");
    let text = fs::read_to_string(out).expect("read report");
    assert!(text.contains("open-cae-construction-stage-report"));
    assert!(text.contains("\"activeElements\""));
}
