use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{Model, OpenCae, OpenCaeOptions, Units, solve_frequency_response};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;
const RHO: f64 = 7.85e-9;
const LOAD: f64 = 1000.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-frequency-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

/// Axial bar with one free translational degree of freedom at `n2`.
fn single_dof() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, RHO);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model.add_load("n2", Some(LOAD), None).expect("load");
    model
}

fn natural_frequency() -> f64 {
    let stiffness = E * A / L;
    let mass = RHO * A * L / 2.0;
    (stiffness / mass).sqrt() / (2.0 * std::f64::consts::PI)
}

#[test]
fn resonance_amplitude_matches_the_single_degree_of_freedom_solution() {
    let model = single_dof();
    let frequency = natural_frequency();
    let damping = 0.02;
    let span = 0.1 * frequency;
    let result = solve_frequency_response(&model, 1, 0, damping, frequency - span, frequency + span, 3).expect("sweep");

    assert_eq!(result.axis, "x");
    assert_eq!(result.points, 3);
    assert_eq!(result.sweep.len(), 3);
    let static_displacement = LOAD / (E * A / L);
    let resonance = static_displacement / (2.0 * damping);

    let middle = &result.sweep[1];
    assert_close(middle.frequency_hz, frequency, 1e-12);
    assert_close(middle.max_amplitude, resonance, 1e-9);
    assert_eq!(middle.governing_node, "n2");

    assert_close(result.peak.frequency_hz, frequency, 1e-12);
    assert_close(result.peak.max_amplitude, resonance, 1e-9);
    assert_eq!(result.nodes[0].max_amplitude, 0.0);
    assert_close(result.nodes[1].max_amplitude, resonance, 1e-9);
    assert_close(result.nodes[1].peak_frequency_hz, frequency, 1e-12);
}

#[test]
fn zero_frequency_limit_reproduces_the_static_displacement() {
    let model = single_dof();
    let frequency = natural_frequency();
    let result = solve_frequency_response(&model, 1, 0, 0.05, 0.0, 10.0 * frequency, 2).expect("sweep");
    let static_displacement = LOAD / (E * A / L);
    assert_close(result.sweep[0].frequency_hz, 0.0, 1e-12);
    assert_close(result.sweep[0].max_amplitude, static_displacement, 1e-9);
    // The low-frequency sample dominates the second sample at 10·f_n.
    assert_close(result.nodes[1].peak_frequency_hz, 0.0, 1e-12);
    assert_close(result.nodes[1].max_amplitude, static_displacement, 1e-9);
}

#[test]
fn invalid_sweeps_and_models_are_rejected() {
    let model = single_dof();
    assert_eq!(
        solve_frequency_response(&model, 0, 0, 0.05, 0.0, 10.0, 11).expect_err("zero modes").code,
        "analysis/invalid-modes"
    );
    assert_eq!(
        solve_frequency_response(&model, 1, 0, 0.05, 0.0, 10.0, 1).expect_err("too few points").code,
        "analysis/invalid-points"
    );
    assert_eq!(
        solve_frequency_response(&model, 1, 0, 0.05, 10.0, 10.0, 11).expect_err("empty range").code,
        "analysis/invalid-range"
    );
    assert_eq!(
        solve_frequency_response(&model, 1, 0, 0.0, 0.0, 10.0, 11).expect_err("zero damping").code,
        "analysis/invalid-damping"
    );
    assert_eq!(solve_frequency_response(&model, 1, 2, 0.05, 0.0, 10.0, 11).expect_err("axis z").code, "analysis/invalid-axis");
    let mut massless = single_dof();
    massless.set_material_full(None, E, A, 0.0);
    assert_eq!(
        solve_frequency_response(&massless, 1, 0, 0.05, 0.0, 10.0, 11).expect_err("zero density").code,
        "material/invalid-density"
    );
}

#[test]
fn session_writes_a_frequency_response_report() {
    let directory = temporary_dir("session");
    let path = directory.join("model.ocae");
    single_dof().save_atomic(&path).expect("save");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open");
    let out = directory.join("frequency.json");
    let result = session.frequency_response(1, 0, 0.05, 0.0, 2000.0, 21, Some(&out)).expect("frequency response");
    assert_eq!(result.format, "open-cae-frequency-response-report");
    let text = fs::read_to_string(out).expect("read report");
    assert!(text.contains("open-cae-frequency-response-report"));
    assert!(text.contains("\"dampingRatio\""));
    assert!(text.contains("\"peakFrequencyHz\""));
}
