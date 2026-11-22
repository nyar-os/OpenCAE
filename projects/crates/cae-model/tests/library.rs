use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{
    ElementKind, Material, Model, OpenCae, OpenCaeOptions, Section, Units, angle_section_report,
    channel_standard_section_report, ellipse_section_report, i_standard_section_report, library_json, material_preset,
    polygon_section_report, principal_section_report, section_report, solve, t_section_report, triangle_section_report,
    tube_standard_section_report, z_section_report,
};

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
fn box_section_properties_match_closed_form() {
    let width = 100.0;
    let height = 200.0;
    let wall = 5.0;
    let section = Section::box_section(width, height, wall).expect("box");
    let properties = section.properties().expect("properties");
    let inner_width = width - 2.0 * wall;
    let inner_height = height - 2.0 * wall;

    assert_close(properties.area, width * height - inner_width * inner_height, 1e-12);
    assert_close(properties.inertia_strong, (width * height.powi(3) - inner_width * inner_height.powi(3)) / 12.0, 1e-12);
    assert_close(properties.inertia_weak, (height * width.powi(3) - inner_height * inner_width.powi(3)) / 12.0, 1e-12);
    assert_close(
        properties.torsion_constant,
        2.0 * wall * (width - wall).powi(2) * (height - wall).powi(2) / (width + height - 2.0 * wall),
        1e-12,
    );
    assert_close(properties.extreme_fiber, height / 2.0, 1e-12);
    assert_close(properties.section_modulus(), properties.inertia_strong / (height / 2.0), 1e-12);
}

#[test]
fn tube_section_properties_match_closed_form() {
    let outer_diameter = 100.0;
    let inner_diameter = 80.0;
    let section = Section::tube(outer_diameter, inner_diameter).expect("tube");
    let properties = section.properties().expect("properties");
    let pi = std::f64::consts::PI;
    let difference = outer_diameter.powi(4) - inner_diameter.powi(4);

    assert_close(properties.area, pi / 4.0 * (outer_diameter.powi(2) - inner_diameter.powi(2)), 1e-12);
    assert_close(properties.inertia_strong, pi / 64.0 * difference, 1e-12);
    assert_close(properties.inertia_weak, pi / 64.0 * difference, 1e-12);
    assert_close(properties.torsion_constant, pi / 32.0 * difference, 1e-12);
    assert_close(properties.extreme_fiber, outer_diameter / 2.0, 1e-12);
    assert_close(properties.section_modulus(), properties.inertia_strong / (outer_diameter / 2.0), 1e-12);
}

#[test]
fn channel_section_properties_match_closed_form() {
    let section = Section::channel(200.0, 80.0, 10.0, 6.0).expect("channel");
    let properties = section.properties().expect("properties");

    // h = 200, b = 80, tf = 10, tw = 6, web height = 180.
    let expected_area = 2.0 * 80.0 * 10.0 + 180.0 * 6.0;
    let centroid = (80.0 * 10.0 * 80.0 + 180.0 * 6.0f64.powi(2) / 2.0) / expected_area;
    let expected_strong = 2.0 * (80.0 * 10.0f64.powi(3) / 12.0 + 80.0 * 10.0 * 95.0f64.powi(2)) + 6.0 * 180.0f64.powi(3) / 12.0;
    let expected_weak = 2.0 * (10.0 * 80.0f64.powi(3) / 12.0 + 80.0 * 10.0 * (40.0 - centroid).powi(2))
        + (180.0 * 6.0f64.powi(3) / 12.0 + 180.0 * 6.0 * (centroid - 3.0f64).powi(2));
    let expected_torsion = (2.0 * 80.0 * 10.0f64.powi(3) + 180.0 * 6.0f64.powi(3)) / 3.0;

    assert_close(properties.area, expected_area, 1e-12);
    assert_close(properties.inertia_strong, expected_strong, 1e-12);
    assert_close(properties.inertia_weak, expected_weak, 1e-12);
    assert_close(properties.torsion_constant, expected_torsion, 1e-12);
    assert_close(properties.extreme_fiber, 100.0, 1e-12);
    assert_close(properties.section_modulus(), expected_strong / 100.0, 1e-12);

    // Hand-evaluated values: A = 2680 mm², x̄ ≈ 25.0896 mm from the web face.
    assert_close(properties.area, 2680.0, 1e-12);
    assert_close(centroid, 1681.0 / 67.0, 1e-12);
    assert_close(properties.inertia_strong, 17_369_333.333_333_334, 1e-12);
}

#[test]
fn channel_section_round_trips_through_serde() {
    let channel = Section::channel(200.0, 80.0, 10.0, 6.0).expect("channel");
    let json = serde_json::to_string(&channel).expect("serialize channel");
    assert!(json.contains("\"shape\":\"channel\""));
    assert!(json.contains("\"flangeWidth\":80.0"));
    assert!(json.contains("\"webThickness\":6.0"));
    let parsed: Section = serde_json::from_str(&json).expect("deserialize channel");
    assert_eq!(parsed, channel);
}

#[test]
fn invalid_channel_geometry_is_rejected() {
    for section in [
        Section::channel(200.0, 80.0, 10.0, 0.0),
        Section::channel(20.0, 80.0, 10.0, 6.0),
        Section::channel(200.0, 6.0, 10.0, 6.0),
        Section::channel(0.0, 80.0, 10.0, 6.0),
        Section::channel(200.0, 80.0, f64::NAN, 6.0),
        Section::channel(200.0, f64::INFINITY, 10.0, 6.0),
    ] {
        let error = section.expect_err("invalid section");
        assert_eq!(error.code, "material/invalid-section");
    }
}

