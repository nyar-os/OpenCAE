use cae_model::{ElementKind, Model, Units, solve_matrices};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;
const RHO: f64 = 7.85e-9;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

/// Two-node axial bar with the free end held against Y translation.
fn axial_bar() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, RHO);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element("e1", "n1", "n2", None).expect("bar");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

fn cantilever_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_full(None, E, A, RHO);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model
}

#[test]
fn axial_bar_stiffness_and_lumped_mass_match_the_closed_form() {
    let matrices = solve_matrices(&axial_bar()).expect("matrices");
    let axial = E * A / L;

    assert_eq!(matrices.size, 4);
    assert_eq!(matrices.dofs.len(), 4);
    assert_eq!(matrices.dofs[0].component, "ux");
    assert_eq!(matrices.dofs[1].component, "uy");
    assert_eq!(matrices.dofs[2].node, "n2");
    assert_eq!(matrices.dofs[2].component, "ux");

    let expected = [[axial, 0.0, -axial, 0.0], [0.0, 0.0, 0.0, 0.0], [-axial, 0.0, axial, 0.0], [0.0, 0.0, 0.0, 0.0]];
    for row in 0..4 {
        for column in 0..4 {
            assert_close(matrices.stiffness[row][column], expected[row][column], 1e-12);
            assert_close(matrices.stiffness[row][column], matrices.stiffness[column][row], 1e-12);
        }
    }

    let lumped = RHO * A * L / 2.0;
    for (index, dof) in matrices.mass.iter().enumerate() {
        for (column, value) in dof.iter().enumerate() {
            assert_close(*value, if index == column { lumped } else { 0.0 }, 1e-12);
        }
    }

    let constrained: Vec<bool> = matrices.dofs.iter().map(|dof| dof.constrained).collect();
    assert_eq!(constrained, vec![true, true, false, true]);
}

#[test]
fn cantilever_beam_stiffness_contains_axial_and_bending_terms() {
    let matrices = solve_matrices(&cantilever_beam()).expect("matrices");
    assert_eq!(matrices.size, 6);
    assert_eq!(matrices.dofs[2].component, "rz");

    let axial = E * A / L;
    let twelve = 12.0 * E * I / L.powi(3);
    let six = 6.0 * E * I / L.powi(2);
    let four = 4.0 * E * I / L;
    let two = 2.0 * E * I / L;

    assert_close(matrices.stiffness[0][0], axial, 1e-12);
    assert_close(matrices.stiffness[0][3], -axial, 1e-12);
    assert_close(matrices.stiffness[1][1], twelve, 1e-12);
    assert_close(matrices.stiffness[1][2], six, 1e-12);
    assert_close(matrices.stiffness[1][4], -twelve, 1e-12);
    assert_close(matrices.stiffness[2][2], four, 1e-12);
    assert_close(matrices.stiffness[2][5], two, 1e-12);

    let translational = RHO * A * L / 2.0;
    let rotary = RHO * I * L / 2.0;
    assert_close(matrices.mass[0][0], translational, 1e-12);
    assert_close(matrices.mass[2][2], rotary, 1e-12);
    assert_close(matrices.mass[1][5], 0.0, 1e-12);
}
