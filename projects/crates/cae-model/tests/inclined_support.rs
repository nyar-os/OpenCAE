use cae_model::{Model, Units, solve, solve_matrices};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;
const LOAD: f64 = 1000.0;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

fn assert_zero(actual: f64) {
    assert!(actual.abs() <= 1e-9 * LOAD, "expected 0, got {actual}");
}

/// Diagonal bar at 45° with a pin at `n1` and an inclined roller at `n2`
/// constraining the direction perpendicular to the bar.
fn diagonal_bar() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, 7.85e-9);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, L).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support_direction("n2", -1.0, 1.0).expect("inclined roller");
    model.add_load_3d("n2", Some(LOAD / 2.0_f64.sqrt()), Some(LOAD / 2.0_f64.sqrt()), None).expect("axial load");
    model
}

#[test]
fn inclined_roller_leaves_the_bar_axial_displacement() {
    let result = solve(&diagonal_bar()).expect("solve");
    let axial = LOAD * (L * 2.0_f64.sqrt()) / (E * A);
    let component = axial / 2.0_f64.sqrt();
    assert_close(result.nodes[1].ux, component, 1e-9);
    assert_close(result.nodes[1].uy, component, 1e-9);

    // The roller is perpendicular to the bar, so it carries no reaction: the
    // pin at `n1` equilibrates the whole load.
    assert_zero(result.reactions[1].fx);
    assert_zero(result.reactions[1].fy);
    assert_close(result.reactions[0].fx, -LOAD / 2.0_f64.sqrt(), 1e-9);
    assert_close(result.reactions[0].fy, -LOAD / 2.0_f64.sqrt(), 1e-9);
    assert!(result.reaction_summary.balanced);
}

#[test]
fn inclined_direction_is_reported_through_global_matrix_and_reactions() {
    let model = diagonal_bar();
    let result = solve(&model).expect("solve");
    let sum_fx: f64 = result.reactions.iter().map(|node| node.fx).sum();
    let sum_fy: f64 = result.reactions.iter().map(|node| node.fy).sum();
    assert_close(sum_fx, -LOAD / 2.0_f64.sqrt(), 1e-9);
    assert_close(sum_fy, -LOAD / 2.0_f64.sqrt(), 1e-9);

    let matrices = solve_matrices(&model).expect("matrices");
    // The exported stiffness is expressed in global coordinates again, so it
    // is the standard bar matrix k·[[c², cs, −c², −cs], …] with c = s = 1/√2.
    let axial = E * A / (L * 2.0_f64.sqrt());
    let half = 0.5 * axial;
    assert_close(matrices.stiffness[2][2], half, 1e-9);
    assert_close(matrices.stiffness[2][3], half, 1e-9);
    assert_close(matrices.stiffness[2][0], -half, 1e-9);
    assert_close(matrices.stiffness[2][1], -half, 1e-9);
    assert!(matrices.dofs[2].constrained, "the local normal degree of freedom is constrained");
    assert!(!matrices.dofs[3].constrained);
}

#[test]
fn horizontal_roller_direction_matches_a_global_y_support() {
    fn model(direction: bool) -> Model {
        let mut model = Model::new(Units::Millimeter);
        model.set_material_full(None, E, A, 7.85e-9);
        model.add_node("n1", 0.0, 0.0).expect("n1");
        model.add_node("n2", L, 0.0).expect("n2");
        model.add_element("e1", "n1", "n2", None).expect("bar");
        model.add_support("n1", true, true).expect("pin");
        if direction {
            model.add_support_direction("n2", 0.0, 1.0).expect("inclined roller");
        }
        else {
            model.add_support("n2", false, true).expect("roller");
        }
        model.add_load_3d("n2", Some(LOAD), Some(0.0), None).expect("axial load");
        model
    }

    let inclined = solve(&model(true)).expect("inclined");
    let global = solve(&model(false)).expect("global");
    assert_close(inclined.nodes[1].ux, global.nodes[1].ux, 1e-12);
    assert_close(inclined.nodes[1].uy, global.nodes[1].uy, 1e-12);
    assert_close(inclined.reactions[1].fy, global.reactions[1].fy, 1e-12);
}

#[test]
fn invalid_inclined_supports_are_rejected() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    assert_eq!(model.add_support_direction("n1", 0.0, 0.0).expect_err("zero vector").code, "support/invalid-direction");

    let mut conflicting = Model::new(Units::Millimeter);
    conflicting.add_node("n1", 0.0, 0.0).expect("n1");
    conflicting.add_support("n1", true, false).expect("dx");
    assert_eq!(conflicting.add_support_direction("n1", 0.0, 1.0).expect_err("dx conflict").code, "support/invalid-direction");

    let mut three = Model::new(Units::Millimeter);
    three.dimension = 3;
    three.add_node("n1", 0.0, 0.0).expect("n1");
    assert_eq!(three.add_support_direction("n1", 0.0, 1.0).expect_err("3D").code, "document/dimension-mismatch");
}