#[test]
fn box_and_tube_sections_round_trip_through_serde() {
    let box_section = Section::box_section(100.0, 200.0, 5.0).expect("box");
    let box_json = serde_json::to_string(&box_section).expect("serialize box");
    assert!(box_json.contains("\"shape\":\"box\""));
    assert!(box_json.contains("\"wall\":5.0"));
    let parsed_box: Section = serde_json::from_str(&box_json).expect("deserialize box");
    assert_eq!(parsed_box, box_section);

    let tube = Section::tube(100.0, 80.0).expect("tube");
    let tube_json = serde_json::to_string(&tube).expect("serialize tube");
    assert!(tube_json.contains("\"shape\":\"tube\""));
    assert!(tube_json.contains("\"outerDiameter\":100.0"));
    assert!(tube_json.contains("\"innerDiameter\":80.0"));
    let parsed_tube: Section = serde_json::from_str(&tube_json).expect("deserialize tube");
    assert_eq!(parsed_tube, tube);
}

#[test]
fn invalid_box_and_tube_geometry_is_rejected() {
    for section in [
        Section::box_section(100.0, 200.0, 0.0),
        Section::box_section(100.0, 200.0, 50.0),
        Section::box_section(100.0, 200.0, 101.0),
        Section::box_section(0.0, 200.0, 5.0),
        Section::box_section(100.0, f64::NAN, 5.0),
        Section::tube(100.0, 100.0),
        Section::tube(100.0, 120.0),
        Section::tube(f64::INFINITY, 80.0),
        Section::tube(100.0, 0.0),
    ] {
        let error = section.expect_err("invalid section");
        assert_eq!(error.code, "material/invalid-section");
    }
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
    assert!(millimetres.contains("\"shape\": \"box\""));
    assert!(millimetres.contains("\"shape\": \"tube\""));
    assert!(millimetres.contains("\"shape\": \"channel\""));

    let metres = library_json(Units::Meter).expect("library");
    assert!(metres.contains("\"youngsModulus\": 210000000000.0"));
    assert!(metres.contains("\"density\": 7850.0"));
}

