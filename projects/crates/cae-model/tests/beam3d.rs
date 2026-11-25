//! Analytical checks for the spatial `beam3d` formulation.
//!
//! Every test compares the solver against a closed-form result: cantilever
//! deflections, torsion and axial springs, a simply supported span, an L-shaped
//! frame combining bending and torsion, the orientation conventions and an
//! exact degeneracy against the two-dimensional `beam` element.

use cae_model::{
    ElementKind, LoadCombination, Model, Units, solve, solve_buckling, solve_combinations, solve_deflection,
    solve_internal_forces, solve_modal, solve_stress,
};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const J: f64 = 2.0e6;
const L: f64 = 1000.0;
const P: f64 = 10_000.0;
const G: f64 = E / 2.6;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn assert_near_zero(actual: f64, absolute: f64) {
    assert!(actual.abs() <= absolute, "expected ~0, got {actual}");
}

fn frame_model() -> Model {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.material.youngs_modulus = E;
    model.material.area = A;
    model
}

fn node<'a>(solution: &'a cae_model::Solution, id: &str) -> &'a cae_model::NodeDisplacement {
    solution.nodes.iter().find(|node| node.id == id).expect("node")
}

fn reaction<'a>(solution: &'a cae_model::Solution, node_id: &str) -> &'a cae_model::NodeReaction {
    solution.reactions.iter().find(|reaction| reaction.node == node_id).expect("reaction")
}

/// A horizontal `beam3d` member with the default reference vector has its
/// local `y` and `z` axes aligned with the global ones.
fn cantilever_x(inertia_weak: Option<f64>) -> Model {
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    model
        .add_frame_element("b1", "n1", "n2", None, Some(I), inertia_weak, Some(J), None, None, None, None, None, None)
        .expect("b1");
    model.add_support_frame("n1", true, true, true, true, true, true).expect("fixed");
    model
}

#[test]
fn cantilever_tip_load_in_local_y_matches_theory() {
    let model = {
        let mut model = cantilever_x(None);
        model.add_load_frame("n2", None, Some(-P), None, None, None, None).expect("load");
        model
    };
    let solution = solve(&model).expect("solve");
    let tip = node(&solution, "n2");
    assert_close(tip.uy, -P * L.powi(3) / (3.0 * E * I), 1e-9);
    assert_close(tip.rz, -P * L * L / (2.0 * E * I), 1e-9);
    assert_close(tip.ux, 0.0, 1e-9);
    assert_close(tip.uz, 0.0, 1e-9);
    assert_close(tip.rx.unwrap_or(f64::NAN), 0.0, 1e-9);
    assert_close(tip.ry.unwrap_or(f64::NAN), 0.0, 1e-9);
    let fixed = reaction(&solution, "n1");
    assert_close(fixed.fy, P, 1e-9);
    assert_close(fixed.mz, P * L, 1e-9);
    assert_close(fixed.mx.unwrap_or(f64::NAN), 0.0, 1e-9);
    assert_close(fixed.my.unwrap_or(f64::NAN), 0.0, 1e-9);
    assert!(solution.reaction_summary.balanced);
    let member = &solution.elements[0];
    assert_eq!(member.kind, ElementKind::Beam3D);
    assert_close(member.axial_force, 0.0, 1e-9);
    let spatial = member.spatial.as_ref().expect("spatial end forces");
    assert_close(spatial.vy1, P, 1e-9);
    assert_close(spatial.mz1, P * L, 1e-9);
    // The free end is the loaded end: the member-end force equals the applied
    // load, and no moment acts there.
    assert_close(spatial.vy2, -P, 1e-9);
    assert_near_zero(spatial.mz2, 1e-6);
}

