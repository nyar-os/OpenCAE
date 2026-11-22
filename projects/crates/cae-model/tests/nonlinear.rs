use cae_model::{ElementKind, Model, Units, solve_nonlinear};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

fn axial_bar() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, 0.0);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

/// Symmetric two-bar shallow truss with a vertical apex load.
fn von_mises_truss(half_span: f64, rise: f64, load: f64) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, 0.0);
    model.add_node("left", -half_span, 0.0).expect("left");
    model.add_node("right", half_span, 0.0).expect("right");
    model.add_node("apex", 0.0, rise).expect("apex");
    model.add_element("left-bar", "left", "apex", None).expect("left bar");
    model.add_element("right-bar", "right", "apex", None).expect("right bar");
    model.add_support("left", true, true).expect("left pin");
    model.add_support("right", true, true).expect("right pin");
    model.add_load("apex", None, Some(-load)).expect("apex load");
    model
}

/// Independent scalar equilibrium solve for the apex settlement `v` (downward
/// positive) of the symmetric two-bar truss.
fn apex_settlement(half_span: f64, rise: f64, load: f64) -> f64 {
    let initial = (half_span * half_span + rise * rise).sqrt();
    let residual = |v: f64| {
        let height = rise - v;
        let length = (half_span * half_span + height * height).sqrt();
        let axial = E * A * (length - initial) / initial;
        // Equilibrium of the apex: the downward load plus the vertical
        // component of the (compressive) bar forces must vanish.
        load + 2.0 * axial * height / length
    };
    let mut lower = 0.0;
    let mut upper = 0.0;
    for step in 1..=1000 {
        let candidate = rise * step as f64 / 1000.0;
        if residual(candidate) <= 0.0 {
            upper = candidate;
            lower = rise * (step - 1) as f64 / 1000.0;
            break;
        }
    }
    assert!(upper > 0.0, "the scalar equilibrium has no bracket below the rise");
    for _ in 0..200 {
        let middle = 0.5 * (lower + upper);
        if residual(middle) > 0.0 {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    0.5 * (lower + upper)
}

#[test]
fn linear_axial_bar_reproduces_the_static_displacement() {
    let mut model = axial_bar();
    model.add_load("n2", Some(1000.0), None).expect("load");
    let result = solve_nonlinear(&model, 5, 20, 1e-10).expect("nonlinear");
    let extension = 1000.0 * L / (E * A);
    assert_close(result.nodes[1].ux, extension, 1e-9);
    assert_close(result.max_displacement.magnitude, extension, 1e-9);
    assert_eq!(result.max_displacement.id, "n2");
    assert!(result.iterations >= result.increments);
    assert_eq!(result.format, "open-cae-nonlinear-report");
    assert!(result.residual_norm < 1e-7, "residual {}", result.residual_norm);
}

#[test]
fn shallow_truss_settles_following_the_geometrically_nonlinear_equilibrium() {
    let half_span = 500.0;
    let rise = 10.0;
    let load = 20.0;
    let model = von_mises_truss(half_span, rise, load);
    let result = solve_nonlinear(&model, 20, 30, 1e-10).expect("nonlinear");

    let expected = apex_settlement(half_span, rise, load);
    let apex = result.nodes.iter().find(|node| node.id == "apex").expect("apex");
    assert_close(-apex.uy, expected, 1e-6);
    assert_close(apex.magnitude, expected, 1e-6);
    assert_eq!(result.max_displacement.id, "apex");
    assert!(result.iterations >= result.increments);
    assert!(result.residual_norm < 1e-7, "residual {}", result.residual_norm);
}

#[test]
fn beams_and_invalid_controls_are_rejected() {
    let mut beam = Model::new(Units::Millimeter);
    beam.set_material_full(None, E, A, 0.0);
    beam.add_node("n1", 0.0, 0.0).expect("n1");
    beam.add_node("n2", L, 0.0).expect("n2");
    beam.add_element_kind("e1", "n1", "n2", None, ElementKind::Beam, Some(1e6), None).expect("beam");
    beam.add_support_full("n1", true, true, false, true).expect("fixed");
    assert_eq!(solve_nonlinear(&beam, 10, 30, 1e-10).expect_err("beam model").code, "capability/not-implemented");

    let model = axial_bar();
    assert_eq!(solve_nonlinear(&model, 0, 30, 1e-10).expect_err("no increments").code, "analysis/invalid-increments");
    assert_eq!(solve_nonlinear(&model, 10, 0, 1e-10).expect_err("no iterations").code, "analysis/invalid-iterations");
    assert_eq!(solve_nonlinear(&model, 10, 30, 0.0).expect_err("zero tolerance").code, "analysis/invalid-tolerance");
}