#[test]
fn section_report_reports_modulus_and_radii_of_gyration() {
    let section = Section::rectangle(10.0, 20.0).expect("rectangle");
    let report = section_report(section, Units::Millimeter).expect("report");

    assert_eq!(report.shape, "rectangle");
    assert_eq!(report.units, "mm");
    assert_close(report.area, 200.0, 1e-12);
    assert_close(report.inertia_strong, section.properties().expect("properties").inertia_strong, 1e-12);
    assert_close(report.section_modulus, report.inertia_strong / report.extreme_fiber, 1e-12);
    assert_close(report.radius_of_gyration_strong, (report.inertia_strong / report.area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_weak, (report.inertia_weak / report.area).sqrt(), 1e-12);
    assert!(report.radius_of_gyration_strong > report.radius_of_gyration_weak);
}

#[test]
fn section_report_serialises_camel_case_and_labels_the_units() {
    let report = section_report(Section::circle(50.0).expect("circle"), Units::Meter).expect("report");
    let json = serde_json::to_value(&report).expect("json");

    assert_eq!(json["shape"], "circle");
    assert_eq!(json["units"], "m");
    assert!(json["sectionModulus"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json["radiusOfGyrationStrong"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json.get("radius_of_gyration_strong").is_none());
}

#[test]
fn section_report_rejects_invalid_geometry() {
    let error = section_report(Section::Rectangle { width: 0.0, height: 20.0 }, Units::Millimeter).expect_err("invalid width");
    assert_eq!(error.code, "material/invalid-section");
}

#[test]
fn polygon_rectangle_matches_closed_form() {
    let vertices = [(0.0, 0.0), (100.0, 0.0), (100.0, 50.0), (0.0, 50.0)];
    let report = polygon_section_report(&vertices, Units::Millimeter).expect("polygon rectangle");

    let area = 100.0 * 50.0;
    let ix = 100.0 * 50.0_f64.powi(3) / 12.0;
    let iy = 50.0 * 100.0_f64.powi(3) / 12.0;

    assert_eq!(report.shape, "polygon");
    assert_eq!(report.units, "mm");
    assert_close(report.area, area, 1e-12);
    assert_close(report.centroid_x, 50.0, 1e-12);
    assert_close(report.centroid_y, 25.0, 1e-12);
    assert_close(report.ix, ix, 1e-12);
    assert_close(report.iy, iy, 1e-12);
    assert!(report.ixy.abs() < 1e-9 * ix, "rectangle product of inertia should vanish, got {}", report.ixy);
    assert_close(report.polar_moment, ix + iy, 1e-12);
    assert_close(report.radius_of_gyration_x, (ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (iy / area).sqrt(), 1e-12);
}

#[test]
fn polygon_triangle_matches_closed_form() {
    let vertices = [(0.0, 0.0), (100.0, 0.0), (0.0, 50.0)];
    let report = polygon_section_report(&vertices, Units::Millimeter).expect("polygon triangle");

    let base: f64 = 100.0;
    let height: f64 = 50.0;
    let area = base * height / 2.0;
    let ix = base * height.powi(3) / 36.0;
    let iy = height * base.powi(3) / 36.0;
    let ixy = -base.powi(2) * height.powi(2) / 72.0;

    assert_close(report.area, area, 1e-12);
    assert_close(report.centroid_x, base / 3.0, 1e-12);
    assert_close(report.centroid_y, height / 3.0, 1e-12);
    assert_close(report.ix, ix, 1e-12);
    assert_close(report.iy, iy, 1e-12);
    assert_close(report.ixy, ixy, 1e-12);
    assert_close(report.polar_moment, ix + iy, 1e-12);
    assert_close(report.radius_of_gyration_x, (ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (iy / area).sqrt(), 1e-12);
}

#[test]
fn polygon_winding_and_closing_vertex_do_not_change_the_result() {
    let counter_clockwise = [(0.0, 0.0), (100.0, 0.0), (0.0, 50.0)];
    let clockwise = [(0.0, 50.0), (100.0, 0.0), (0.0, 0.0)];
    let closed = [(0.0, 0.0), (100.0, 0.0), (0.0, 50.0), (0.0, 0.0)];

    let reference = polygon_section_report(&counter_clockwise, Units::Millimeter).expect("counter-clockwise");
    assert_eq!(polygon_section_report(&clockwise, Units::Millimeter).expect("clockwise"), reference);
    assert_eq!(polygon_section_report(&closed, Units::Millimeter).expect("closed"), reference);
    assert!(reference.ixy < 0.0);
}

#[test]
fn polygon_section_report_serialises_camel_case() {
    let vertices = [(0.0, 0.0), (100.0, 0.0), (100.0, 50.0), (0.0, 50.0)];
    let report = polygon_section_report(&vertices, Units::Millimeter).expect("polygon");
    let json = serde_json::to_value(&report).expect("json");

    assert_eq!(json["shape"], "polygon");
    assert_eq!(json["units"], "mm");
    assert!(json["centroidX"].as_f64().is_some_and(|value| (value - 50.0).abs() < 1e-12));
    assert!(json["ix"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json["polarMoment"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json.get("radiusOfGyrationX").is_some());
    assert!(json.get("centroid_x").is_none());
}

#[test]
fn polygon_section_rejects_invalid_vertex_lists() {
    let error = polygon_section_report(&[(0.0, 0.0), (100.0, 0.0)], Units::Millimeter).expect_err("two vertices");
    assert_eq!(error.code, "section/polygon-too-few-vertices");

    let collinear = [(0.0, 0.0), (50.0, 0.0), (100.0, 0.0)];
    let error = polygon_section_report(&collinear, Units::Millimeter).expect_err("collinear");
    assert_eq!(error.code, "section/polygon-degenerate");

    let crossed = [(0.0, 0.0), (100.0, 100.0), (0.0, 100.0), (100.0, 0.0)];
    let error = polygon_section_report(&crossed, Units::Millimeter).expect_err("self-intersection");
    assert_eq!(error.code, "section/polygon-self-intersecting");

    let repeated = [(10.0, 10.0), (10.0, 10.0), (10.0, 10.0)];
    let error = polygon_section_report(&repeated, Units::Millimeter).expect_err("no distinct vertices");
    assert_eq!(error.code, "section/polygon-degenerate");

    let non_finite = [(0.0, 0.0), (f64::NAN, 0.0), (0.0, 10.0)];
    let error = polygon_section_report(&non_finite, Units::Millimeter).expect_err("non-finite");
    assert_eq!(error.code, "section/invalid-polygon");
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
            alpha: None,
            conductivity: None,
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
            alpha: None,
            conductivity: None,
            section: None,
        })
        .expect_err("negative allowable stress");
    assert_eq!(error.code, "material/invalid-allowable-stress");

    let error = model
        .add_element_kind("b1", "n1", "n2", None, ElementKind::Beam, None, None)
        .expect_err("beam needs inertia or a section");
    assert_eq!(error.code, "element/invalid-inertia");

    let mut with_section = cantilever_with_section();
    let error = with_section
        .add_element_kind("b2", "n1", "n2", None, ElementKind::Beam, Some(-1.0), None)
        .expect_err("an explicit negative inertia is always invalid");
    assert_eq!(error.code, "element/invalid-inertia");
}

#[test]
fn standard_i_section_matches_closed_form() {
    let height: f64 = 200.0;
    let flange_width: f64 = 100.0;
    let web_thickness: f64 = 6.0;
    let flange_thickness: f64 = 10.0;
    let report =
        i_standard_section_report(height, flange_width, web_thickness, flange_thickness, Units::Millimeter).expect("i section");

    let web_height = height - 2.0 * flange_thickness;
    let area = 2.0 * flange_width * flange_thickness + web_height * web_thickness;
    let ix = (flange_width * height.powi(3) - (flange_width - web_thickness) * web_height.powi(3)) / 12.0;
    let iy = (2.0 * flange_thickness * flange_width.powi(3) + web_height * web_thickness.powi(3)) / 12.0;
    let torsion = (2.0 * flange_width * flange_thickness.powi(3) + web_height * web_thickness.powi(3)) / 3.0;

    assert_eq!(report.shape, "i");
    assert_eq!(report.units, "mm");
    assert_close(report.area, area, 1e-12);
    assert_close(report.centroid_x, flange_width / 2.0, 1e-12);
    assert_close(report.centroid_y, height / 2.0, 1e-12);
    assert_close(report.ix, ix, 1e-12);
    assert_close(report.iy, iy, 1e-12);
    assert_close(report.ixy, 0.0, 1e-12);
    assert_close(report.polar_moment, torsion, 1e-12);
    assert_close(report.radius_of_gyration_x, (ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (iy / area).sqrt(), 1e-12);
    assert!(report.ix > report.iy);
}

#[test]
fn standard_tube_section_matches_closed_form() {
    let height: f64 = 200.0;
    let width: f64 = 100.0;
    let thickness: f64 = 5.0;
    let report = tube_standard_section_report(height, width, thickness, Units::Millimeter).expect("rectangular tube");

    let inner_width = width - 2.0 * thickness;
    let inner_height = height - 2.0 * thickness;
    let area = width * height - inner_width * inner_height;
    let ix = (width * height.powi(3) - inner_width * inner_height.powi(3)) / 12.0;
    let iy = (height * width.powi(3) - inner_height * inner_width.powi(3)) / 12.0;
    let torsion =
        2.0 * thickness * (width - thickness).powi(2) * (height - thickness).powi(2) / (width + height - 2.0 * thickness);

    assert_eq!(report.shape, "tube");
    assert_close(report.area, area, 1e-12);
    assert_close(report.centroid_x, width / 2.0, 1e-12);
    assert_close(report.centroid_y, height / 2.0, 1e-12);
    assert_close(report.ix, ix, 1e-12);
    assert_close(report.iy, iy, 1e-12);
    assert_close(report.ixy, 0.0, 1e-12);
    assert_close(report.polar_moment, torsion, 1e-12);
    assert_close(report.radius_of_gyration_x, (ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (iy / area).sqrt(), 1e-12);
}

#[test]
fn standard_channel_section_matches_parallel_axis_formula() {
    let height: f64 = 200.0;
    let flange_width: f64 = 80.0;
    let web_thickness: f64 = 6.0;
    let flange_thickness: f64 = 10.0;
    let report = channel_standard_section_report(height, flange_width, web_thickness, flange_thickness, Units::Millimeter)
        .expect("channel");

    let web_height = height - 2.0 * flange_thickness;
    let area = 2.0 * flange_width * flange_thickness + web_height * web_thickness;
    let centroid_x = (2.0 * flange_width * flange_thickness * (flange_width / 2.0)
        + web_height * web_thickness * (web_thickness / 2.0))
        / area;
    let flange_offset = height / 2.0 - flange_thickness / 2.0;
    let ix = 2.0 * (flange_width * flange_thickness.powi(3) / 12.0 + flange_width * flange_thickness * flange_offset.powi(2))
        + web_thickness * web_height.powi(3) / 12.0;
    let iy = 2.0
        * (flange_thickness * flange_width.powi(3) / 12.0
            + flange_width * flange_thickness * (flange_width / 2.0 - centroid_x).powi(2))
        + (web_height * web_thickness.powi(3) / 12.0 + web_height * web_thickness * (centroid_x - web_thickness / 2.0).powi(2));
    let torsion = (2.0 * flange_width * flange_thickness.powi(3) + web_height * web_thickness.powi(3)) / 3.0;

    assert_eq!(report.shape, "channel");
    assert_close(report.area, area, 1e-12);
    assert_close(report.area, 2680.0, 1e-12);
    assert_close(report.centroid_x, centroid_x, 1e-12);
    assert_close(report.centroid_x, 1681.0 / 67.0, 1e-12);
    assert!(report.centroid_x > 0.0 && report.centroid_x < flange_width / 2.0);
    assert_close(report.centroid_y, height / 2.0, 1e-12);
    assert_close(report.ix, ix, 1e-12);
    assert_close(report.iy, iy, 1e-12);
    assert_close(report.ixy, 0.0, 1e-12);
    assert_close(report.polar_moment, torsion, 1e-12);
    assert_close(report.radius_of_gyration_x, (ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (iy / area).sqrt(), 1e-12);

    // The standalone calculator agrees with the parametric channel section.
    let parametric = Section::channel(height, flange_width, flange_thickness, web_thickness)
        .expect("parametric channel")
        .properties()
        .expect("properties");
    assert_close(report.area, parametric.area, 1e-12);
    assert_close(report.ix, parametric.inertia_strong, 1e-12);
    assert_close(report.iy, parametric.inertia_weak, 1e-12);
    assert_close(report.polar_moment, parametric.torsion_constant, 1e-12);
}

#[test]
fn standard_section_reports_are_deterministic_and_serialise_camel_case() {
    let first = i_standard_section_report(200.0, 100.0, 6.0, 10.0, Units::Meter).expect("i section");
    let second = i_standard_section_report(200.0, 100.0, 6.0, 10.0, Units::Meter).expect("i section");
    assert_eq!(first, second);

    let json = serde_json::to_value(&first).expect("json");
    assert_eq!(json["shape"], "i");
    assert_eq!(json["units"], "m");
    assert!(json["centroidX"].as_f64().is_some_and(|value| (value - 50.0).abs() < 1e-12));
    assert!(json["polarMoment"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json.get("radiusOfGyrationX").is_some());
    assert!(json.get("centroid_x").is_none());
}

#[test]
fn standard_sections_reject_invalid_dimensions() {
    for result in [
        i_standard_section_report(200.0, 100.0, 6.0, 100.0, Units::Millimeter),
        i_standard_section_report(200.0, 100.0, 6.0, 0.0, Units::Millimeter),
        i_standard_section_report(200.0, 100.0, 100.0, 10.0, Units::Millimeter),
        i_standard_section_report(f64::NAN, 100.0, 6.0, 10.0, Units::Millimeter),
        i_standard_section_report(200.0, f64::INFINITY, 6.0, 10.0, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid i-section dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }

    for result in [
        tube_standard_section_report(200.0, 100.0, 50.0, Units::Millimeter),
        tube_standard_section_report(200.0, 100.0, 0.0, Units::Millimeter),
        tube_standard_section_report(200.0, -100.0, 5.0, Units::Millimeter),
        tube_standard_section_report(200.0, 100.0, f64::NAN, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid rectangular tube dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }

    for result in [
        channel_standard_section_report(200.0, 80.0, 6.0, 100.0, Units::Millimeter),
        channel_standard_section_report(200.0, 80.0, 6.0, 0.0, Units::Millimeter),
        channel_standard_section_report(200.0, 80.0, 80.0, 10.0, Units::Millimeter),
        channel_standard_section_report(200.0, 80.0, f64::NAN, 10.0, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid channel dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }
}

#[test]
fn angle_section_matches_parallel_axis_formula_and_polygon() {
    let height: f64 = 150.0;
    let width: f64 = 100.0;
    let thickness: f64 = 10.0;
    let report = angle_section_report(height, width, thickness, Units::Millimeter).expect("angle section");

    // Horizontal leg w × t and vertical leg t × (h − t); the t × t corner is
    // counted once. Hand-evaluated: A = 2400, x̄ = 23.75, ȳ = 48.75.
    let area = thickness * (width + height - thickness);
    let centroid_x = (width * thickness * (width / 2.0) + thickness * (height - thickness) * (thickness / 2.0)) / area;
    let centroid_y = (width * thickness * (thickness / 2.0)
        + thickness * (height - thickness) * (thickness + (height - thickness) / 2.0))
        / area;
    let expected_ix = width * thickness.powi(3) / 12.0
        + width * thickness * (thickness / 2.0).powi(2)
        + thickness * (height - thickness).powi(3) / 12.0
        + thickness * (height - thickness) * (height + thickness).powi(2) / 4.0
        - area * centroid_y.powi(2);
    let expected_iy = thickness * width.powi(3) / 12.0
        + width * thickness * (width / 2.0).powi(2)
        + (height - thickness) * thickness.powi(3) / 12.0
        + thickness * (height - thickness) * (thickness / 2.0).powi(2)
        - area * centroid_x.powi(2);
    let expected_ixy = width * thickness * (width / 2.0) * (thickness / 2.0)
        + thickness * (height - thickness) * (thickness / 2.0) * ((height + thickness) / 2.0)
        - area * centroid_x * centroid_y;
    let expected_torsion = (width * thickness.powi(3) + (height - thickness) * thickness.powi(3)) / 3.0;

    assert_eq!(report.shape, "angle");
    assert_eq!(report.units, "mm");
    assert_close(report.area, 2400.0, 1e-12);
    assert_close(report.centroid_x, 23.75, 1e-12);
    assert_close(report.centroid_y, 48.75, 1e-12);
    assert_close(report.ix, 5_576_250.0, 1e-12);
    assert_close(report.iy, 2_026_250.0, 1e-12);
    assert_close(report.ixy, -1_968_750.0, 1e-12);
    assert_close(report.polar_moment, 80_000.0, 1e-12);
    assert_close(report.radius_of_gyration_x, (report.ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (report.iy / area).sqrt(), 1e-12);
    assert_close(report.area, area, 1e-12);
    assert_close(report.centroid_x, centroid_x, 1e-12);
    assert_close(report.centroid_y, centroid_y, 1e-12);
    assert_close(report.ix, expected_ix, 1e-12);
    assert_close(report.iy, expected_iy, 1e-12);
    assert_close(report.ixy, expected_ixy, 1e-12);
    assert_close(report.polar_moment, expected_torsion, 1e-12);
    assert!(report.ix > report.iy);
    assert!(report.ixy < 0.0);

    // The closed polygon of the same outline must agree to round-off.
    let vertices = [(0.0, 0.0), (width, 0.0), (width, thickness), (thickness, thickness), (thickness, height), (0.0, height)];
    let polygon = polygon_section_report(&vertices, Units::Millimeter).expect("angle polygon");
    assert_close(polygon.area, report.area, 1e-12);
    assert_close(polygon.centroid_x, report.centroid_x, 1e-12);
    assert_close(polygon.centroid_y, report.centroid_y, 1e-12);
    assert_close(polygon.ix, report.ix, 1e-12);
    assert_close(polygon.iy, report.iy, 1e-12);
    assert_close(polygon.ixy, report.ixy, 1e-12);
}

#[test]
fn t_section_matches_rectangle_decomposition_and_polygon() {
    let height: f64 = 100.0;
    let width: f64 = 80.0;
    let web_thickness: f64 = 6.0;
    let flange_thickness: f64 = 10.0;
    let report = t_section_report(height, width, web_thickness, flange_thickness, Units::Millimeter).expect("t section");

    let web_height = height - flange_thickness;
    let area = width * flange_thickness + web_thickness * web_height;
    let centroid_y =
        (web_thickness * web_height * (web_height / 2.0) + width * flange_thickness * (height - flange_thickness / 2.0)) / area;
    let expected_ix = web_thickness * web_height.powi(3) / 12.0
        + web_thickness * web_height * (web_height / 2.0 - centroid_y).powi(2)
        + width * flange_thickness.powi(3) / 12.0
        + width * flange_thickness * (height - flange_thickness / 2.0 - centroid_y).powi(2);
    let expected_iy = flange_thickness * width.powi(3) / 12.0 + web_height * web_thickness.powi(3) / 12.0;
    let expected_torsion = (width * flange_thickness.powi(3) + web_height * web_thickness.powi(3)) / 3.0;

    assert_eq!(report.shape, "t");
    assert_eq!(report.units, "mm");
    assert_close(report.area, 1340.0, 1e-12);
    assert_close(report.centroid_x, width / 2.0, 1e-12);
    assert_close(report.centroid_y, 5015.0 / 67.0, 1e-12);
    assert_close(report.ix, expected_ix, 1e-12);
    assert_close(report.iy, expected_iy, 1e-12);
    assert_close(report.ixy, 0.0, 1e-12);
    assert_close(report.polar_moment, expected_torsion, 1e-12);
    assert_close(report.radius_of_gyration_x, (report.ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (report.iy / area).sqrt(), 1e-12);
    assert_close(report.area, area, 1e-12);

    // The closed polygon of the same outline must agree to round-off.
    let half_web = web_thickness / 2.0;
    let vertices = [
        (0.0, web_height),
        (0.0, height),
        (width, height),
        (width, web_height),
        (width / 2.0 + half_web, web_height),
        (width / 2.0 + half_web, 0.0),
        (width / 2.0 - half_web, 0.0),
        (width / 2.0 - half_web, web_height),
    ];
    let polygon = polygon_section_report(&vertices, Units::Millimeter).expect("t polygon");
    assert_close(polygon.area, report.area, 1e-12);
    assert_close(polygon.centroid_x, report.centroid_x, 1e-12);
    assert_close(polygon.centroid_y, report.centroid_y, 1e-12);
    assert_close(polygon.ix, report.ix, 1e-12);
    assert_close(polygon.iy, report.iy, 1e-12);
    assert!(polygon.ixy.abs() < 1e-9 * polygon.ix, "symmetric T-section product of inertia should vanish");
}

#[test]
fn angle_and_t_sections_reject_invalid_dimensions() {
    for result in [
        angle_section_report(150.0, 100.0, 0.0, Units::Millimeter),
        angle_section_report(150.0, 100.0, 100.0, Units::Millimeter),
        angle_section_report(150.0, 100.0, 150.0, Units::Millimeter),
        angle_section_report(0.0, 100.0, 10.0, Units::Millimeter),
        angle_section_report(f64::NAN, 100.0, 10.0, Units::Millimeter),
        angle_section_report(150.0, f64::INFINITY, 10.0, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid angle dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }

    for result in [
        t_section_report(100.0, 80.0, 0.0, 10.0, Units::Millimeter),
        t_section_report(100.0, 80.0, 6.0, 0.0, Units::Millimeter),
        t_section_report(100.0, 80.0, 80.0, 10.0, Units::Millimeter),
        t_section_report(100.0, 80.0, 6.0, 100.0, Units::Millimeter),
        t_section_report(0.0, 80.0, 6.0, 10.0, Units::Millimeter),
        t_section_report(100.0, 80.0, f64::NAN, 10.0, Units::Millimeter),
        t_section_report(100.0, f64::INFINITY, 6.0, 10.0, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid t-section dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }
}

#[test]
fn ellipse_section_matches_closed_form_and_inscribed_polygon() {
    let width: f64 = 100.0;
    let height: f64 = 50.0;
    let report = ellipse_section_report(width, height, Units::Millimeter).expect("ellipse section");

    let a = width / 2.0;
    let b = height / 2.0;
    let pi = std::f64::consts::PI;
    let area = pi * a * b;
    let ix = pi * a * b.powi(3) / 4.0;
    let iy = pi * a.powi(3) * b / 4.0;
    let torsion = pi * a.powi(3) * b.powi(3) / (a.powi(2) + b.powi(2));

    assert_eq!(report.shape, "ellipse");
    assert_eq!(report.units, "mm");
    assert_close(report.area, area, 1e-12);
    assert_close(report.centroid_x, a, 1e-12);
    assert_close(report.centroid_y, b, 1e-12);
    assert_close(report.ix, ix, 1e-12);
    assert_close(report.iy, iy, 1e-12);
    assert_close(report.ixy, 0.0, 1e-12);
    // `polarMoment` carries the exact solid-ellipse torsion constant J, which
    // is not Ix + Iy for a non-circular ellipse.
    assert_close(report.polar_moment, torsion, 1e-12);
    assert!((report.polar_moment - (ix + iy)).abs() > 1e-3 * torsion);
    assert_close(report.radius_of_gyration_x, b / 2.0, 1e-12);
    assert_close(report.radius_of_gyration_y, a / 2.0, 1e-12);
    assert_close(report.radius_of_gyration_x, (ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (iy / area).sqrt(), 1e-12);

    // A circular ellipse reduces to the circle, where J does equal Ix + Iy.
    let circle = ellipse_section_report(20.0, 20.0, Units::Millimeter).expect("circular ellipse");
    assert_close(circle.ix, circle.iy, 1e-12);
    assert_close(circle.polar_moment, circle.ix + circle.iy, 1e-12);

    // Cross-check against a 256-edge inscribed ellipse polygon. The area and
    // inertias of an inscribed polygon converge from below with a relative
    // error of order `1/n²`, so a 1e-3 tolerance absorbs the discretisation.
    let edges = 256;
    let vertices: Vec<(f64, f64)> = (0..edges)
        .map(|index| {
            let angle = 2.0 * pi * index as f64 / edges as f64;
            (a * (1.0 + angle.cos()), b * (1.0 + angle.sin()))
        })
        .collect();
    let polygon = polygon_section_report(&vertices, Units::Millimeter).expect("inscribed ellipse polygon");
    assert_close(polygon.area, area, 1e-3);
    assert_close(polygon.ix, ix, 1e-3);
    assert_close(polygon.iy, iy, 1e-3);
    assert_close(polygon.centroid_x, a, 1e-3);
    assert_close(polygon.centroid_y, b, 1e-3);
}

#[test]
fn ellipse_section_rejects_invalid_dimensions() {
    for result in [
        ellipse_section_report(0.0, 50.0, Units::Millimeter),
        ellipse_section_report(100.0, 0.0, Units::Millimeter),
        ellipse_section_report(-100.0, 50.0, Units::Millimeter),
        ellipse_section_report(100.0, -50.0, Units::Millimeter),
        ellipse_section_report(f64::NAN, 50.0, Units::Millimeter),
        ellipse_section_report(100.0, f64::INFINITY, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid ellipse dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }
}

#[test]
fn ellipse_section_report_is_deterministic_with_cli_field_names() {
    let first = ellipse_section_report(100.0, 50.0, Units::Meter).expect("ellipse");
    let second = ellipse_section_report(100.0, 50.0, Units::Meter).expect("ellipse");
    assert_eq!(first, second);

    let json = serde_json::to_value(&first).expect("json");
    let mut keys: Vec<&str> = json.as_object().expect("object").keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "area",
            "centroidX",
            "centroidY",
            "ix",
            "ixy",
            "iy",
            "polarMoment",
            "radiusOfGyrationX",
            "radiusOfGyrationY",
            "shape",
            "units",
        ]
    );
    assert_eq!(json["shape"], "ellipse");
    assert_eq!(json["units"], "m");
    assert!(json["polarMoment"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json.get("centroid_x").is_none());
}

#[test]
fn angle_and_t_section_reports_are_deterministic_with_cli_field_names() {
    let angle = angle_section_report(150.0, 100.0, 10.0, Units::Meter).expect("angle");
    assert_eq!(angle, angle_section_report(150.0, 100.0, 10.0, Units::Meter).expect("angle"));
    let tee = t_section_report(100.0, 80.0, 6.0, 10.0, Units::Meter).expect("t");
    assert_eq!(tee, t_section_report(100.0, 80.0, 6.0, 10.0, Units::Meter).expect("t"));

    for (report, shape) in [(&angle, "angle"), (&tee, "t")] {
        let json = serde_json::to_value(report).expect("json");
        let mut keys: Vec<&str> = json.as_object().expect("object").keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "area",
                "centroidX",
                "centroidY",
                "ix",
                "ixy",
                "iy",
                "polarMoment",
                "radiusOfGyrationX",
                "radiusOfGyrationY",
                "shape",
                "units",
            ]
        );
        assert_eq!(json["shape"], shape);
        assert_eq!(json["units"], "m");
        assert!(json["polarMoment"].as_f64().is_some_and(|value| value > 0.0));
        assert!(json.get("centroid_x").is_none());
    }
}

#[test]
fn z_section_matches_rectangle_decomposition_and_polygon() {
    let height: f64 = 200.0;
    let width: f64 = 100.0;
    let thickness: f64 = 10.0;
    let report = z_section_report(height, width, thickness, Units::Millimeter).expect("z section");

    // Top and bottom flanges b × t with b = w/2 + t/2, plus the web between
    // them. Hand-evaluated for h = 200, w = 100, t = 10: A = 2900, x̄ = 50,
    // ȳ = 100, Ix = 14 796 666.67, Iy = 849 166.67, Ixy = 2 351 250 and
    // J = 290 000/3.
    let flange_length = width / 2.0 + thickness / 2.0;
    let web_height = height - 2.0 * thickness;
    let area = 2.0 * flange_length * thickness + web_height * thickness;
    let flange_offset_y = (height - thickness) / 2.0;
    let flange_offset_x = (width - thickness) / 4.0;
    let expected_ix = 2.0 * (flange_length * thickness.powi(3) / 12.0 + flange_length * thickness * flange_offset_y.powi(2))
        + thickness * web_height.powi(3) / 12.0;
    let expected_iy = 2.0 * (thickness * flange_length.powi(3) / 12.0 + flange_length * thickness * flange_offset_x.powi(2))
        + web_height * thickness.powi(3) / 12.0;
    let expected_ixy = 2.0 * flange_length * thickness * flange_offset_x * flange_offset_y;
    let expected_torsion = (width + height - thickness) * thickness.powi(3) / 3.0;

    assert_eq!(report.shape, "z");
    assert_eq!(report.units, "mm");
    assert_close(report.area, 2900.0, 1e-12);
    assert_close(report.centroid_x, 50.0, 1e-12);
    assert_close(report.centroid_y, 100.0, 1e-12);
    assert_close(report.ix, 14_796_666.666_666_667, 1e-12);
    assert_close(report.iy, 849_166.666_666_666_6, 1e-12);
    assert_close(report.ixy, 2_351_250.0, 1e-12);
    assert_close(report.polar_moment, 290_000.0 / 3.0, 1e-12);
    assert_close(report.radius_of_gyration_x, (report.ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (report.iy / area).sqrt(), 1e-12);
    assert_close(report.area, area, 1e-12);
    assert_close(report.ix, expected_ix, 1e-12);
    assert_close(report.iy, expected_iy, 1e-12);
    assert_close(report.ixy, expected_ixy, 1e-12);
    assert_close(report.polar_moment, expected_torsion, 1e-12);
    assert!(report.ix > report.iy);
    assert!(report.ixy > 0.0, "the point-symmetric Z-section has a positive product of inertia");

    // The closed polygon of the same outline must agree to round-off.
    let right = width / 2.0 + thickness / 2.0;
    let left = width / 2.0 - thickness / 2.0;
    let vertices = [
        (0.0, 0.0),
        (right, 0.0),
        (right, height - thickness),
        (width, height - thickness),
        (width, height),
        (left, height),
        (left, thickness),
        (0.0, thickness),
    ];
    let polygon = polygon_section_report(&vertices, Units::Millimeter).expect("z polygon");
    assert_close(polygon.area, report.area, 1e-12);
    assert_close(polygon.centroid_x, report.centroid_x, 1e-12);
    assert_close(polygon.centroid_y, report.centroid_y, 1e-12);
    assert_close(polygon.ix, report.ix, 1e-12);
    assert_close(polygon.iy, report.iy, 1e-12);
    assert_close(polygon.ixy, report.ixy, 1e-12);
}

#[test]
fn z_section_rejects_invalid_dimensions() {
    for result in [
        z_section_report(200.0, 100.0, 0.0, Units::Millimeter),
        z_section_report(200.0, 100.0, -10.0, Units::Millimeter),
        z_section_report(200.0, 100.0, 100.0, Units::Millimeter),
        z_section_report(200.0, 100.0, 150.0, Units::Millimeter),
        z_section_report(20.0, 100.0, 10.0, Units::Millimeter),
        z_section_report(0.0, 100.0, 10.0, Units::Millimeter),
        z_section_report(200.0, 0.0, 10.0, Units::Millimeter),
        z_section_report(f64::NAN, 100.0, 10.0, Units::Millimeter),
        z_section_report(200.0, f64::INFINITY, 10.0, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid z-section dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }
}

#[test]
fn triangle_section_matches_closed_form_and_closed_polygon() {
    let side: f64 = 100.0;
    let report = triangle_section_report(side, Units::Millimeter).expect("triangle section");

    let root_three = 3.0_f64.sqrt();
    let height = root_three * side / 2.0;
    let area = root_three * side.powi(2) / 4.0;
    let second_moment = root_three * side.powi(4) / 96.0;
    let torsion = root_three * side.powi(4) / 80.0;

    assert_eq!(report.shape, "triangle");
    assert_eq!(report.units, "mm");
    assert_close(report.area, area, 1e-12);
    assert_close(report.centroid_x, side / 2.0, 1e-12);
    assert_close(report.centroid_y, height / 3.0, 1e-12);
    assert_close(report.ix, second_moment, 1e-12);
    assert_close(report.iy, second_moment, 1e-12);
    assert_close(report.ixy, 0.0, 1e-12);
    // `polarMoment` carries the exact Saint-Venant torsion constant J, which is
    // not Ix + Iy for a solid triangle.
    assert_close(report.polar_moment, torsion, 1e-12);
    assert!((report.polar_moment - (report.ix + report.iy)).abs() > 1e-3 * torsion);
    assert_close(report.radius_of_gyration_x, side / 24.0_f64.sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, side / 24.0_f64.sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_x, (report.ix / area).sqrt(), 1e-12);
    assert_close(report.radius_of_gyration_y, (report.iy / area).sqrt(), 1e-12);

    // The closed equilateral outline (base along the bottom edge, apex at the
    // top centre) must reproduce the same area, centroid and second moments
    // with the polygon sign convention, where `Ixy = 0` by symmetry.
    let vertices = [(0.0, 0.0), (side, 0.0), (side / 2.0, height)];
    let polygon = polygon_section_report(&vertices, Units::Millimeter).expect("triangle polygon");
    assert_close(polygon.area, area, 1e-12);
    assert_close(polygon.centroid_x, side / 2.0, 1e-12);
    assert_close(polygon.centroid_y, height / 3.0, 1e-12);
    assert_close(polygon.ix, second_moment, 1e-12);
    assert_close(polygon.iy, second_moment, 1e-12);
    assert!(polygon.ixy.abs() <= 1e-12 * second_moment, "the equilateral triangle has Ixy = 0, got {}", polygon.ixy);
}

#[test]
fn triangle_section_rejects_invalid_dimensions() {
    for result in [
        triangle_section_report(0.0, Units::Millimeter),
        triangle_section_report(-100.0, Units::Millimeter),
        triangle_section_report(f64::NAN, Units::Millimeter),
        triangle_section_report(f64::INFINITY, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid triangle dimensions");
        assert_eq!(error.code, "section/invalid-dimensions");
    }
}

#[test]
fn triangle_section_report_is_deterministic_with_cli_field_names() {
    let first = triangle_section_report(100.0, Units::Meter).expect("triangle");
    let second = triangle_section_report(100.0, Units::Meter).expect("triangle");
    assert_eq!(first, second);

    let json = serde_json::to_value(&first).expect("json");
    let mut keys: Vec<&str> = json.as_object().expect("object").keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "area",
            "centroidX",
            "centroidY",
            "ix",
            "ixy",
            "iy",
            "polarMoment",
            "radiusOfGyrationX",
            "radiusOfGyrationY",
            "shape",
            "units",
        ]
    );
    assert_eq!(json["shape"], "triangle");
    assert_eq!(json["units"], "m");
    assert!(json["polarMoment"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json.get("centroid_x").is_none());
}

#[test]
fn z_section_report_is_deterministic_with_cli_field_names() {
    let first = z_section_report(200.0, 100.0, 10.0, Units::Meter).expect("z");
    let second = z_section_report(200.0, 100.0, 10.0, Units::Meter).expect("z");
    assert_eq!(first, second);

    let json = serde_json::to_value(&first).expect("json");
    let mut keys: Vec<&str> = json.as_object().expect("object").keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "area",
            "centroidX",
            "centroidY",
            "ix",
            "ixy",
            "iy",
            "polarMoment",
            "radiusOfGyrationX",
            "radiusOfGyrationY",
            "shape",
            "units",
        ]
    );
    assert_eq!(json["shape"], "z");
    assert_eq!(json["units"], "m");
    assert!(json["polarMoment"].as_f64().is_some_and(|value| value > 0.0));
    assert!(json.get("centroid_x").is_none());
}

#[test]
fn principal_section_matches_closed_form_cases() {
    for (ix, iy, ixy, expected_major, expected_minor, expected_angle) in [
        (100.0, 50.0, 0.0, 100.0, 50.0, 0.0),
        (100.0, 100.0, 20.0, 120.0, 80.0, 45.0),
        (50.0, 100.0, 0.0, 100.0, 50.0, 90.0),
        (100.0, 100.0, -20.0, 120.0, 80.0, -45.0),
    ] {
        let report = principal_section_report(ix, iy, ixy, Units::Millimeter).expect("principal section");
        assert_eq!(report.units, "mm");
        assert_close(report.ix, ix, 1e-12);
        assert_close(report.iy, iy, 1e-12);
        assert_close(report.ixy, ixy, 1e-12);
        assert_close(report.i1, expected_major, 1e-12);
        assert_close(report.i2, expected_minor, 1e-12);
        assert_close(report.polar_moment, ix + iy, 1e-12);
        assert_close(report.principal_angle, expected_angle, 1e-12);
        assert!(report.i1 >= report.i2, "the major moment must not be smaller than the minor one");
        assert!(
            report.principal_angle > -90.0 && report.principal_angle <= 90.0,
            "the angle must lie in (-90, 90], got {}",
            report.principal_angle
        );
    }
}

#[test]
fn principal_section_normalises_the_negative_right_angle_boundary() {
    let report = principal_section_report(50.0, 100.0, -0.0, Units::Millimeter).expect("principal section");
    assert_close(report.principal_angle, 90.0, 1e-12);
    assert!(report.principal_angle > -90.0 && report.principal_angle <= 90.0);
}

#[test]
fn principal_section_rejects_invalid_second_moments() {
    for result in [
        principal_section_report(0.0, 100.0, 0.0, Units::Millimeter),
        principal_section_report(-1.0, 100.0, 0.0, Units::Millimeter),
        principal_section_report(100.0, 0.0, 0.0, Units::Millimeter),
        principal_section_report(100.0, -1.0, 0.0, Units::Millimeter),
        principal_section_report(100.0, 50.0, 80.0, Units::Millimeter),
        principal_section_report(100.0, 50.0, -80.0, Units::Millimeter),
        principal_section_report(f64::NAN, 50.0, 0.0, Units::Millimeter),
        principal_section_report(100.0, f64::INFINITY, 0.0, Units::Millimeter),
        principal_section_report(100.0, 50.0, f64::NAN, Units::Millimeter),
        principal_section_report(100.0, 50.0, f64::NEG_INFINITY, Units::Millimeter),
    ] {
        let error = result.expect_err("invalid second moments");
        assert_eq!(error.code, "section/invalid-second-moments");
    }
}

#[test]
fn principal_section_report_is_deterministic_with_cli_field_names() {
    let first = principal_section_report(100.0, 100.0, 20.0, Units::Meter).expect("principal section");
    let second = principal_section_report(100.0, 100.0, 20.0, Units::Meter).expect("principal section");
    assert_eq!(first, second);
    let millimetres = principal_section_report(100.0, 100.0, 20.0, Units::Millimeter).expect("principal section");
    assert_eq!(millimetres.i1, first.i1);
    assert_eq!(millimetres.i2, first.i2);
    assert_eq!(millimetres.polar_moment, first.polar_moment);
    assert_eq!(millimetres.principal_angle, first.principal_angle);

    let json = serde_json::to_value(&first).expect("json");
    let mut keys: Vec<&str> = json.as_object().expect("object").keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["i1", "i2", "ix", "ixy", "iy", "polarMoment", "principalAngle", "units"]);
    assert_eq!(json["units"], "m");
    assert_close(json["ix"].as_f64().expect("ix"), 100.0, 1e-12);
    assert_close(json["iy"].as_f64().expect("iy"), 100.0, 1e-12);
    assert_close(json["ixy"].as_f64().expect("ixy"), 20.0, 1e-12);
    assert!(json.get("principal_angle").is_none());
}