#[test]
fn cantilever_tip_load_in_global_z_uses_the_weak_inertia() {
    let weak = 5.0e5;
    let model = {
        let mut model = cantilever_x(Some(weak));
        model.add_load_frame("n2", None, None, Some(P), None, None, None).expect("load");
        model
    };
    let solution = solve(&model).expect("solve");
    let tip = node(&solution, "n2");
    assert_close(tip.uz, P * L.powi(3) / (3.0 * E * weak), 1e-9);
    assert_close(tip.ry.unwrap_or(f64::NAN), -P * L * L / (2.0 * E * weak), 1e-9);
    assert_close(tip.uy, 0.0, 1e-9);
    assert_close(tip.rz, 0.0, 1e-9);
    let fixed = reaction(&solution, "n1");
    assert_close(fixed.fz, -P, 1e-9);
    assert_close(fixed.my.unwrap_or(f64::NAN), P * L, 1e-9);
    assert!(solution.reaction_summary.balanced);
    let spatial = solution.elements[0].spatial.as_ref().expect("spatial end forces");
    assert_close(spatial.vz1, -P, 1e-9);
    assert_close(spatial.my1, P * L, 1e-9);
}

#[test]
fn axial_force_and_torsion_superpose() {
    let model = {
        let mut model = cantilever_x(None);
        model.add_load_frame("n2", Some(P), None, None, Some(P * 100.0), None, None).expect("load");
        model
    };
    let solution = solve(&model).expect("solve");
    let tip = node(&solution, "n2");
    assert_close(tip.ux, P * L / (E * A), 1e-9);
    assert_close(tip.rx.unwrap_or(f64::NAN), P * 100.0 * L / (G * J), 1e-9);
    assert_close(tip.uy, 0.0, 1e-9);
    let member = &solution.elements[0];
    assert_close(member.axial_force, P, 1e-9);
    let spatial = member.spatial.as_ref().expect("spatial end forces");
    assert_close(spatial.n1, -P, 1e-9);
    assert_close(spatial.n2, P, 1e-9);
    assert_close(spatial.t1, -P * 100.0, 1e-9);
    assert_close(spatial.t2, P * 100.0, 1e-9);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn simply_supported_span_under_uniform_z_load_matches_closed_form() {
    let q = 10.0;
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L / 2.0, 0.0, 0.0).expect("n2");
    model.add_node_3d("n3", L, 0.0, 0.0).expect("n3");
    for (id, from, to) in [("e1", "n1", "n2"), ("e2", "n2", "n3")] {
        model
            .add_frame_element(id, from, to, None, Some(I), Some(I), Some(J), None, None, None, None, Some(q), Some(q))
            .expect(id);
    }
    model.add_support_frame("n1", true, true, true, true, false, false).expect("support");
    model.add_support_frame("n3", true, true, true, true, false, false).expect("support");
    let solution = solve(&model).expect("solve");
    let mid = node(&solution, "n2");
    assert_close(mid.uz, 5.0 * q * L.powi(4) / (384.0 * E * I), 1e-8);
    assert_near_zero(mid.uy, 1e-9);
    for id in ["n1", "n3"] {
        let support = reaction(&solution, id);
        assert_close(support.fz, -q * L / 2.0, 1e-9);
        assert_near_zero(support.my.unwrap_or(f64::NAN), 1e-6);
    }
    assert!(solution.reaction_summary.balanced);
    let diagram = solve_internal_forces(&model, 5).expect("internal forces");
    let mid_station = &diagram.elements[0].stations[diagram.elements[0].stations.len() / 2];
    assert_close(mid_station.position, L / 4.0, 1e-12);
    assert_close(mid_station.shear_z.unwrap_or(f64::NAN), -q * L / 4.0, 1e-9);
    assert_close(mid_station.moment_y.unwrap_or(f64::NAN), 3.0 * q * L * L / 32.0, 1e-9);
}

#[test]
fn orientation_angle_swaps_the_bending_planes() {
    let weak = 5.0e5;
    let build = |angle: Option<f64>| {
        let mut model = cantilever_x(Some(weak));
        if angle.is_some() {
            model.elements[0].orientation_angle = angle;
        }
        model.add_load_frame("n2", None, None, Some(P), None, None, None).expect("load");
        solve(&model).expect("solve")
    };
    let default_axes = build(None);
    assert_close(node(&default_axes, "n2").uz, P * L.powi(3) / (3.0 * E * weak), 1e-9);
    let rotated = build(Some(std::f64::consts::FRAC_PI_2));
    assert_close(node(&rotated, "n2").uz, P * L.powi(3) / (3.0 * E * I), 1e-9);
}

