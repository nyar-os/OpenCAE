use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, Material, Model, OpenCae, OpenCaeOptions, Section, Units, library_json, material_preset, solve};

const E: f64 = 210_000.0;
const L: f64 = 1000.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-library-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

#[test]
fn rectangular_section_properties_match_closed_form() {
    let section = Section::rectangle(10.0, 20.0).expect("rectangle");
    let properties = section.properties().expect("properties");

    assert_close(properties.area, 200.0, 1e-12);
    assert_close(properties.inertia_strong, 10.0 * 20.0_f64.powi(3) / 12.0, 1e-12);
    assert_close(properties.inertia_weak, 20.0 * 10.0_f64.powi(3) / 12.0, 1e-12);
    assert_close(properties.extreme_fiber, 10.0, 1e-12);
    assert_close(properties.section_modulus(), properties.inertia_strong / 10.0, 1e-12);

    let large: f64 = 20.0;
    let small: f64 = 10.0;
    let ratio = small / large;
    let torsion = large * small.powi(3) / 3.0 * (1.0 - 0.63 * ratio + 0.052 * ratio.powi(5));
    assert_close(properties.torsion_constant, torsion, 1e-12);
}

#[test]
fn circular_section_properties_match_closed_form() {
    let diameter = 10.0;
    let section = Section::circle(diameter).expect("circle");
    let properties = section.properties().expect("properties");
    let pi = std::f64::consts::PI;

    assert_close(properties.area, pi * diameter.powi(2) / 4.0, 1e-12);
    assert_close(properties.inertia_strong, pi * diameter.powi(4) / 64.0, 1e-12);
    assert_close(properties.inertia_weak, pi * diameter.powi(4) / 64.0, 1e-12);
    assert_close(properties.torsion_constant, pi * diameter.powi(4) / 32.0, 1e-12);
    assert_close(properties.extreme_fiber, diameter / 2.0, 1e-12);
}

#[test]
fn i_section_properties_match_parallel_axis_formula() {
    let depth = 200.0;
    let flange_width = 100.0;
    let flange_thickness = 10.0;
    let web_thickness = 6.0;
    let section = Section::i_section(depth, flange_width, flange_thickness, web_thickness).expect("i-section");
    let properties = section.properties().expect("properties");
    let web_height = depth - 2.0 * flange_thickness;

    let expected_area = 2.0 * flange_width * flange_thickness + web_height * web_thickness;
    let expected_inertia = (flange_width * depth.powi(3) - (flange_width - web_thickness) * web_height.powi(3)) / 12.0;
    assert_close(properties.area, expected_area, 1e-12);
    assert_close(properties.inertia_strong, expected_inertia, 1e-12);
    assert_close(properties.extreme_fiber, depth / 2.0, 1e-12);
}

#[test]
fn material_presets_convert_between_unit_systems() {
    let steel_mm = material_preset("steel").expect("steel");
    let steel_m = material_preset("steel").expect("steel");
    assert_close(steel_mm.youngs_modulus(Units::Millimeter), 210_000.0, 1e-12);
    assert_close(steel_mm.density(Units::Millimeter), 7.85e-9, 1e-12);
    assert_close(steel_m.youngs_modulus(Units::Meter), 2.1e11, 1e-12);
    assert_close(steel_m.density(Units::Meter), 7_850.0, 1e-12);
    assert!(steel_mm.allowable_stress(Units::Millimeter).is_some_and(|value| value > 0.0));

    let concrete = material_preset("concrete").expect("concrete");
    assert!(concrete.allowable_stress(Units::Millimeter).is_none());

    let error = material_preset("unobtainium").expect_err("unknown preset");
    assert_eq!(error.code, "material/unknown-preset");
}

