//! Influence-line tests.
//!
//! Simply supported and cantilever two-dimensional beams have closed-form
//! influence lines for reactions and section forces, so every sampled ordinate
//! can be checked against the analytic value.

use cae_model::{
    ElementKind, INFLUENCE_ANALYSIS, INFLUENCE_LINE_REPORT_FORMAT, INFLUENCE_LINE_REPORT_VERSION, InfluenceDirection,
    InfluenceQuantity, Model, ReactionComponent, SectionComponent, Units, solve_influence_line,
};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;
const I: f64 = 1.0e6;

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = expected.abs().max(1.0);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn simply_supported() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material(None, E, 100.0);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

fn cantilever() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material(None, E, 100.0);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model
}

fn reaction(node: &str, component: ReactionComponent) -> InfluenceQuantity {
    InfluenceQuantity::Reaction { node: node.to_string(), component }
}

fn section(position: f64, component: SectionComponent) -> InfluenceQuantity {
    InfluenceQuantity::Section { element: None, position, component }
}

#[test]
fn simply_supported_reaction_follows_the_linear_influence_line() {
    let model = simply_supported();
    let solution = solve_influence_line(&model, "b1", &reaction("n2", ReactionComponent::Fy), InfluenceDirection::Y, 5)
        .expect("influence line");
    assert_eq!(solution.format, INFLUENCE_LINE_REPORT_FORMAT);
    assert_eq!(solution.version, INFLUENCE_LINE_REPORT_VERSION);
    assert_eq!(solution.analysis, INFLUENCE_ANALYSIS);
    assert_eq!(solution.track, "b1");
    assert_eq!(solution.direction, "y");
    assert_eq!(solution.units, "mm");
    assert_eq!(solution.points, 5);
    assert_close(solution.length, L, 1e-12);
    for (index, sample) in solution.samples.iter().enumerate() {
        let t = index as f64 / 4.0;
        assert_close(sample.position, t, 1e-12);
        assert_close(sample.value, -t, 1e-9);
    }
    assert_close(solution.min.value, -1.0, 1e-9);
    assert_close(solution.min.position, 1.0, 1e-12);
    assert_close(solution.max.value, 0.0, 1e-9);
    assert_close(solution.max.position, 0.0, 1e-12);
    assert_close(solution.governing.value, -1.0, 1e-9);
    assert_close(solution.min.value + solution.max.value, -1.0, 1e-9);

    let left = solve_influence_line(&model, "b1", &reaction("n1", ReactionComponent::Fy), InfluenceDirection::Y, 5)
        .expect("left reaction line");
    for (index, sample) in left.samples.iter().enumerate() {
        let t = index as f64 / 4.0;
        assert_close(sample.value, t - 1.0, 1e-9);
    }
}

#[test]
fn simply_supported_midspan_moment_reaches_minus_l_over_four() {
    let model = simply_supported();
    let solution =
        solve_influence_line(&model, "b1", &section(0.5, SectionComponent::Moment), InfluenceDirection::Y, 5).expect("line");
    let expected = [0.0, -125.0, -250.0, -125.0, 0.0];
    assert_eq!(solution.samples.len(), expected.len());
    for (sample, value) in solution.samples.iter().zip(expected) {
        assert_close(sample.value, value, 1e-9);
    }
    assert_close(solution.samples[0].value, 0.0, 1e-9);
    assert_close(solution.samples[4].value, 0.0, 1e-9);
    assert_close(solution.samples[1].value, solution.samples[3].value, 1e-9);
    assert_close(solution.samples[2].value.abs(), 250.0, 1e-9);
    assert_close(solution.governing.value, -250.0, 1e-9);
    assert_close(solution.governing.position, 0.5, 1e-12);
    // The section element defaults to the track element in the report.
    assert_eq!(
        solution.quantity,
        InfluenceQuantity::Section { element: Some("b1".to_string()), position: 0.5, component: SectionComponent::Moment }
    );
}

