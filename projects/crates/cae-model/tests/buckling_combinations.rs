//! Tests for linear buckling under several load combinations.
//!
//! Each combination scales the applied loads and is solved as its own buckling
//! case, so the first factor is inversely proportional to the compression.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Model, OpenCae, OpenCaeOptions, Units, solve_buckling_combinations};

const E: f64 = 210_000.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-buckling-combinations-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() / scale <= relative, "expected {expected}, got {actual} (relative tolerance {relative})");
}

/// Pinned-pinned beam column with a compressive axial force of `load`.
fn pinned_pinned_column(elements: usize, load: f64) -> Model {
    let mut model = Model::new(Units::Millimeter);
    for index in 0..=elements {
        let x = L * index as f64 / elements as f64;
        model.add_node(format!("n{}", index + 1), x, 0.0).expect("node");
    }
    for index in 0..elements {
        model
            .add_element_kind(
                format!("e{}", index + 1),
                format!("n{}", index + 1),
                format!("n{}", index + 2),
                None,
                ElementKind::Beam,
                Some(I),
                None,
            )
            .expect("beam element");
    }
    model.add_support("n1", true, true).expect("pinned support");
    model.add_support(format!("n{}", elements + 1), false, true).expect("lateral support");
    model.add_load(format!("n{}", elements + 1), Some(load), None).expect("axial load");
    model
}

#[test]
fn buckling_factors_scale_inversely_with_the_combination_load() {
    let mut model = pinned_pinned_column(20, -1000.0);
    model.add_combination("P", 1.0, 0.0).expect("combination P");
    model.add_combination("2P", 2.0, 0.0).expect("combination 2P");

    let result = solve_buckling_combinations(&model, &model.combinations, 1).expect("buckling combinations");
    assert_eq!(result.format, "open-cae-buckling-combination-report");
    assert_eq!(result.combinations.len(), 2);

    let euler = std::f64::consts::PI.powi(2) * E * I / (L * L);
    let first = &result.combinations[0];
    let second = &result.combinations[1];
    assert_eq!(first.name, "P");
    assert_eq!(second.name, "2P");
    assert_close(first.solution.modes[0].factor, euler / 1000.0, 1e-3);
    assert_close(second.solution.modes[0].factor, euler / 2000.0, 1e-3);
    assert_close(second.solution.modes[0].factor, first.solution.modes[0].factor / 2.0, 1e-3);

    let envelope = result.envelope.expect("governing combination");
    assert_eq!(envelope.combination, "2P");
    assert_close(envelope.factor, second.solution.modes[0].factor, 1e-12);
}

#[test]
fn zero_load_combination_has_no_buckling_mode_and_does_not_win_the_envelope() {
    let mut model = pinned_pinned_column(10, -1000.0);
    model.add_combination("zero", 0.0, 0.0).expect("zero combination");
    model.add_combination("P", 1.0, 0.0).expect("combination P");

    let result = solve_buckling_combinations(&model, &model.combinations, 2).expect("buckling combinations");
    let zero = &result.combinations[0];
    assert!(zero.solution.modes.is_empty(), "a zero load produces no positive factor");
    assert!(!result.combinations[1].solution.modes.is_empty());
    assert_eq!(result.envelope.expect("governing combination").combination, "P");
}

#[test]
fn buckling_combinations_reject_empty_input_and_zero_modes() {
    let model = pinned_pinned_column(5, -1000.0);
    assert_eq!(solve_buckling_combinations(&model, &[], 1).expect_err("no combinations").code, "combination/none");
    let mut with_combination = model.clone();
    with_combination.add_combination("P", 1.0, 0.0).expect("combination");
    assert_eq!(
        solve_buckling_combinations(&with_combination, &with_combination.combinations, 0).expect_err("zero modes").code,
        "analysis/invalid-modes"
    );
}

#[test]
fn session_writes_the_buckling_combination_report() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    let mut model = pinned_pinned_column(10, -1000.0);
    model.add_combination("P", 1.0, 0.0).expect("combination P");
    model.add_combination("2P", 2.0, 0.0).expect("combination 2P");
    model.save_atomic(&path).expect("save model");

    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open");
    let out = directory.join("buckling-combinations.json");
    let result = session.solve_buckling_combinations(2, Some(&out)).expect("solve");
    assert_eq!(result.combinations.len(), 2);
    assert!(result.envelope.is_some());

    let written = fs::read_to_string(&out).expect("report");
    assert!(written.contains("open-cae-buckling-combination-report"), "report: {written}");
    assert!(written.contains("\"envelope\""), "report: {written}");
}
