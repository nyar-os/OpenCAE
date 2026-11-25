//! Surface-convection tests for steady-state heat conduction.
//!
//! Each member adds `h·P·L/6·[[2, 1], [1, 2]]` to the conduction matrix and
//! `h·P·L·Ta/2·[1, 1]` to the load vector. The tests compare a single two-node
//! element with the hand-assembled 2×2 system, refine a fin towards the
//! analytic insulated-tip `cosh` solution, check the energy balance and the
//! stable error codes, and verify that documents without convection keep their
//! previous byte-identical output.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{HeatConvection, Material, Model, Units, solve, solve_heat};

const E: f64 = 210.0e9;
const K: f64 = 50.0;
const AREA: f64 = 1.0e-2;
const LENGTH: f64 = 1.0;
const CONVECTION: f64 = 2.0;
const PERIMETER: f64 = 0.1;
const AMBIENT: f64 = 20.0;
const HOT: f64 = 100.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn model() -> Model {
    let mut model = Model::new(Units::Meter);
    model
        .set_material_data(Material {
            name: "steel".to_string(),
            youngs_modulus: E,
            area: AREA,
            density: 0.0,
            allowable_stress: None,
            alpha: None,
            conductivity: Some(K),
            shear_modulus: None,
            poissons_ratio: None,
            section: None,
        })
        .expect("material");
    model
}

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-convection-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

/// A single two-node member: `n1` fixed at `HOT`, `n2` free, both surfaces
/// exposed to the ambient temperature.
fn convective_bar() -> Model {
    let mut model = model();
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", LENGTH, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("element");
    model.set_element_convection("e1", Some(CONVECTION)).expect("convection");
    model.set_element_perimeter("e1", Some(PERIMETER)).expect("perimeter");
    model.set_ambient_temperature(Some(AMBIENT)).expect("ambient");
    model.set_node_temperature("n1", Some(HOT)).expect("fixed n1");
    model
}

fn hand_solution() -> (f64, f64, f64, f64) {
    let conductance = K * AREA / LENGTH;
    let surface = CONVECTION * PERIMETER * LENGTH;
    let a = conductance + surface / 3.0;
    let b = -conductance + surface / 6.0;
    let ambient_load = surface * AMBIENT / 2.0;
    let free = (ambient_load - b * HOT) / a;
    let flux = conductance * (HOT - free);
    let reaction = flux + surface / 6.0 * (2.0 * HOT + free) - ambient_load;
    let convective = surface * ((HOT + free) / 2.0 - AMBIENT);
    (free, flux, reaction, convective)
}

#[test]
fn two_node_element_matches_the_hand_assembled_convection_system() {
    let result = solve_heat(&convective_bar()).expect("heat solution");
    let (free, flux, reaction, convective) = hand_solution();

    let element = &result.element_flux[0];
    assert_close(element.temperature_to, free, 1e-12);
    assert_close(element.flux, flux, 1e-12);
    let convection = element.convection.clone().expect("convection is reported");
    assert_eq!(
        convection,
        HeatConvection { coefficient: CONVECTION, perimeter: PERIMETER, ambient_temperature: AMBIENT, heat: convective }
    );

    assert_eq!(result.reactions.len(), 1);
    assert_close(result.reactions[0].heat, reaction, 1e-12);
    assert_close(result.residual, 0.0, 1e-12);
    assert!(result.ok);
}

#[test]
fn energy_balance_is_reaction_equals_convective_loss() {
    let result = solve_heat(&convective_bar()).expect("heat solution");
    let (_, _, _, convective) = hand_solution();
    let reaction: f64 = result.reactions.iter().map(|reaction| reaction.heat).sum();
    let loss: f64 = result.element_flux.iter().map(|element| element.convection.clone().expect("convection").heat).sum();
    assert_close(reaction, convective, 1e-12);
    assert_close(loss, convective, 1e-12);
    // `Σfixed reaction + Σapplied nodal heat = Σconvective heat`; there is no
    // applied nodal heat in this model, so the reaction equals the loss and the
    // signed balance `reaction − loss` vanishes.
    assert!((reaction - loss).abs() <= 1e-12 * reaction.abs().max(1.0), "reaction {reaction} vs loss {loss}");
}

fn fin(element_count: usize) -> Model {
    let mut model = model();
    for index in 0..=element_count {
        let x = LENGTH * index as f64 / element_count as f64;
        model.add_node(format!("n{index}"), x, 0.0).expect("node");
    }
    for index in 0..element_count {
        let id = format!("e{index}");
        model.add_element(&id, format!("n{index}"), format!("n{}", index + 1), None).expect("element");
        model.set_element_convection(&id, Some(10.0)).expect("convection");
        model.set_element_perimeter(&id, Some(0.1)).expect("perimeter");
    }
    model.set_ambient_temperature(Some(AMBIENT)).expect("ambient");
    model.set_node_temperature("n0", Some(HOT)).expect("fixed n0");
    model
}