#[test]
fn cantilever_reaction_and_moment_are_constant_and_linear() {
    let model = cantilever();
    let shear = solve_influence_line(&model, "b1", &reaction("n1", ReactionComponent::Fy), InfluenceDirection::Y, 5)
        .expect("shear line");
    for sample in &shear.samples {
        assert_close(sample.value, -1.0, 1e-9);
    }
    let moment = solve_influence_line(&model, "b1", &reaction("n1", ReactionComponent::Mz), InfluenceDirection::Y, 5)
        .expect("moment line");
    for (index, sample) in moment.samples.iter().enumerate() {
        let t = index as f64 / 4.0;
        assert_close(sample.value, -t * L, 1e-9);
    }
    assert_close(moment.governing.value, -1000.0, 1e-9);
}

#[test]
fn influence_line_reports_are_deterministic() {
    let model = simply_supported();
    let first = serde_json::to_string(
        &solve_influence_line(&model, "b1", &section(0.5, SectionComponent::Moment), InfluenceDirection::Y, 7).expect("first"),
    )
    .expect("json");
    let second = serde_json::to_string(
        &solve_influence_line(&model, "b1", &section(0.5, SectionComponent::Moment), InfluenceDirection::Y, 7).expect("second"),
    )
    .expect("json");
    assert_eq!(first, second);
}

#[test]
fn influence_line_validation_reports_stable_codes() {
    let model = simply_supported();
    let right = reaction("n2", ReactionComponent::Fy);

    let error = solve_influence_line(&model, "b9", &right, InfluenceDirection::Y, 5).unwrap_err();
    assert_eq!(error.code, "element/unknown-id");

    let mut truss = Model::new(Units::Millimeter);
    truss.set_material(None, E, 100.0);
    truss.add_node("n1", 0.0, 0.0).expect("n1");
    truss.add_node("n2", L, 0.0).expect("n2");
    truss.add_element_kind("e1", "n1", "n2", None, ElementKind::Truss, None, None).expect("truss");
    truss.add_support("n1", true, true).expect("pin");
    truss.add_support("n2", false, true).expect("roller");
    let error = solve_influence_line(&truss, "e1", &right, InfluenceDirection::Y, 5).unwrap_err();
    assert_eq!(error.code, "influence/track-not-beam");

    let error = solve_influence_line(&model, "b1", &right, InfluenceDirection::Y, 1).unwrap_err();
    assert_eq!(error.code, "analysis/invalid-points");

    let error =
        solve_influence_line(&model, "b1", &section(1.5, SectionComponent::Moment), InfluenceDirection::Y, 5).unwrap_err();
    assert_eq!(error.code, "influence/invalid-position");

    let mut with_truss = simply_supported();
    with_truss.add_element_kind("e1", "n1", "n2", None, ElementKind::Truss, None, None).expect("truss");
    let error = solve_influence_line(
        &with_truss,
        "b1",
        &InfluenceQuantity::Section { element: Some("e1".to_string()), position: 0.5, component: SectionComponent::Shear },
        InfluenceDirection::Y,
        5,
    )
    .unwrap_err();
    assert_eq!(error.code, "influence/invalid-component");

    let mut loaded = simply_supported();
    loaded.add_load("n2", None, Some(-10.0)).expect("load");
    let error = solve_influence_line(&loaded, "b1", &right, InfluenceDirection::Y, 5).unwrap_err();
    assert_eq!(error.code, "influence/loads-present");

    let error =
        solve_influence_line(&model, "b1", &reaction("n9", ReactionComponent::Fy), InfluenceDirection::Y, 5).unwrap_err();
    assert_eq!(error.code, "node/unknown-id");

    let mut free = simply_supported();
    free.add_node("n3", L / 2.0, 0.0).expect("n3");
    let error =
        solve_influence_line(&free, "b1", &reaction("n3", ReactionComponent::Fy), InfluenceDirection::Y, 5).unwrap_err();
    assert_eq!(error.code, "result/no-reaction");

    let mut three_d = simply_supported();
    three_d.dimension = 3;
    let error = solve_influence_line(&three_d, "b1", &right, InfluenceDirection::Y, 5).unwrap_err();
    assert_eq!(error.code, "capability/not-implemented");
}
