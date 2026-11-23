use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, LoadCombination, Material, Model, OpenCae, OpenCaeOptions, Units, solve, solve_combinations};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;
const RHO: f64 = 7.85e-9;
const GRAVITY: f64 = 9810.0;
const TIP_LOAD: f64 = -1000.0;
const ALPHA: f64 = 12.0e-6;
const DT: f64 = 50.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

/// Cantilever beam carrying both a tip load and self-weight.
fn cantilever_load_case() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(Some("steel".to_string()), E, A, RHO);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed support");
    model.add_load("n2", None, Some(TIP_LOAD)).expect("tip load");
    model.set_gravity(0.0, -GRAVITY, 0.0).expect("gravity");
    model
}

fn combinations() -> Vec<LoadCombination> {
    vec![
        LoadCombination { name: "D".to_string(), loads: 0.0, gravity: 1.0, thermal: 1.0, patterns: Vec::new() },
        LoadCombination { name: "L".to_string(), loads: 1.0, gravity: 0.0, thermal: 1.0, patterns: Vec::new() },
        LoadCombination { name: "D+L".to_string(), loads: 1.0, gravity: 1.0, thermal: 1.0, patterns: Vec::new() },
    ]
}

fn node_uy(solution: &cae_model::Solution, id: &str) -> f64 {
    solution.nodes.iter().find(|node| node.id == id).expect("node result").uy
}

#[test]
fn combination_results_superpose_linearly() {
    let model = cantilever_load_case();
    let result = solve_combinations(&model, &combinations()).expect("combinations");
    assert_eq!(result.combinations.len(), 3);

    let dead = &result.combinations[0].solution;
    let live = &result.combinations[1].solution;
    let total = &result.combinations[2].solution;

    assert_close(node_uy(total, "n2"), node_uy(dead, "n2") + node_uy(live, "n2"), 1e-12);
    let dead_moment = dead.reactions.iter().find(|reaction| reaction.node == "n1").expect("reaction").mz;
    let live_moment = live.reactions.iter().find(|reaction| reaction.node == "n1").expect("reaction").mz;
    let total_moment = total.reactions.iter().find(|reaction| reaction.node == "n1").expect("reaction").mz;
    assert_close(total_moment, dead_moment + live_moment, 1e-12);
}

#[test]
fn combination_factors_scale_self_weight_and_tip_load_independently() {
    let model = cantilever_load_case();
    let result = solve_combinations(&model, &combinations()).expect("combinations");

    let weight_per_length = RHO * A * GRAVITY;
    let dead_deflection = -weight_per_length * L.powi(4) / (8.0 * E * I);
    let live_deflection = TIP_LOAD * L.powi(3) / (3.0 * E * I);
    assert_close(node_uy(&result.combinations[0].solution, "n2"), dead_deflection, 1e-9);
    assert_close(node_uy(&result.combinations[1].solution, "n2"), live_deflection, 1e-9);
    assert_close(node_uy(&result.combinations[2].solution, "n2"), dead_deflection + live_deflection, 1e-9);
}

#[test]
fn combination_envelope_selects_the_governing_combination() {
    let model = cantilever_load_case();
    let result = solve_combinations(&model, &combinations()).expect("combinations");

    assert_eq!(result.envelope.max_displacement.combination, "D+L");
    assert_eq!(result.envelope.max_displacement.entity, "n2");
    assert_close(result.envelope.max_displacement.value, node_uy(&result.combinations[2].solution, "n2").abs(), 1e-12);

    assert_eq!(result.envelope.nodes.len(), 2);
    let tip = result.envelope.nodes.iter().find(|node| node.node == "n2").expect("tip envelope");
    assert_eq!(tip.combination, "D+L");
    assert_close(tip.max_abs_displacement, node_uy(&result.combinations[2].solution, "n2").abs(), 1e-12);

    assert_eq!(result.envelope.elements.len(), 1);
    let element = &result.envelope.elements[0];
    let min_axial: f64 =
        result.combinations.iter().map(|case| case.solution.elements[0].axial_force).fold(f64::INFINITY, f64::min);
    let max_axial: f64 =
        result.combinations.iter().map(|case| case.solution.elements[0].axial_force).fold(f64::NEG_INFINITY, f64::max);
    assert_close(element.min_axial_force, min_axial, 1e-12);
    assert_close(element.max_axial_force, max_axial, 1e-12);
}

#[test]
fn combination_solver_rejects_an_empty_list() {
    let model = cantilever_load_case();
    let error = solve_combinations(&model, &[]).expect_err("empty combinations must be rejected");
    assert_eq!(error.code, "combination/none");
}

#[test]
fn combination_solver_rejects_invalid_factors() {
    let model = cantilever_load_case();
    let error = solve_combinations(
        &model,
        &[LoadCombination { name: "bad".to_string(), loads: f64::NAN, gravity: 1.0, thermal: 1.0, patterns: Vec::new() }],
    )
    .expect_err("non-finite factors must be rejected");
    assert_eq!(error.code, "combination/invalid-factor");
}