#[test]
fn fin_tip_converges_to_the_analytic_cosh_solution() {
    let m = (10.0 * 0.1 / (K * AREA)).sqrt();
    let analytic = AMBIENT + (HOT - AMBIENT) / (m * LENGTH).cosh();
    let tip = |count: usize| {
        let result = solve_heat(&fin(count)).expect("heat solution");
        result.temperatures.last().expect("tip node").temperature
    };

    let coarse = (tip(2) - analytic).abs();
    let medium = (tip(10) - analytic).abs();
    let fine = (tip(40) - analytic).abs();
    assert!(fine < medium, "refining 10→40 must reduce the error: {medium} then {fine}");
    assert!(medium < coarse, "refining 2→10 must reduce the error: {coarse} then {medium}");
    assert_close(tip(40), analytic, 2e-3);
}

#[test]
fn convection_inputs_use_stable_error_codes() {
    let mut missing_ambient = convective_bar();
    missing_ambient.ambient_temperature = None;
    assert_eq!(solve_heat(&missing_ambient).expect_err("missing ambient").code, "heat/missing-ambient");
    assert!(missing_ambient.validate().iter().any(|issue| issue.code == "heat/missing-ambient"));

    let mut missing_perimeter = convective_bar();
    missing_perimeter.elements[0].perimeter = None;
    assert_eq!(solve_heat(&missing_perimeter).expect_err("missing perimeter").code, "heat/missing-perimeter");
    assert!(missing_perimeter.validate().iter().any(|issue| issue.code == "heat/missing-perimeter"));

    let mut negative_convection = convective_bar();
    negative_convection.elements[0].convection = Some(-1.0);
    assert_eq!(solve_heat(&negative_convection).expect_err("negative convection").code, "heat/invalid-convection");
    assert!(negative_convection.validate().iter().any(|issue| issue.code == "heat/invalid-convection"));

    let mut zero_perimeter = convective_bar();
    zero_perimeter.elements[0].perimeter = Some(0.0);
    assert_eq!(solve_heat(&zero_perimeter).expect_err("zero perimeter").code, "heat/invalid-perimeter");
    assert!(zero_perimeter.validate().iter().any(|issue| issue.code == "heat/invalid-perimeter"));

    let mut bad_ambient = convective_bar();
    bad_ambient.ambient_temperature = Some(f64::NAN);
    assert_eq!(solve_heat(&bad_ambient).expect_err("non-finite ambient").code, "heat/invalid-ambient");
    assert!(bad_ambient.validate().iter().any(|issue| issue.code == "heat/invalid-ambient"));

    let mut valid = convective_bar();
    assert!(valid.set_element_convection("e1", Some(0.0)).is_err(), "zero convection is rejected");
    assert!(valid.set_element_perimeter("e1", Some(-0.1)).is_err(), "negative perimeter is rejected");
    assert!(valid.set_ambient_temperature(Some(f64::INFINITY)).is_err(), "non-finite ambient is rejected");
}

#[test]
fn documents_without_convection_keep_their_previous_output() {
    let mut baseline = model();
    baseline.add_node("n1", 0.0, 0.0).expect("n1");
    baseline.add_node("n2", LENGTH, 0.0).expect("n2");
    baseline.add_element("e1", "n1", "n2", None).expect("element");
    baseline.add_support("n1", true, true).expect("support n1");
    baseline.add_support("n2", false, true).expect("support n2");
    baseline.add_load("n2", Some(1000.0), None).expect("load n2");
    baseline.set_node_temperature("n1", Some(HOT)).expect("fixed n1");
    baseline.set_node_temperature("n2", Some(0.0)).expect("fixed n2");

    let text = baseline.to_json_pretty().expect("serialize");
    assert!(!text.contains("\"convection\""), "absent convection must not be serialized: {text}");
    assert!(!text.contains("\"perimeter\""), "absent perimeter must not be serialized: {text}");
    assert!(!text.contains("\"ambientTemperature\""), "absent ambient must not be serialized: {text}");

    let heat = solve_heat(&baseline.clone()).expect("heat solution");
    let heat_text = serde_json::to_string(&heat).expect("serialize heat");
    assert!(!heat_text.contains("\"convection\""), "conduction-only reports keep their shape: {heat_text}");

    let mut with_ambient = baseline.clone();
    with_ambient.set_ambient_temperature(Some(AMBIENT)).expect("ambient");
    assert_eq!(heat, solve_heat(&with_ambient).expect("unchanged heat solution"));

    let structural = solve(&baseline).expect("structural solution");
    let mut with_convection = baseline.clone();
    with_convection.set_element_convection("e1", Some(CONVECTION)).expect("convection");
    with_convection.set_element_perimeter("e1", Some(PERIMETER)).expect("perimeter");
    with_convection.set_ambient_temperature(Some(AMBIENT)).expect("ambient");
    let actual = solve(&with_convection).expect("structural solution with convection");
    assert_eq!(actual.nodes, structural.nodes);
    assert_eq!(actual.elements, structural.elements);
}

#[test]
fn convection_fields_survive_a_save_and_reopen() {
    let directory = temporary_dir("reopen");
    let path = directory.join("model.ocae");
    let model = convective_bar();
    model.save_atomic(&path).expect("save");

    let reopened = Model::load(&path).expect("load");
    assert_eq!(reopened.elements[0].convection, Some(CONVECTION));
    assert_eq!(reopened.elements[0].perimeter, Some(PERIMETER));
    assert_eq!(reopened.ambient_temperature, Some(AMBIENT));
    assert_eq!(solve_heat(&reopened).expect("heat solution"), solve_heat(&model).expect("heat solution"));
    fs::remove_file(&path).expect("remove document");
}