#[test]
fn vertical_member_falls_back_to_the_reference_vector() {
    let model = {
        let mut model = frame_model();
        model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
        model.add_node_3d("n2", 0.0, 0.0, L).expect("n2");
        model
            .add_frame_element("c1", "n1", "n2", None, Some(I), Some(I), Some(J), None, None, None, None, None, None)
            .expect("c1");
        model.add_support_frame("n1", true, true, true, true, true, true).expect("fixed");
        model.add_load_frame("n2", Some(P), None, None, None, None, None).expect("load");
        model
    };
    let solution = solve(&model).expect("solve");
    let tip = node(&solution, "n2");
    assert_close(tip.ux, P * L.powi(3) / (3.0 * E * I), 1e-9);
    assert_close(tip.ry.unwrap_or(f64::NAN).abs(), P * L * L / (2.0 * E * I), 1e-9);
    assert_close(tip.uy, 0.0, 1e-9);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn l_shaped_cantilever_combines_bending_and_torsion() {
    let model = {
        let mut model = frame_model();
        model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
        model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
        model.add_node_3d("n3", L, 0.0, L).expect("n3");
        model
            .add_frame_element("e1", "n1", "n2", None, Some(I), None, Some(J), None, None, None, None, None, None)
            .expect("e1");
        model
            .add_frame_element("e2", "n2", "n3", None, Some(I), None, Some(J), None, None, None, None, None, None)
            .expect("e2");
        model.add_support_frame("n1", true, true, true, true, true, true).expect("fixed");
        model.add_load_frame("n3", None, Some(-P), None, None, None, None).expect("load");
        model
    };
    let solution = solve(&model).expect("solve");
    let tip = node(&solution, "n3");
    let expected = -(2.0 * P * L.powi(3) / (3.0 * E * I) + P * L.powi(3) / (G * J));
    assert_close(tip.uy, expected, 1e-8);
    let fixed = reaction(&solution, "n1");
    assert_close(fixed.fy, P, 1e-9);
    assert_close(fixed.mx.unwrap_or(f64::NAN), -P * L, 1e-9);
    assert_close(fixed.my.unwrap_or(f64::NAN), 0.0, 1e-9);
    assert_close(fixed.mz, P * L, 1e-9);
    assert!(solution.reaction_summary.balanced);
    let e1 = solution.elements.iter().find(|element| element.id == "e1").expect("e1");
    let spatial = e1.spatial.as_ref().expect("spatial");
    assert_close(spatial.t1.abs(), P * L, 1e-9);
    assert_close(spatial.t2.abs(), P * L, 1e-9);
}

#[test]
fn spatial_member_degenerates_to_the_two_dimensional_beam() {
    let q = 8.0;
    let mut planar = Model::new(Units::Millimeter);
    planar.material.youngs_modulus = E;
    planar.material.area = A;
    planar.add_node("n1", 0.0, 0.0).expect("n1");
    planar.add_node("n2", L, 0.0).expect("n2");
    planar.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(q), Some(q)).expect("beam");
    planar.add_support_full("n1", true, true, false, true).expect("fixed");
    planar.add_load_full("n2", Some(P), Some(-P), None, Some(P * 50.0)).expect("load");
    let planar = solve(&planar).expect("solve 2d");

    let spatial_model = {
        let mut model = cantilever_x(Some(I));
        model.elements[0].distributed_load = Some(q);
        model.elements[0].distributed_load_end = Some(q);
        model.add_load_frame("n2", Some(P), Some(-P), None, None, None, Some(P * 50.0)).expect("load");
        model
    };
    let spatial = solve(&spatial_model).expect("solve 3d");

    assert_close(node(&spatial, "n2").uy, node(&planar, "n2").uy, 1e-9);
    assert_close(node(&spatial, "n2").rz, node(&planar, "n2").rz, 1e-9);
    assert_close(node(&spatial, "n2").ux, node(&planar, "n2").ux, 1e-9);
    assert_close(node(&spatial, "n2").uz, 0.0, 1e-9);
    let planar_end = &planar.elements[0].end_forces;
    let spatial_end = &spatial.elements[0].end_forces;
    for (a, b) in [(&planar_end.n1, &spatial_end.n1), (&planar_end.v1, &spatial_end.v1), (&planar_end.m1, &spatial_end.m1)] {
        assert_close(*b, *a, 1e-9);
    }
    assert_close(spatial.elements[0].axial_force, planar.elements[0].axial_force, 1e-9);
}