#[test]
fn model_validates_combination_names_and_factors() {
    let mut model = cantilever_load_case();
    model.combinations.push(LoadCombination {
        name: "".to_string(),
        loads: 1.0,
        gravity: 0.0,
        thermal: 1.0,
        patterns: Vec::new(),
    });
    model.combinations.push(LoadCombination {
        name: "D".to_string(),
        loads: 1.0,
        gravity: 1.0,
        thermal: 1.0,
        patterns: Vec::new(),
    });
    model.combinations.push(LoadCombination {
        name: "D".to_string(),
        loads: 1.0,
        gravity: 1.0,
        thermal: 1.0,
        patterns: Vec::new(),
    });
    let codes: Vec<String> = model.validate().into_iter().map(|issue| issue.code).collect();
    assert!(codes.iter().any(|code| code == "combination/invalid-name"));
    assert!(codes.iter().any(|code| code == "combination/duplicate-name"));

    let mut model = cantilever_load_case();
    let error = model.add_combination("  ", 1.0, 1.0).expect_err("empty name");
    assert_eq!(error.code, "combination/invalid-name");
    let error = model.add_combination("x", f64::INFINITY, 1.0).expect_err("infinite factor");
    assert_eq!(error.code, "combination/invalid-factor");
    let error = model.add_combination_full("x", 1.0, 1.0, f64::NAN, Vec::new()).expect_err("non-finite thermal factor");
    assert_eq!(error.code, "combination/invalid-factor");
}

/// Bar restrained at both ends and heated uniformly.
fn restrained_thermal_bar() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model
        .set_material_data(Material {
            name: "steel".to_string(),
            youngs_modulus: E,
            area: A,
            density: 0.0,
            allowable_stress: None,
            alpha: Some(ALPHA),
            conductivity: None,
            section: None,
        })
        .expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("fixed n1");
    model.add_support("n2", true, true).expect("fixed n2");
    model.set_element_temperature("e1", Some(DT)).expect("temperature");
    model
}

#[test]
fn combination_thermal_factor_scales_member_temperature() {
    let model = restrained_thermal_bar();
    let thermal_combination = |thermal: f64| LoadCombination {
        name: format!("thermal {thermal}"),
        loads: 0.0,
        gravity: 0.0,
        thermal,
        patterns: Vec::new(),
    };

    let expected = E * A * ALPHA * DT;
    let full = solve_combinations(&model, &[thermal_combination(1.0)]).expect("full thermal");
    let half = solve_combinations(&model, &[thermal_combination(0.5)]).expect("half thermal");
    let none = solve_combinations(&model, &[thermal_combination(0.0)]).expect("no thermal");

    assert_close(full.combinations[0].solution.elements[0].axial_force, -expected, 1e-12);
    assert_close(half.combinations[0].solution.elements[0].axial_force, -0.5 * expected, 1e-12);
    assert_close(none.combinations[0].solution.elements[0].axial_force, 0.0, 1e-12);
    assert_close(half.combinations[0].thermal, 0.5, 1e-12);
}

#[test]
fn thermal_factor_round_trips_and_defaults_to_one() {
    let directory = temporary_dir("thermal-combination");
    let path = directory.join("thermal.ocae");
    let mut model = restrained_thermal_bar();
    model.add_combination_full("half", 0.0, 0.0, 0.5, Vec::new()).expect("combination");
    model.save_atomic(&path).expect("save");

    let loaded = Model::load(&path).expect("load");
    assert_eq!(loaded.combinations, model.combinations);

    // A document written before the factor existed keeps thermal actions whole.
    let text = fs::read_to_string(&path).expect("document");
    let mut legacy: serde_json::Value = serde_json::from_str(&text).expect("json");
    legacy["combinations"][0].as_object_mut().expect("combination object").remove("thermal").expect("thermal field");
    fs::write(&path, serde_json::to_string_pretty(&legacy).expect("serialize legacy")).expect("write legacy document");
    let loaded = Model::load(&path).expect("load legacy");
    assert_close(loaded.combinations[0].thermal, 1.0, 1e-12);
}

#[test]
fn combinations_round_trip_and_solve_through_the_session() {
    let directory = temporary_dir("combinations");
    let path = directory.join("model.ocae");
    let mut model = cantilever_load_case();
    model.add_combination("D+L", 1.0, 1.0).expect("combination");
    model.save_atomic(&path).expect("save");

    let loaded = Model::load(&path).expect("load");
    assert_eq!(loaded.combinations, model.combinations);

    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open");
    session.add_combination("D", 0.0, 1.0).expect("add combination");
    session.add_combination("L", 1.0, 0.0).expect("add combination");
    let out = directory.join("combinations.json");
    let result = session.solve_combinations(Some(&out)).expect("solve combinations");
    assert_eq!(result.combinations.len(), 3);
    assert_eq!(result.envelope.nodes.len(), 2);
    let text = fs::read_to_string(&out).expect("report");
    assert!(text.contains("\"envelope\""));
}

#[test]
fn live_only_combination_solves_without_density() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("lateral guide");
    model.add_load("n2", Some(1000.0), None).expect("load");
    let result = solve_combinations(
        &model,
        &[LoadCombination { name: "L".to_string(), loads: 1.0, gravity: 0.0, thermal: 1.0, patterns: Vec::new() }],
    )
    .expect("live combination");
    let expected = 1000.0 * L / (E * A);
    assert_close(result.combinations[0].solution.nodes[1].ux, expected, 1e-9);
}

#[test]
fn single_combination_matches_the_static_solution() {
    let model = cantilever_load_case();
    let static_solution = solve(&model).expect("static");
    let result = solve_combinations(
        &model,
        &[LoadCombination { name: "full".to_string(), loads: 1.0, gravity: 1.0, thermal: 1.0, patterns: Vec::new() }],
    )
    .expect("combination");
    assert_eq!(result.combinations[0].solution, static_solution);
}
