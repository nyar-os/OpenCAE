//! Named load patterns mixed into load combinations.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{
    ElementKind, Load, LoadCombination, LoadPattern, Model, OpenCae, OpenCaeOptions, PatternFactor, Units, solve,
    solve_combinations,
};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;

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

/// Cantilever beam without any applied load case.
fn bare_cantilever() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(Some("steel".to_string()), E, A, 0.0);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed support");
    model
}

/// Builds a combination that mixes the given pattern names with factors.
fn combination(name: &str, patterns: &[(&str, f64)]) -> LoadCombination {
    LoadCombination {
        name: name.to_string(),
        loads: 1.0,
        gravity: 0.0,
        patterns: patterns
            .iter()
            .map(|(pattern, factor)| PatternFactor { pattern: (*pattern).to_string(), factor: *factor })
            .collect(),
    }
}

#[test]
fn mixed_patterns_equal_the_superposed_loads() {
    let mut model = bare_cantilever();
    model.add_pattern_load("live", "n2", None, Some(-1000.0), None, None).expect("live load");
    model.add_pattern_load("wind", "n2", Some(500.0), None, None, None).expect("wind load");
    model
        .add_combination_with_patterns(
            "mix",
            0.0,
            0.0,
            vec![
                PatternFactor { pattern: "live".to_string(), factor: 1.0 },
                PatternFactor { pattern: "wind".to_string(), factor: 2.0 },
            ],
        )
        .expect("combination");

    let mut reference = bare_cantilever();
    reference.add_load_full("n2", Some(1000.0), Some(-1000.0), None, None).expect("reference load");
    let expected = solve(&reference).expect("reference solve");

    let result = solve_combinations(&model, &model.combinations).expect("combination solve");
    assert_eq!(result.combinations.len(), 1);
    assert_eq!(
        result.combinations[0].patterns,
        vec![
            PatternFactor { pattern: "live".to_string(), factor: 1.0 },
            PatternFactor { pattern: "wind".to_string(), factor: 2.0 },
        ]
    );
    let solved = &result.combinations[0].solution;
    for (node, want) in solved.nodes.iter().zip(&expected.nodes) {
        assert_close(node.ux, want.ux, 1e-9);
        assert_close(node.uy, want.uy, 1e-9);
    }
    assert_close(solved.elements[0].axial_force, expected.elements[0].axial_force, 1e-9);
}

#[test]
fn pattern_loads_mix_with_the_applied_load_case() {
    let mut model = bare_cantilever();
    model.add_load("n2", None, Some(-250.0)).expect("applied load");
    model.add_pattern_load("live", "n2", None, Some(-1000.0), None, None).expect("live load");
    model
        .add_combination_with_patterns("all", 1.0, 0.0, vec![PatternFactor { pattern: "live".to_string(), factor: 0.5 }])
        .expect("combination");

    let mut reference = bare_cantilever();
    reference.add_load_full("n2", None, Some(-750.0), None, None).expect("reference load");
    let expected = solve(&reference).expect("reference solve");

    let result = solve_combinations(&model, &model.combinations).expect("combination solve");
    let solved = &result.combinations[0].solution;
    for (node, want) in solved.nodes.iter().zip(&expected.nodes) {
        assert_close(node.uy, want.uy, 1e-9);
    }
}