#[test]
fn unsupported_analyses_reject_beam3d_models() {
    let model = {
        let mut model = cantilever_x(None);
        model.add_load_frame("n2", None, Some(-P), None, None, None, None).expect("load");
        model
    };
    for result in [
        solve_modal(&model, 2).map(|_| ()),
        solve_buckling(&model, 2).map(|_| ()),
        solve_stress(&model, 5).map(|_| ()),
        solve_deflection(&model, 5).map(|_| ()),
    ] {
        let error = result.expect_err("beam3d analysis must be rejected");
        assert_eq!(error.code, "capability/not-implemented", "{}", error.message);
    }
}

#[test]
fn combinations_solve_beam3d_models() {
    let model = {
        let mut model = cantilever_x(None);
        model.add_load_frame("n2", None, Some(-P), None, None, None, None).expect("load");
        model
    };
    let combinations =
        vec![LoadCombination { name: "ULS".to_string(), loads: 1.5, gravity: 0.0, thermal: 1.0, patterns: vec![] }];
    let solution = solve_combinations(&model, &combinations).expect("combinations");
    let case = &solution.combinations[0];
    assert_close(node(&case.solution, "n2").uy, -1.5 * P * L.powi(3) / (3.0 * E * I), 1e-9);
}

#[test]
fn validation_and_construction_reject_bad_beam3d_documents() {
    // A beam3d element in a two-dimensional document.
    let mut planar = Model::new(Units::Millimeter);
    planar.add_node("n1", 0.0, 0.0).expect("n1");
    planar.add_node("n2", L, 0.0).expect("n2");
    assert_eq!(
        planar
            .add_frame_element("b1", "n1", "n2", None, Some(I), None, Some(J), None, None, None, None, None, None)
            .expect_err("dimension")
            .code,
        "document/dimension-mismatch"
    );

    // A missing torsion constant without a section fallback.
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    assert_eq!(
        model
            .add_frame_element("b1", "n1", "n2", None, Some(I), None, None, None, None, None, None, None, None)
            .expect_err("torsion")
            .code,
        "element/invalid-torsion"
    );

    // An orientation vector parallel to the member axis.
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    assert_eq!(
        model
            .add_frame_element(
                "b1",
                "n1",
                "n2",
                None,
                Some(I),
                None,
                Some(J),
                Some([1.0, 0.0, 0.0]),
                None,
                None,
                None,
                None,
                None
            )
            .expect_err("orientation")
            .code,
        "element/invalid-orientation"
    );

    // A rotation support at a node without beam3d members.
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", L, 0.0, 0.0).expect("n2");
    model.add_node_3d("n3", 0.0, L, 0.0).expect("n3");
    model.add_frame_element("b1", "n1", "n2", None, Some(I), None, Some(J), None, None, None, None, None, None).expect("b1");
    model.add_support_frame("n3", false, false, false, true, false, false).expect("rx");
    assert!(model.validate().iter().any(|issue| issue.code == "support/inactive-dof"));
}

#[test]
fn json_round_trip_preserves_frame_fields() {
    let mut model = frame_model();
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("n1");
    model.add_node_3d("n2", 0.0, 0.0, L).expect("n2");
    model
        .add_frame_element(
            "c1",
            "n1",
            "n2",
            None,
            Some(I),
            Some(5.0e5),
            Some(J),
            Some([1.0, 1.0, 0.9]),
            Some(0.25),
            Some(3.0),
            Some(4.0),
            Some(5.0),
            Some(6.0),
        )
        .expect("c1");
    model.add_support_frame("n1", true, true, true, true, true, true).expect("support");
    model.add_load_frame("n2", Some(P), None, None, Some(1.0), Some(2.0), Some(3.0)).expect("load");
    let text = model.to_json_pretty().expect("serialize");
    let parsed = Model::parse_json(&text).expect("parse");
    assert_eq!(model, parsed);
}
