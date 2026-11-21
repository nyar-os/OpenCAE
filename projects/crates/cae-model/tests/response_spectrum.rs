use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{Model, OpenCae, OpenCaeOptions, SpectrumPoint, Units, solve_response_spectrum};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;
const RHO: f64 = 7.85e-9;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

/// Axial bar with a single free translational degree of freedom at `n2`.
fn single_dof() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, RHO);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

#[test]
fn single_degree_of_freedom_matches_the_static_spectral_displacement() {
    let model = single_dof();
    let mass = RHO * A * L / 2.0;
    let stiffness = E * A / L;
    let acceleration = 2.0;
    let spectrum = [SpectrumPoint { period: 0.0, acceleration }, SpectrumPoint { period: 10.0, acceleration }];

    let result = solve_response_spectrum(&model, 1, 0, &spectrum).expect("response spectrum");
    assert_eq!(result.axis, "x");
    assert_eq!(result.modes.len(), 1);

    let expected = acceleration * mass / stiffness;
    assert_close(result.max_displacement.ux, expected, 1e-9);
    assert_close(result.nodes[1].ux, expected, 1e-9);
    assert_close(result.nodes[0].ux, 0.0, 1e-9);
    assert_close(result.nodes[1].uy, 0.0, 1e-12);

    let mode = &result.modes[0];
    assert_close(mode.spectral_displacement, acceleration * mass / stiffness, 1e-9);
    assert_close(mode.participation_factor, mass.sqrt(), 1e-9);
    assert_close(mode.scale, mass.sqrt() * acceleration * mass / stiffness, 1e-9);
}

#[test]
fn spectrum_values_are_interpolated_at_the_natural_period() {
    let model = single_dof();
    let stiffness = E * A / L;
    let mass = RHO * A * L / 2.0;
    let period = 2.0 * std::f64::consts::PI * (mass / stiffness).sqrt();
    let spectrum =
        [SpectrumPoint { period: 0.0, acceleration: 1.0 }, SpectrumPoint { period: 2.0 * period, acceleration: 3.0 }];

    let result = solve_response_spectrum(&model, 1, 0, &spectrum).expect("response spectrum");
    let mode = &result.modes[0];
    assert_close(mode.period, period, 1e-9);
    assert_close(mode.spectral_acceleration, 1.0 + (3.0 - 1.0) * (period / (2.0 * period)), 1e-9);
    let expected = mode.spectral_acceleration * mass / stiffness;
    assert_close(result.max_displacement.ux, expected, 1e-9);
}

#[test]
fn invalid_spectra_and_axes_are_rejected() {
    let model = single_dof();
    assert_eq!(solve_response_spectrum(&model, 1, 0, &[]).expect_err("empty spectrum").code, "analysis/invalid-spectrum");
    assert_eq!(
        solve_response_spectrum(&model, 1, 2, &[SpectrumPoint { period: 0.0, acceleration: 1.0 }]).expect_err("axis z").code,
        "analysis/invalid-axis"
    );
    assert_eq!(
        solve_response_spectrum(&model, 1, 0, &[SpectrumPoint { period: -1.0, acceleration: 1.0 }])
            .expect_err("negative period")
            .code,
        "analysis/invalid-spectrum"
    );
}

#[test]
fn session_writes_a_response_spectrum_report() {
    let directory = temporary_dir("spectrum");
    let path = directory.join("model.ocae");
    single_dof().save_atomic(&path).expect("save");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(&path).expect("open");
    let out = directory.join("spectrum.json");
    let result = session
        .response_spectrum(1, 0, &[SpectrumPoint { period: 0.0, acceleration: 1.0 }], Some(&out))
        .expect("response spectrum");
    assert_eq!(result.format, "open-cae-response-spectrum-report");
    let text = fs::read_to_string(out).expect("read report");
    assert!(text.contains("open-cae-response-spectrum-report"));
    assert!(text.contains("\"modes\""));
}
