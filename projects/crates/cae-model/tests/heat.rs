//! Steady-state heat-conduction tests.
//!
//! Each truss member is a 1D conductor with `k·A/L·[[1, -1], [-1, 1]]`. The
//! analytic cases below exercise a single bar, a series of bars, a fixed
//! boundary reaction and an adiabatic node, and compare the reported flux with
//! `q = k·A·(T₁ − T₂)/L`.

use cae_model::{HEAT_REPORT_FORMAT, Material, Model, Units, solve, solve_heat};

const E: f64 = 210.0e9;
const K: f64 = 50.0;
const AREA: f64 = 1.0e-4;

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

fn bar(length: f64, temperature_from: Option<f64>, temperature_to: Option<f64>) -> Model {
    let mut model = model();
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", length, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("element");
    model.set_node_temperature("n1", temperature_from).expect("temperature n1");
    model.set_node_temperature("n2", temperature_to).expect("temperature n2");
    model
}

#[test]
fn single_bar_flux_matches_analytic_solution() {
    let result = solve_heat(&bar(1.0, Some(100.0), Some(0.0))).expect("heat solution");
    assert_eq!(result.format, HEAT_REPORT_FORMAT);
    assert_eq!(result.analysis, "heat");
    assert_eq!(result.units, "m");

    let expected = K * AREA * (100.0 - 0.0) / 1.0;
    assert_close(result.element_flux[0].flux, expected, 1e-12);
    assert_close(result.temperatures[0].temperature, 100.0, 1e-12);
    assert_close(result.temperatures[1].temperature, 0.0, 1e-12);
    assert!(result.temperatures.iter().all(|node| node.fixed));
    assert_close(result.reactions[0].heat, expected, 1e-12);
    assert_close(result.reactions[1].heat, -expected, 1e-12);
    assert_close(result.residual, 0.0, 1e-12);
    assert!(result.ok);
}

#[test]
fn series_bars_share_flux_and_reproduce_interface_temperature() {
    let mut model = model();
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", 1.0, 0.0).expect("n2");
    model.add_node("n3", 2.0, 0.0).expect("n3");
    model.add_element("e1", "n1", "n2", None).expect("e1");
    model.add_element("e2", "n2", "n3", None).expect("e2");
    model.set_node_temperature("n1", Some(100.0)).expect("fixed n1");
    model.set_node_temperature("n3", Some(0.0)).expect("fixed n3");

    let result = solve_heat(&model).expect("heat solution");
    let conductance = K * AREA / 1.0;
    let interface = (conductance * 100.0 + conductance * 0.0) / (2.0 * conductance);
    assert_close(result.temperatures[1].temperature, interface, 1e-12);
    assert!(!result.temperatures[1].fixed);

    let flux = conductance * (100.0 - interface);
    assert_close(result.element_flux[0].flux, flux, 1e-12);
    assert_close(result.element_flux[1].flux, flux, 1e-12);
    assert_close(result.reactions[0].heat, flux, 1e-12);
    assert_close(result.reactions[1].heat, -flux, 1e-12);
    assert!(result.ok);
}

#[test]
fn fixed_temperature_reaction_balances_the_applied_heat() {
    let mut model = bar(1.0, Some(100.0), None);
    model.set_node_heat("n2", 10.0).expect("heat load");

    let result = solve_heat(&model).expect("heat solution");
    let conductance = K * AREA / 1.0;
    assert_close(result.temperatures[1].temperature, 100.0 + 10.0 / conductance, 1e-12);
    assert_close(result.element_flux[0].flux, -10.0, 1e-12);
    assert_eq!(result.reactions.len(), 1);
    assert_close(result.reactions[0].heat, -10.0, 1e-12);
    let reaction_sum: f64 = result.reactions.iter().map(|reaction| reaction.heat).sum();
    assert_close(reaction_sum + 10.0, 0.0, 1e-12);
    assert!(result.ok);
}

#[test]
fn adiabatic_node_reaches_the_fixed_temperature() {
    let result = solve_heat(&bar(1.0, Some(100.0), None)).expect("heat solution");
    assert_close(result.temperatures[1].temperature, 100.0, 1e-12);
    assert_close(result.element_flux[0].flux, 0.0, 1e-12);
    assert_close(result.reactions[0].heat, 0.0, 1e-12);
    assert!(result.ok);
}

#[test]
fn heat_analysis_reports_stable_error_codes() {
    let mut no_fixed = bar(1.0, None, None);
    assert_eq!(solve_heat(&no_fixed).expect_err("no fixed temperature").code, "heat/no-fixed-temperature");

    let missing = bar(1.0, Some(100.0), Some(0.0));
    let mut no_conductivity = missing.clone();
    no_conductivity.material.conductivity = None;
    assert_eq!(solve_heat(&no_conductivity).expect_err("missing conductivity").code, "material/invalid-conductivity");

    let mut zero_conductivity = missing.clone();
    zero_conductivity.material.conductivity = Some(0.0);
    assert_eq!(solve_heat(&zero_conductivity).expect_err("zero conductivity").code, "material/invalid-conductivity");

    let mut invalid_heat = bar(1.0, Some(100.0), None);
    invalid_heat.nodes[1].heat = f64::NAN;
    assert_eq!(solve_heat(&invalid_heat).expect_err("non-finite heat").code, "load/invalid-heat");

    let mut no_nodes = model();
    assert_eq!(solve_heat(&no_nodes).expect_err("no nodes").code, "model/no-nodes");
    no_nodes.add_node("n1", 0.0, 0.0).expect("n1");
    no_nodes.add_node("n2", 1.0, 0.0).expect("n2");
    assert_eq!(solve_heat(&no_nodes).expect_err("no elements").code, "model/no-elements");
    no_fixed.nodes.clear();
    assert_eq!(solve_heat(&no_fixed).expect_err("no nodes after clear").code, "model/no-nodes");
}

#[test]
fn structural_solution_is_unchanged_by_heat_fields() {
    let mut baseline = model();
    baseline.add_node("n1", 0.0, 0.0).expect("n1");
    baseline.add_node("n2", 1.0, 0.0).expect("n2");
    baseline.add_element("e1", "n1", "n2", None).expect("element");
    baseline.add_support("n1", true, true).expect("support n1");
    baseline.add_support("n2", false, true).expect("support n2");
    baseline.add_load("n2", Some(1000.0), None).expect("load n2");
    let expected = solve(&baseline).expect("structural solution");

    let mut with_heat = baseline.clone();
    with_heat.set_node_temperature("n1", Some(100.0)).expect("temperature");
    with_heat.set_node_heat("n2", 5.0).expect("heat");
    let actual = solve(&with_heat).expect("structural solution with heat fields");

    assert_eq!(actual.nodes, expected.nodes);
    assert_eq!(actual.elements, expected.elements);

    let text = baseline.to_json_pretty().expect("serialize");
    assert!(!text.contains("\"heat\""), "zero heat fields must not be serialized: {text}");
    assert!(!text.contains("\"temperature\""), "absent temperatures must not be serialized: {text}");
}

#[test]
fn heat_report_is_deterministic_and_contains_no_timestamp() {
    let model = bar(1.0, Some(100.0), Some(0.0));
    let first = solve_heat(&model).expect("first");
    let second = solve_heat(&model).expect("second");
    assert_eq!(first, second);
    let first_text = serde_json::to_string_pretty(&first).expect("serialize first");
    let second_text = serde_json::to_string_pretty(&second).expect("serialize second");
    assert_eq!(first_text, second_text);
    assert!(!first_text.contains("timestamp"));
    assert!(!first_text.contains("generated"));
}
