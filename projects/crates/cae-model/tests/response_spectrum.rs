use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{Model, OpenCae, OpenCaeOptions, SpectrumCombination, SpectrumPoint, Units, solve_response_spectrum};

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

/// One free node joined to two pinned nodes by a 45° bar and a horizontal bar,
/// both of length `L`. The free stiffness is `k·[[1.5, 0.5], [0.5, 0.5]]` with
/// `k = E·A/L` and the lumped mass at the free node is `ρ·A·L`, so both modes
/// share the free node and their frequencies and vectors are known in closed
/// form.
fn coupled_two_mode() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, RHO);
    let offset = L / std::f64::consts::SQRT_2;
    model.add_node("q1", -offset, -offset).expect("q1");
    model.add_node("q2", -L, 0.0).expect("q2");
    model.add_node("p", 0.0, 0.0).expect("p");
    model.add_element("e1", "q1", "p", None).expect("diagonal bar");
    model.add_element("e2", "q2", "p", None).expect("horizontal bar");
    model.add_support("q1", true, true).expect("pin q1");
    model.add_support("q2", true, true).expect("pin q2");
    model
}

/// Closed-form modes of [`coupled_two_mode`]: `(ω, v_x, v_y)` for unit vectors.
fn coupled_modes() -> [(f64, f64, f64); 2] {
    let omega = (E / (RHO * L * L)).sqrt();
    let lower = 1.0 - 1.0 / std::f64::consts::SQRT_2;
    let upper = 1.0 + 1.0 / std::f64::consts::SQRT_2;
    [
        (lower.sqrt() * omega, 0.382_683_432_365_089_8, -0.923_879_532_511_286_7),
        (upper.sqrt() * omega, 0.923_879_532_511_286_7, 0.382_683_432_365_089_8),
    ]
}

/// Der Kiureghian coefficient as documented by `solve_response_spectrum`.
fn correlation(omega_i: f64, omega_j: f64, damping: f64) -> f64 {
    let r = omega_j / omega_i;
    let numerator = 8.0 * damping * damping * (1.0 + r) * r.powf(1.5);
    let denominator = (1.0 + r * r).powi(2) + 4.0 * damping * damping * r * (1.0 + r).powi(2);
    numerator / denominator
}

#[test]
fn single_degree_of_freedom_matches_the_static_spectral_displacement() {
    let model = single_dof();
    let mass = RHO * A * L / 2.0;
    let stiffness = E * A / L;
    let acceleration = 2.0;
    let spectrum = [SpectrumPoint { period: 0.0, acceleration }, SpectrumPoint { period: 10.0, acceleration }];

    let result = solve_response_spectrum(&model, 1, 0, &spectrum, SpectrumCombination::Srss, None).expect("response spectrum");
    assert_eq!(result.axis, "x");
    assert_eq!(result.combination, "srss");
    assert_eq!(result.damping, None);
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

    let result = solve_response_spectrum(&model, 1, 0, &spectrum, SpectrumCombination::Srss, None).expect("response spectrum");
    let mode = &result.modes[0];
    assert_close(mode.period, period, 1e-9);
    assert_close(mode.spectral_acceleration, 1.0 + (3.0 - 1.0) * (period / (2.0 * period)), 1e-9);
    let expected = mode.spectral_acceleration * mass / stiffness;
    assert_close(result.max_displacement.ux, expected, 1e-9);
}

#[test]
fn single_mode_cqc_equals_srss() {
    let model = single_dof();
    let spectrum = [SpectrumPoint { period: 0.0, acceleration: 2.0 }];

    let srss = solve_response_spectrum(&model, 1, 0, &spectrum, SpectrumCombination::Srss, None).expect("srss spectrum");
    let cqc = solve_response_spectrum(&model, 1, 0, &spectrum, SpectrumCombination::Cqc, Some(0.05)).expect("cqc spectrum");

    assert_eq!(cqc.combination, "cqc");
    assert_eq!(cqc.damping, Some(0.05));
    assert_eq!(cqc.nodes[1].ux, srss.nodes[1].ux);
    assert_eq!(cqc.nodes[1].magnitude, srss.nodes[1].magnitude);
    assert_eq!(cqc.max_displacement.magnitude, srss.max_displacement.magnitude);
}