#[test]
fn session_round_trip_preserves_patterns_and_combinations() {
    let directory = temporary_dir("patterns-session");
    let path = directory.join("mix.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element("b1", "n1", "n2", Some(E), Some(A)).expect("element");
    session.add_support_full("n1", true, true, false, false).expect("support");
    session.add_support_full("n2", false, true, false, false).expect("vertical support");
    session.add_pattern_load("live", "n2", Some(1000.0), None, None, None).expect("live load");
    session.add_pattern_load("wind", "n2", Some(-400.0), None, None, None).expect("wind load");
    session
        .add_combination_with_patterns(
            "net",
            0.0,
            0.0,
            &[
                PatternFactor { pattern: "live".to_string(), factor: 1.0 },
                PatternFactor { pattern: "wind".to_string(), factor: 1.0 },
            ],
        )
        .expect("combination");

    let reopened = Model::load(&path).expect("reload");
    assert_eq!(
        reopened.patterns,
        vec![
            LoadPattern {
                name: "live".to_string(),
                loads: vec![Load { node: "n2".to_string(), fx: 1000.0, fy: 0.0, fz: 0.0, mz: 0.0 }],
            },
            LoadPattern {
                name: "wind".to_string(),
                loads: vec![Load { node: "n2".to_string(), fx: -400.0, fy: 0.0, fz: 0.0, mz: 0.0 }],
            },
        ]
    );
    assert_eq!(reopened.combinations[0].patterns.len(), 2);

    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open");
    let result = session.solve_combinations(None).expect("solve");
    assert_close(result.combinations[0].solution.nodes[1].ux, 600.0 * L / (E * A), 1e-9);
}

#[test]
fn remove_pattern_load_keeps_the_pattern() {
    let mut model = bare_cantilever();
    model.add_pattern_load("live", "n2", None, Some(-1000.0), None, None).expect("live load");
    assert!(model.remove_pattern_load("live", "n2"));
    assert!(!model.remove_pattern_load("live", "n2"));
    assert_eq!(model.patterns.len(), 1);
    assert!(model.patterns[0].loads.is_empty());
    assert!(model.remove_pattern("live"));
    assert!(!model.remove_pattern("live"));
    assert!(model.patterns.is_empty());
}

#[test]
fn pattern_edits_validate_like_plain_loads() {
    let mut model = bare_cantilever();
    let error = model.add_pattern_load("live", "ghost", Some(1.0), None, None, None).expect_err("unknown node");
    assert_eq!(error.code, "load/unknown-node");
    let error = model.add_pattern_load("live", "n2", None, None, Some(1.0), None).expect_err("2D fz");
    assert_eq!(error.code, "document/dimension-mismatch");
    let error = model.add_pattern_load(" ", "n2", Some(1.0), None, None, None).expect_err("empty name");
    assert_eq!(error.code, "pattern/invalid-name");
}

#[test]
fn validation_reports_pattern_errors() {
    let mut model = bare_cantilever();
    model.add_pattern_load("live", "n2", None, Some(-1000.0), None, None).expect("live load");
    model.patterns.push(LoadPattern { name: "live".to_string(), loads: Vec::new() });
    model.combinations.push(combination("ghost", &[("missing", 1.0), ("missing", 2.0)]));
    let codes: Vec<String> = model.validate().into_iter().map(|issue| issue.code).collect();
    assert!(codes.iter().any(|code| code == "pattern/duplicate-name"), "codes: {codes:?}");
    assert!(codes.iter().any(|code| code == "combination/duplicate-pattern"), "codes: {codes:?}");

    let mut model = bare_cantilever();
    model.combinations.push(combination("ghost", &[("missing", 1.0)]));
    let codes: Vec<String> = model.validate().into_iter().map(|issue| issue.code).collect();
    assert!(codes.iter().any(|code| code == "combination/unknown-pattern"), "codes: {codes:?}");
    let error = solve_combinations(&model, &model.combinations).expect_err("unknown pattern must stop the solve");
    assert_eq!(error.code, "combination/unknown-pattern");
}

#[test]
fn combination_rejects_invalid_pattern_factors() {
    let mut model = bare_cantilever();
    let error = model
        .add_combination_with_patterns("bad", 1.0, 0.0, vec![PatternFactor { pattern: "x".to_string(), factor: f64::NAN }])
        .expect_err("non-finite factor");
    assert_eq!(error.code, "combination/invalid-factor");
    let error = model
        .add_combination_with_patterns("bad", 1.0, 0.0, vec![PatternFactor { pattern: " ".to_string(), factor: 1.0 }])
        .expect_err("empty pattern name");
    assert_eq!(error.code, "combination/invalid-name");
    let error = model
        .add_combination_with_patterns(
            "bad",
            1.0,
            0.0,
            vec![
                PatternFactor { pattern: "x".to_string(), factor: 1.0 },
                PatternFactor { pattern: "x".to_string(), factor: 2.0 },
            ],
        )
        .expect_err("duplicate pattern");
    assert_eq!(error.code, "combination/duplicate-pattern");
}

#[test]
fn session_removes_patterns_with_stable_errors() {
    let directory = temporary_dir("patterns-remove");
    let path = directory.join("mix.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_pattern_load("live", "n2", Some(1000.0), None, None, None).expect("live load");
    let error = session.remove_pattern("ghost").expect_err("missing pattern");
    assert_eq!(error.code, "pattern/unknown-pattern");
    let error = session.remove_pattern_load("live", "n1").expect_err("missing entry");
    assert_eq!(error.code, "load/unknown-load");
    session.remove_pattern_load("live", "n2").expect("remove entry");
    session.remove_pattern("live").expect("remove pattern");
    let reopened = Model::load(&path).expect("reload");
    assert!(reopened.patterns.is_empty());
}