#[test]
fn library_json_reports_presets_and_shapes_in_the_requested_units() {
    let millimetres = library_json(Units::Millimeter).expect("library");
    assert!(millimetres.contains("\"name\": \"steel\""));
    assert!(millimetres.contains("\"youngsModulus\": 210000.0"));
    assert!(millimetres.contains("\"shape\": \"rectangle\""));
    assert!(millimetres.contains("\"shape\": \"i\""));

    let metres = library_json(Units::Meter).expect("library");
    assert!(metres.contains("\"youngsModulus\": 210000000000.0"));
    assert!(metres.contains("\"density\": 7850.0"));
}

fn cantilever_with_section() -> Model {
    let section = Section::rectangle(10.0, 20.0).expect("section");
    let properties = section.properties().expect("properties");
    let mut model = Model::new(Units::Millimeter);
    model
        .set_material_data(Material {
            name: "steel".to_string(),
            youngs_modulus: E,
            area: properties.area,
            density: 7.85e-9,
            allowable_stress: Some(235.0),
            section: Some(section),
        })
        .expect("material");
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, None, None).expect("beam");
    model.add_support_full("n1", true, true, false, true).expect("fixed");
    model.add_load("n2", None, Some(-1000.0)).expect("tip load");
    model
}

#[test]
fn beam_without_inertia_uses_the_material_section() {
    let model = cantilever_with_section();
    let section = Section::rectangle(10.0, 20.0).expect("section").properties().expect("properties");
    let solution = solve(&model).expect("solve");
    let tip = solution.nodes.iter().find(|node| node.id == "n2").expect("tip");
    assert_close(tip.uy, -1000.0 * L.powi(3) / (3.0 * E * section.inertia_strong), 1e-9);
}

#[test]
fn session_applies_a_preset_and_round_trips_the_section() {
    let directory = temporary_dir("preset");
    let path = directory.join("model.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions::default()).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    let section = Section::rectangle(10.0, 20.0).expect("section");
    let summary = session.apply_material_preset("aluminum", None, Some(section), Some(150.0)).expect("preset");

    assert_close(summary.material.youngs_modulus, 70_000.0, 1e-12);
    assert_close(summary.material.area, 200.0, 1e-12);
    assert_eq!(summary.material.section, Some(section));
    assert_eq!(summary.material.allowable_stress, Some(150.0));

    let written = fs::read_to_string(&path).expect("read document");
    assert!(written.contains("\"shape\": \"rectangle\""), "document: {written}");
    assert!(written.contains("\"allowableStress\": 150.0"), "document: {written}");

    // A beam that omits `inertia` now solves from the stored section.
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, None, None).expect("beam");
    session.add_support_full("n1", true, true, false, true).expect("fixed");
    session.add_load("n2", None, Some(-1000.0), None).expect("load");
    let validation = session.validate().expect("validate");
    assert!(validation.ok, "unexpected issues: {:?}", validation.issues);
    let solution = session.solve(None).expect("solve");
    assert!(solution.nodes.iter().find(|node| node.id == "n2").expect("tip").uy < 0.0);
}

#[test]
fn invalid_sections_and_allowable_stress_are_rejected() {
    let error = Section::rectangle(0.0, 20.0).expect_err("zero width");
    assert_eq!(error.code, "material/invalid-section");
    let error = Section::circle(f64::NAN).expect_err("non-finite diameter");
    assert_eq!(error.code, "material/invalid-section");
    let error = Section::i_section(20.0, 100.0, 10.0, 6.0).expect_err("impossible flange");
    assert_eq!(error.code, "material/invalid-section");

    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    let error = model
        .set_material_data(Material {
            name: "steel".to_string(),
            youngs_modulus: E,
            area: 100.0,
            density: 0.0,
            allowable_stress: Some(-1.0),
            section: None,
        })
        .expect_err("negative allowable stress");
    assert_eq!(error.code, "material/invalid-allowable-stress");

    let error = model
        .add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, None, None)
        .expect_err("beam needs inertia or a section");
    assert_eq!(error.code, "element/invalid-inertia");
}