#[test]
fn two_mode_cqc_matches_the_closed_form_combination() {
    let model = coupled_two_mode();
    let damping = 0.05;
    let acceleration = 2.0;
    let spectrum = [SpectrumPoint { period: 0.0, acceleration }, SpectrumPoint { period: 10.0, acceleration }];

    let result =
        solve_response_spectrum(&model, 2, 0, &spectrum, SpectrumCombination::Cqc, Some(damping)).expect("cqc spectrum");
    assert_eq!(result.combination, "cqc");
    assert_eq!(result.damping, Some(damping));
    assert_eq!(result.modes.len(), 2);

    let modes = coupled_modes();
    for (reported, (omega, _, _)) in result.modes.iter().zip(modes.iter()) {
        assert_close(reported.frequency_hz, omega / (2.0 * std::f64::consts::PI), 1e-9);
    }

    let displacements: Vec<f64> = modes.iter().map(|(omega, _, _)| acceleration / (omega * omega)).collect();
    let rho = correlation(modes[0].0, modes[1].0, damping);
    let contributions: Vec<(f64, f64)> =
        modes.iter().zip(&displacements).map(|((_, vx, vy), sd)| (vx * vx * sd, vx * vy * sd)).collect();
    let (x1, y1) = contributions[0];
    let (x2, y2) = contributions[1];
    let expected_x = (x1 * x1 + x2 * x2 + 2.0 * rho * x1 * x2).sqrt();
    let expected_y = (y1 * y1 + y2 * y2 + 2.0 * rho * y1 * y2).sqrt();

    let free = &result.nodes[2];
    assert_close(free.ux, expected_x, 1e-8);
    assert_close(free.uy, expected_y, 1e-8);
    assert_close(free.magnitude, (expected_x * expected_x + expected_y * expected_y).sqrt(), 1e-8);
    assert_close(result.max_displacement.magnitude, free.magnitude, 1e-12);
}

#[test]
fn small_damping_makes_cqc_approach_srss() {
    let model = coupled_two_mode();
    let spectrum = [SpectrumPoint { period: 0.0, acceleration: 2.0 }];

    let srss = solve_response_spectrum(&model, 2, 0, &spectrum, SpectrumCombination::Srss, None).expect("srss spectrum");
    let cqc = solve_response_spectrum(&model, 2, 0, &spectrum, SpectrumCombination::Cqc, Some(1e-9)).expect("cqc spectrum");

    for (combined, reference) in cqc.nodes.iter().zip(srss.nodes.iter()) {
        assert_close(combined.ux, reference.ux, 1e-6);
        assert_close(combined.uy, reference.uy, 1e-6);
        assert_close(combined.magnitude, reference.magnitude, 1e-6);
    }
    assert_close(cqc.max_displacement.magnitude, srss.max_displacement.magnitude, 1e-6);
}

#[test]
fn invalid_combination_and_damping_are_rejected() {
    let model = single_dof();
    let spectrum = [SpectrumPoint { period: 0.0, acceleration: 1.0 }];

    assert_eq!(SpectrumCombination::parse("srss").expect("srss"), SpectrumCombination::Srss);
    assert_eq!(SpectrumCombination::parse("cqc").expect("cqc"), SpectrumCombination::Cqc);
    assert_eq!(SpectrumCombination::parse("square").expect_err("unknown combination").code, "spectrum/invalid-combination");

    assert_eq!(
        solve_response_spectrum(&model, 1, 0, &spectrum, SpectrumCombination::Cqc, None).expect_err("missing damping").code,
        "spectrum/invalid-damping"
    );
    for damping in [0.0, -0.05, 1.5, f64::NAN, f64::INFINITY] {
        assert_eq!(
            solve_response_spectrum(&model, 1, 0, &spectrum, SpectrumCombination::Cqc, Some(damping))
                .expect_err("invalid damping")
                .code,
            "spectrum/invalid-damping"
        );
    }
    // SRSS ignores a supplied damping ratio and never reports it.
    let srss =
        solve_response_spectrum(&model, 1, 0, &spectrum, SpectrumCombination::Srss, Some(0.05)).expect("srss with damping");
    assert_eq!(srss.damping, None);

    // Existing spectrum errors keep their codes even when the combination is
    // CQC and the damping ratio is missing.
    assert_eq!(
        solve_response_spectrum(&model, 1, 0, &[], SpectrumCombination::Cqc, None).expect_err("empty spectrum").code,
        "analysis/invalid-spectrum"
    );
}

#[test]
fn invalid_spectra_and_axes_are_rejected() {
    let model = single_dof();
    assert_eq!(
        solve_response_spectrum(&model, 1, 0, &[], SpectrumCombination::Srss, None).expect_err("empty spectrum").code,
        "analysis/invalid-spectrum"
    );
    assert_eq!(
        solve_response_spectrum(
            &model,
            1,
            2,
            &[SpectrumPoint { period: 0.0, acceleration: 1.0 }],
            SpectrumCombination::Srss,
            None
        )
        .expect_err("axis z")
        .code,
        "analysis/invalid-axis"
    );
    assert_eq!(
        solve_response_spectrum(
            &model,
            1,
            0,
            &[SpectrumPoint { period: -1.0, acceleration: 1.0 }],
            SpectrumCombination::Srss,
            None
        )
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
        .response_spectrum(
            1,
            0,
            &[SpectrumPoint { period: 0.0, acceleration: 1.0 }],
            SpectrumCombination::Srss,
            None,
            Some(&out),
        )
        .expect("response spectrum");
    assert_eq!(result.format, "open-cae-response-spectrum-report");
    assert_eq!(result.combination, "srss");
    let text = fs::read_to_string(out).expect("read report");
    assert!(text.contains("open-cae-response-spectrum-report"));
    assert!(text.contains("\"combination\": \"srss\""));
    assert!(!text.contains("\"damping\""));
    assert!(text.contains("\"modes\""));
}
