//! Uniform temperature-change (thermal expansion) tests.
//!
//! A uniform temperature change only produces axial strain; the equivalent
//! nodal load is `E·A·alpha·ΔT`, so a free member expands without stress and a
//! restrained member carries `N = −E·A·alpha·ΔT`.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{
    ElementKind, Material, Model, OpenCae, OpenCaeOptions, Section, Severity, Units, solve, solve_deflection,
    solve_internal_forces,
};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const L: f64 = 1000.0;
const I: f64 = 1.0e6;
const ALPHA: f64 = 12.0e-6;
const DT: f64 = 50.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-thermal-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn material(alpha: Option<f64>) -> Material {
    Material {
        name: "steel".to_string(),
        youngs_modulus: E,
        area: A,
        density: 0.0,
        allowable_stress: None,
        alpha,
        conductivity: None,
        shear_modulus: None,
        poissons_ratio: None,
        section: None,
    }
}

fn bar(alpha: Option<f64>, kind: ElementKind, inertia: Option<f64>) -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.set_material_data(material(alpha)).expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("e1", "n1", "n2", None, kind, inertia, None).expect("element");
    model.set_element_temperature("e1", Some(DT)).expect("temperature");
    model
}

#[test]
fn free_member_expands_without_stress() {
    let mut model = bar(Some(ALPHA), ElementKind::Truss, None);
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("lateral guide");

    let solution = cae_model::solve(&model).expect("solve");
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2");
    assert_close(tip.ux, ALPHA * DT * L, 1e-12);
    assert_close(tip.uy, 0.0, 1e-12);
    let element = &solution.elements[0];
    assert_close(element.axial_force, 0.0, 1e-12);
    assert_close(element.stress, 0.0, 1e-12);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn restrained_member_carries_the_thermal_force() {
    let mut model = bar(Some(ALPHA), ElementKind::Truss, None);
    model.add_support("n1", true, true).expect("fixed");
    model.add_support("n2", true, true).expect("fixed");

    let solution = cae_model::solve(&model).expect("solve");
    let expected = E * A * ALPHA * DT;
    let element = &solution.elements[0];
    assert_close(element.axial_force, -expected, 1e-12);
    assert_close(element.stress, -E * ALPHA * DT, 1e-12);

    let reaction = solution.reactions.iter().find(|reaction| reaction.node == "n1").expect("n1 reaction");
    assert_close(reaction.fx, expected, 1e-12);
    assert!(solution.reaction_summary.balanced);
}

#[test]
fn uniform_temperature_bends_no_cantilever_beam() {
    let mut model = bar(Some(ALPHA), ElementKind::Beam, Some(I));
    model.add_support_full("n1", true, true, false, true).expect("fixed");

    let solution = cae_model::solve(&model).expect("solve");
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2");
    assert_close(tip.ux, ALPHA * DT * L, 1e-12);
    assert_close(tip.uy, 0.0, 1e-12);
    assert_close(tip.rz, 0.0, 1e-12);
    assert_close(solution.elements[0].axial_force, 0.0, 1e-12);
    assert_close(solution.max_moment.moment, 0.0, 1e-12);
}

#[test]
fn temperature_without_alpha_is_rejected_and_reported() {
    let mut model = bar(None, ElementKind::Truss, None);
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("guide");

    let issues = model.validate();
    assert!(
        issues.iter().any(|issue| issue.code == "material/missing-alpha" && issue.severity == Severity::Error),
        "issues: {issues:?}"
    );
    assert_eq!(cae_model::solve(&model).expect_err("missing alpha").code, "material/missing-alpha");

    let mut with_zero = bar(Some(0.0), ElementKind::Truss, None);
    with_zero.add_support("n1", true, true).expect("pin");
    with_zero.add_support("n2", false, true).expect("guide");
    assert!(
        with_zero
            .validate()
            .iter()
            .any(|issue| issue.code == "element/inactive-temperature" && issue.severity == Severity::Warning)
    );
}

#[test]
fn temperature_edits_reject_invalid_input_and_round_trip() {
    let directory = temporary_dir("roundtrip");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.set_material(E, A, Some("steel".to_string())).expect("material");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.add_element_thermal("e1", "n1", "n2", None, None, ElementKind::Truss, None, None, None, Some(DT)).expect("element");
    session.add_support("n1", true, true, false).expect("pin");
    session.add_support("n2", false, true, false).expect("guide");

    let error = session.set_element_temperature("e1", Some(f64::NAN)).expect_err("non-finite");
    assert_eq!(error.code, "element/invalid-temperature");
    let error = session.set_element_temperature("missing", Some(1.0)).expect_err("unknown element");
    assert_eq!(error.code, "element/unknown-id");

    // The thermal element needs alpha before it can be solved.
    let out = directory.join("report.json");
    assert_eq!(session.solve(Some(&out)).expect_err("missing alpha").code, "material/missing-alpha");

    let document = Material {
        name: "steel".to_string(),
        youngs_modulus: E,
        area: A,
        density: 0.0,
        allowable_stress: None,
        alpha: Some(ALPHA),
        conductivity: None,
        shear_modulus: None,
        poissons_ratio: None,
        section: None,
    };
    session.set_material_data(document).expect("material with alpha");

    let solution = session.solve(None).expect("solve");
    assert_close(solution.nodes.iter().find(|node| node.id == "n2").expect("n2").ux, ALPHA * DT * L, 1e-12);

    let text = fs::read_to_string(&path).expect("document");
    assert!(text.contains("\"temperature\": 50.0"), "document: {text}");
    assert!(text.contains("\"alpha\": 0.000012"), "document: {text}");
}

const DEPTH: f64 = 200.0;
const GRADIENT: f64 = 40.0;

fn gradient_material(alpha: Option<f64>, section: Option<Section>) -> Material {
    Material {
        name: "steel".to_string(),
        youngs_modulus: E,
        area: A,
        density: 0.0,
        allowable_stress: None,
        alpha,
        conductivity: None,
        shear_modulus: None,
        poissons_ratio: None,
        section,
    }
}

fn gradient_beam(fixed_both_ends: bool) -> Model {
    let mut model = Model::new(Units::Millimeter);
    let section = Some(Section::Rectangle { width: 100.0, height: DEPTH });
    model.set_material_data(gradient_material(Some(ALPHA), section)).expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    model.set_element_temperature_gradient("b1", Some(GRADIENT)).expect("gradient");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    if fixed_both_ends {
        model.add_support_full("n2", true, true, false, true).expect("fixed");
    }
    model
}

#[test]
fn gradient_on_a_cantilever_bends_it_without_end_moments() {
    let model = gradient_beam(false);
    // Free curvature `kappa = -alpha·ΔT/h`, hot side at local `+y`.
    let curvature = -ALPHA * GRADIENT / DEPTH;
    let solution = solve(&model).expect("solve");
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("n2");
    assert_close(tip.uy, curvature * L * L / 2.0, 1e-12);
    assert_close(tip.rz, curvature * L, 1e-12);
    assert_close(tip.ux, 0.0, 1e-12);
    let moment_scale = E * I * ALPHA * GRADIENT / DEPTH;
    assert!(solution.elements[0].end_forces.m1.abs() < 1e-9 * moment_scale);
    assert!(solution.elements[0].end_forces.m2.abs() < 1e-9 * moment_scale);
    assert!(solution.max_moment.moment.abs() < 1e-9 * moment_scale);
    assert!(solution.reaction_summary.balanced);

    let diagram = solve_internal_forces(&model, 5).expect("diagram");
    for station in &diagram.elements[0].stations {
        assert!(station.moment.abs() < 1e-9 * moment_scale);
        assert!(station.shear.abs() < 1e-9 * moment_scale / L);
    }

    let deflection = solve_deflection(&model, 5).expect("deflection");
    let tip_station = deflection.elements[0].stations.last().expect("tip");
    assert_close(tip_station.transverse, curvature * L * L / 2.0, 1e-12);
    assert_close(tip_station.rotation, curvature * L, 1e-12);
}

#[test]
fn gradient_on_a_fixed_fixed_beam_produces_the_restraining_end_moments() {
    let model = gradient_beam(true);
    let moment = E * I * ALPHA * GRADIENT / DEPTH;
    let solution = solve(&model).expect("solve");

    for node in &solution.nodes {
        assert_close(node.ux, 0.0, 1e-12);
        assert_close(node.uy, 0.0, 1e-12);
        assert_close(node.rz, 0.0, 1e-12);
    }
    assert_close(solution.elements[0].end_forces.m1, -moment, 1e-12);
    assert_close(solution.elements[0].end_forces.m2, moment, 1e-12);
    assert_close(solution.max_moment.moment.abs(), moment, 1e-12);
    let start = solution.reactions.iter().find(|reaction| reaction.node == "n1").expect("n1");
    let end = solution.reactions.iter().find(|reaction| reaction.node == "n2").expect("n2");
    assert_close(start.mz, -moment, 1e-12);
    assert_close(end.mz, moment, 1e-12);
    assert_close(start.fy, 0.0, 1e-12);
    assert!(solution.reaction_summary.balanced);

    let diagram = solve_internal_forces(&model, 5).expect("diagram");
    for station in &diagram.elements[0].stations {
        assert_close(station.moment, moment, 1e-12);
        assert_close(station.shear, 0.0, 1e-12);
    }
}

#[test]
fn gradient_without_alpha_or_section_is_rejected() {
    let mut truss = Model::new(Units::Millimeter);
    truss.add_node("n1", 0.0, 0.0).expect("n1");
    truss.add_node("n2", L, 0.0).expect("n2");
    truss.add_element("e1", "n1", "n2", None).expect("bar");
    assert_eq!(
        truss.set_element_temperature_gradient("e1", Some(GRADIENT)).expect_err("truss").code,
        "element/gradient-requires-beam"
    );
    assert_eq!(
        truss.set_element_temperature_gradient("missing", Some(GRADIENT)).expect_err("unknown").code,
        "element/unknown-id"
    );

    let mut no_alpha = Model::new(Units::Millimeter);
    let section = Some(Section::Rectangle { width: 100.0, height: DEPTH });
    no_alpha.set_material_data(gradient_material(None, section)).expect("material");
    no_alpha.add_node("n1", 0.0, 0.0).expect("n1");
    no_alpha.add_node("n2", L, 0.0).expect("n2");
    no_alpha.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    no_alpha.set_element_temperature_gradient("b1", Some(GRADIENT)).expect("gradient");
    no_alpha.add_support_full("n1", true, true, false, true).expect("fixed");
    assert_eq!(solve(&no_alpha).expect_err("missing alpha").code, "material/missing-alpha");

    let mut no_section = Model::new(Units::Millimeter);
    no_section.set_material_data(gradient_material(Some(ALPHA), None)).expect("material");
    no_section.add_node("n1", 0.0, 0.0).expect("n1");
    no_section.add_node("n2", L, 0.0).expect("n2");
    no_section.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, Some(I), None).expect("beam");
    no_section.set_element_temperature_gradient("b1", Some(GRADIENT)).expect("gradient");
    no_section.add_support_full("n1", true, true, false, true).expect("fixed");
    assert_eq!(solve(&no_section).expect_err("missing section").code, "material/missing-section");
    assert!(
        no_section.validate().iter().any(|issue| issue.code == "material/missing-section" && issue.severity == Severity::Error)
    );
}
