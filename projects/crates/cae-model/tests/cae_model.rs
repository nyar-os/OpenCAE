use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{
    CAPABILITIES_JSON, Element, ElementMaterial, Model, Node, Severity, Units, VERSION, export_csv, export_vtk, solve,
};

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    let error = (actual - expected).abs();
    assert!(error <= relative * scale, "expected {expected}, got {actual}, relative error {}", error / scale);
}

fn single_bar_model() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", 1000.0, 0.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");
    model.add_support("n1", true, true).expect("support n1");
    model.add_support("n2", false, true).expect("support n2");
    model.add_load("n2", Some(1000.0), None).expect("load n2");
    model
}

fn two_bar_model() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", -500.0, 0.0).expect("node n1");
    model.add_node("n2", 500.0, 0.0).expect("node n2");
    model.add_node("n3", 0.0, 500.0).expect("node n3");
    model.add_element("e1", "n1", "n3", None).expect("element e1");
    model.add_element("e2", "n2", "n3", None).expect("element e2");
    model.add_support("n1", true, true).expect("support n1");
    model.add_support("n2", true, true).expect("support n2");
    model.add_load("n3", Some(0.0), Some(-1000.0)).expect("load n3");
    model
}

fn vertical_bar_3d_model() -> Model {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("node n1");
    model.add_node_3d("n2", 0.0, 0.0, 1000.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");
    model.add_support_3d("n1", true, true, true).expect("support n1");
    model.add_support_3d("n2", true, true, false).expect("support n2");
    model.add_load_3d("n2", None, None, Some(-1000.0)).expect("load n2");
    model
}

fn tetrahedron_3d_model() -> Model {
    let radius = 500.0;
    let offset = 0.866_025_403_784_438_6 * radius;
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, radius, 0.0).expect("node n1");
    model.add_node_3d("n2", -offset, -0.5 * radius, 0.0).expect("node n2");
    model.add_node_3d("n3", offset, -0.5 * radius, 0.0).expect("node n3");
    model.add_node_3d("n4", 0.0, 0.0, 500.0).expect("node n4");
    model.add_element("e1", "n1", "n4", None).expect("element e1");
    model.add_element("e2", "n2", "n4", None).expect("element e2");
    model.add_element("e3", "n3", "n4", None).expect("element e3");
    model.add_support_3d("n1", true, true, true).expect("support n1");
    model.add_support_3d("n2", true, true, true).expect("support n2");
    model.add_support_3d("n3", true, true, true).expect("support n3");
    model.add_load_3d("n4", None, None, Some(-1000.0)).expect("load n4");
    model
}

#[test]
fn single_bar_axial_extension_matches_analytic_solution() {
    let model = single_bar_model();
    let solution = solve(&model).expect("solve");

    let expected_displacement = 1000.0 * 1000.0 / (210_000.0 * 100.0);
    assert_close(expected_displacement, 0.047619047619047616, 1e-12);

    let node = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 result");
    assert_close(node.ux, expected_displacement, 1e-9);
    assert!(node.uy.abs() < 1e-12);

    let element = &solution.elements[0];
    assert_close(element.axial_force, 1000.0, 1e-9);
    assert_close(element.stress, 10.0, 1e-9);
    assert_close(element.strain, 10.0 / 210_000.0, 1e-9);
    assert_close(element.length, 1000.0, 1e-12);
}

#[test]
fn symmetric_two_bar_truss_matches_analytic_solution() {
    let model = two_bar_model();
    let solution = solve(&model).expect("solve");

    let length = 500.0 * 2.0_f64.sqrt();
    let sine = (2.0_f64).sqrt() / 2.0;
    let expected_axial = -1000.0 / (2.0 * sine);
    let expected_displacement = -1000.0 * length / (2.0 * 210_000.0 * 100.0 * sine * sine);

    let apex = solution.nodes.iter().find(|node| node.id == "n3").expect("apex result");
    assert!(apex.ux.abs() < 1e-12);
    assert_close(apex.uy, expected_displacement, 1e-9);

    for element in &solution.elements {
        assert_close(element.axial_force, expected_axial, 1e-9);
        assert!(element.axial_force < 0.0, "members must be in compression");
    }

    let reaction_sum: f64 = solution.reactions.iter().map(|reaction| reaction.fy).sum();
    assert_close(reaction_sum, 1000.0, 1e-9);
}

#[test]
fn under_constrained_model_reports_mechanism() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", 1000.0, 0.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");
    model.add_support("n1", true, false).expect("support n1");

    let issues = model.validate();
    assert!(
        issues.iter().any(|issue| issue.code == "model/mechanism" && issue.severity == Severity::Error),
        "issues: {issues:?}"
    );

    let error = solve(&model).expect_err("mechanism must not solve");
    assert_eq!(error.code, "model/mechanism");
    assert_eq!(error.exit_code(), 1);
}

#[test]
fn fully_constrained_structure_balances_external_loads() {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("node n1");
    model.add_node("n2", 1000.0, 0.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");
    model.add_support("n1", true, true).expect("support n1");
    model.add_support("n2", true, true).expect("support n2");
    model.add_load("n2", Some(500.0), Some(-300.0)).expect("load n2");

    let solution = solve(&model).expect("solve");
    for node in &solution.nodes {
        assert!(node.ux.abs() < 1e-12);
        assert!(node.uy.abs() < 1e-12);
    }

    let reaction_fx: f64 = solution.reactions.iter().map(|reaction| reaction.fx).sum();
    let reaction_fy: f64 = solution.reactions.iter().map(|reaction| reaction.fy).sum();
    let load_fx: f64 = model.loads.iter().map(|load| load.fx).sum();
    let load_fy: f64 = model.loads.iter().map(|load| load.fy).sum();
    assert!(reaction_fx + load_fx < 1e-9);
    assert!(reaction_fy + load_fy < 1e-9);
}

#[test]
fn validation_reports_required_error_codes() {
    let mut duplicate = single_bar_model();
    duplicate.nodes.push(Node { id: "n2".to_string(), x: 10.0, y: 10.0, z: 0.0 });
    assert!(duplicate.validate().iter().any(|issue| issue.code == "node/duplicate-id"));

    let mut zero_length = single_bar_model();
    zero_length.nodes[1].x = 0.0;
    zero_length.nodes[1].y = 0.0;
    assert!(zero_length.validate().iter().any(|issue| issue.code == "element/zero-length"));

    let mut invalid_material = single_bar_model();
    invalid_material.material.youngs_modulus = 0.0;
    assert!(invalid_material.validate().iter().any(|issue| issue.code == "material/invalid"));

    let mut duplicate_element = single_bar_model();
    duplicate_element.elements.push(Element {
        id: "e2".to_string(),
        from: "n2".to_string(),
        to: "n1".to_string(),
        material: None,
        kind: cae_model::ElementKind::Truss,
        inertia: None,
        distributed_load: None,
        distributed_load_end: None,
        temperature: None,
        temperature_gradient: None,
        release: None,
    });
    assert!(duplicate_element.validate().iter().any(|issue| issue.code == "element/duplicate"));

    let mut unknown_units = single_bar_model();
    unknown_units.units = None;
    assert!(
        unknown_units
            .validate()
            .iter()
            .any(|issue| issue.code == "document/assumed-units" && issue.severity == Severity::Notice)
    );
}

#[test]
fn validation_warns_about_unconnected_nodes_and_duplicate_coordinates() {
    let mut model = single_bar_model();
    model.add_node("n3", 0.0, 0.0).expect("node n3");
    model.add_support("n3", true, true).expect("support n3");

    let issues = model.validate();
    assert!(
        issues.iter().any(|issue| issue.code == "node/unconnected" && issue.severity == Severity::Warning),
        "missing node/unconnected warning: {issues:?}"
    );
    assert!(
        issues.iter().any(|issue| issue.code == "node/duplicate-coordinates" && issue.severity == Severity::Warning),
        "missing node/duplicate-coordinates warning: {issues:?}"
    );
    assert!(!issues.iter().any(|issue| issue.severity == Severity::Error), "warnings must not block solving: {issues:?}");
    solve(&model).expect("a fully supported unconnected node does not change the solution");
}

#[test]
fn validation_does_not_warn_about_connected_nodes() {
    let model = single_bar_model();
    let issues = model.validate();
    assert!(
        !issues.iter().any(|issue| issue.code == "node/unconnected" || issue.code == "node/duplicate-coordinates"),
        "connected nodes must not be reported: {issues:?}"
    );
}

#[test]
fn csv_vtk_and_json_exports_are_consistent() {
    let directory = temporary_dir("export");
    let model = single_bar_model();
    let solution = solve(&model).expect("solve");
    let expected_displacement = 1000.0 * 1000.0 / (210_000.0 * 100.0);

    export_csv(&model, &solution, &directory).expect("csv export");
    let nodes = fs::read_to_string(directory.join("nodes.csv")).expect("nodes.csv");
    let elements = fs::read_to_string(directory.join("elements.csv")).expect("elements.csv");
    assert_eq!(nodes.lines().count(), 1 + model.nodes.len());
    assert_eq!(elements.lines().count(), 1 + model.elements.len());
    assert!(nodes.starts_with("id,x,y,z,ux,uy,uz"));

    export_vtk(&model, &solution, &directory).expect("vtk export");
    let vtk = fs::read_to_string(directory.join("model.vtk")).expect("model.vtk");
    assert!(vtk.contains("POINT_DATA"));
    assert!(vtk.contains("DISPLACEMENT"));
    assert!(vtk.contains("CELL_DATA"));
    assert!(vtk.contains("axial_force"));
    assert!(vtk.contains("stress"));

    let report = serde_json::to_value(&solution).expect("report json");
    assert_eq!(report["format"], "open-cae-truss-report");
    assert_eq!(report["version"], VERSION);
    let maximum = report["maxDisplacement"]["magnitude"].as_f64().expect("maxDisplacement");
    assert_close(maximum, expected_displacement, 1e-9);
}

#[test]
fn element_material_override_changes_stiffness() {
    let mut model = single_bar_model();
    model.elements[0].material = Some(ElementMaterial { youngs_modulus: Some(100_000.0), area: Some(50.0) });

    let solution = solve(&model).expect("solve");
    let node = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 result");
    assert_close(node.ux, 1000.0 * 1000.0 / (100_000.0 * 50.0), 1e-9);
    assert_close(solution.elements[0].axial_force, 1000.0, 1e-9);
    assert_close(solution.elements[0].stress, 20.0, 1e-9);
}

#[test]
fn document_round_trip_preserves_content_and_revision() {
    let mut model = two_bar_model();
    model
        .add_element("e3", "n1", "n2", Some(ElementMaterial { youngs_modulus: Some(200_000.0), area: Some(50.0) }))
        .expect("element e3");

    let text = model.to_json_pretty().expect("serialize");
    let parsed = Model::parse_json(&text).expect("parse");
    assert_eq!(model, parsed);
    assert!(parsed.revision > 0);

    let directory = temporary_dir("roundtrip");
    let path = directory.join("model.ocae");
    model.save_atomic(&path).expect("save");
    let loaded = Model::load(&path).expect("load");
    assert_eq!(model, loaded);
    assert!(text.contains("\"youngsModulus\": 200000.0"));
}

#[test]
fn capability_matrix_is_explicit_about_unsupported_features() {
    let capabilities: serde_json::Value = serde_json::from_str(CAPABILITIES_JSON).expect("capability json");
    assert_eq!(capabilities["solver"]["truss2d"], "supported");
    assert_eq!(capabilities["solver"]["truss3d"], "supported");
    assert_eq!(capabilities["solver"]["beam2d"], "supported");
    assert_eq!(capabilities["solver"]["beam3d"], "unsupported");
    assert_eq!(capabilities["solver"]["frame3d"], "unsupported");
    assert_eq!(capabilities["solver"]["modal"], "supported");
    assert_eq!(capabilities["solver"]["selfWeight"], "supported");
    assert_eq!(capabilities["solver"]["prescribedDisplacement"], "supported");
    assert_eq!(capabilities["solver"]["nonlinear"], "unsupported");
    assert_eq!(capabilities["solver"]["buckling"], "supported");
    assert_eq!(capabilities["solver"]["internalForces"], "supported");
    assert_eq!(capabilities["solver"]["loadCombinations"], "supported");
    assert_eq!(capabilities["solver"]["thermal"], "unsupported");
    assert_eq!(capabilities["io"]["csv"], "supported");
    assert_eq!(capabilities["io"]["vtk"], "supported");
    assert_eq!(capabilities["io"]["md"], "supported");
    assert_eq!(capabilities["io"]["inp"], "unsupported");
}

#[test]
fn three_dimensional_vertical_bar_matches_analytic_solution() {
    let model = vertical_bar_3d_model();
    let solution = solve(&model).expect("solve");

    let expected_displacement = -1000.0 * 1000.0 / (210_000.0 * 100.0);
    let node = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 result");
    assert_close(node.uz, expected_displacement, 1e-9);
    assert!(node.ux.abs() < 1e-12);
    assert!(node.uy.abs() < 1e-12);
    assert_close(node.magnitude, expected_displacement.abs(), 1e-9);

    let element = &solution.elements[0];
    assert_close(element.length, 1000.0, 1e-12);
    assert_close(element.axial_force, -1000.0, 1e-9);
    assert_close(element.stress, -10.0, 1e-9);
    assert_close(element.strain, -10.0 / 210_000.0, 1e-9);

    let report = serde_json::to_value(&solution).expect("report json");
    assert_close(report["nodes"][1]["uz"].as_f64().expect("uz"), expected_displacement, 1e-9);
    assert!(report["reactions"][0]["fz"].as_f64().expect("fz") > 999.999_999);
}

#[test]
fn three_dimensional_tetrahedron_is_symmetric_under_vertical_load() {
    let model = tetrahedron_3d_model();
    let solution = solve(&model).expect("solve");

    let leg_length = 500.0_f64.hypot(500.0);
    let vertical_cosine = 500.0 / leg_length;
    let expected_apex = -1000.0 * leg_length / (3.0 * 210_000.0 * 100.0 * vertical_cosine * vertical_cosine);
    let expected_axial = -1000.0 / (3.0 * vertical_cosine);

    let apex = solution.nodes.iter().find(|node| node.id == "n4").expect("apex result");
    assert!(apex.ux.abs() < 1e-9, "apex ux = {}", apex.ux);
    assert!(apex.uy.abs() < 1e-9, "apex uy = {}", apex.uy);
    assert_close(apex.uz, expected_apex, 1e-9);

    for element in &solution.elements {
        assert_close(element.axial_force, expected_axial, 1e-9);
        assert!(element.axial_force < 0.0, "legs must be in compression");
    }

    let reaction_sum: f64 = solution.reactions.iter().map(|reaction| reaction.fz).sum();
    assert_close(reaction_sum, 1000.0, 1e-9);
}

#[test]
fn three_dimensional_under_constrained_model_reports_mechanism() {
    let mut model = Model::new_with_dimension(Units::Millimeter, 3);
    model.add_node_3d("n1", 0.0, 0.0, 0.0).expect("node n1");
    model.add_node_3d("n2", 0.0, 0.0, 1000.0).expect("node n2");
    model.add_element("e1", "n1", "n2", None).expect("element e1");
    model.add_support_3d("n1", true, true, true).expect("support n1");
    model.add_load_3d("n2", None, None, Some(-1000.0)).expect("load n2");

    let issues = model.validate();
    assert!(
        issues.iter().any(|issue| issue.code == "model/mechanism" && issue.severity == Severity::Error),
        "issues: {issues:?}"
    );
    let error = solve(&model).expect_err("mechanism must not solve");
    assert_eq!(error.code, "model/mechanism");
}

#[test]
fn three_dimensional_document_round_trip_preserves_dimension_and_z() {
    let model = vertical_bar_3d_model();
    let text = model.to_json_pretty().expect("serialize");
    assert!(text.contains("\"dimension\": 3"));
    assert!(text.contains("\"z\": 1000.0"));
    assert!(text.contains("\"dz\": true"));
    assert!(text.contains("\"fz\": -1000.0"));

    let parsed = Model::parse_json(&text).expect("parse");
    assert_eq!(model, parsed);
    assert_eq!(parsed.dimension, 3);
    assert!(parsed.revision > 0);

    let directory = temporary_dir("roundtrip-3d");
    let path = directory.join("model.ocae");
    model.save_atomic(&path).expect("save");
    let loaded = Model::load(&path).expect("load");
    assert_eq!(model, loaded);
    assert_eq!(loaded.dimension, 3);
    assert_eq!(loaded.nodes[1].z, 1000.0);
}

#[test]
fn legacy_two_dimensional_document_defaults_dimension_and_z() {
    let legacy = r#"{
      "format": "open-cae-truss",
      "version": 1,
      "units": "mm",
      "material": { "name": "steel", "youngsModulus": 210000.0, "area": 100.0 },
      "nodes": [
        { "id": "n1", "x": 0.0, "y": 0.0 },
        { "id": "n2", "x": 1000.0, "y": 0.0 }
      ],
      "elements": [ { "id": "e1", "from": "n1", "to": "n2" } ],
      "supports": [
        { "node": "n1", "dx": true, "dy": true },
        { "node": "n2", "dy": true }
      ],
      "loads": [ { "node": "n2", "fx": 1000.0 } ],
      "revision": 0
    }"#;
    let model = Model::parse_json(legacy).expect("parse legacy document");
    assert_eq!(model.dimension, 2);
    assert!(model.nodes.iter().all(|node| node.z == 0.0));
    assert!(model.supports.iter().all(|support| !support.dz));
    assert!(model.loads.iter().all(|load| load.fz == 0.0));
    assert!(model.validate().iter().all(|issue| issue.severity != Severity::Error));

    let solution = solve(&model).expect("legacy model still solves");
    let node = solution.nodes.iter().find(|node| node.id == "n2").expect("n2 result");
    assert_close(node.ux, 1000.0 * 1000.0 / (210_000.0 * 100.0), 1e-9);
    assert_eq!(node.uz, 0.0);
}

#[test]
fn two_dimensional_documents_reject_z_fields() {
    let mut model = single_bar_model();
    let error = model.add_node_3d("n3", 0.0, 0.0, 10.0).expect_err("z must be rejected");
    assert_eq!(error.code, "document/dimension-mismatch");
    assert_eq!(error.exit_code(), 1);

    let mut z_node = single_bar_model();
    z_node.nodes[1].z = 5.0;
    assert!(z_node.validate().iter().any(|issue| issue.code == "document/dimension-mismatch"));

    let mut dz_support = single_bar_model();
    dz_support.supports[0].dz = true;
    assert!(dz_support.validate().iter().any(|issue| issue.code == "document/dimension-mismatch"));

    let mut fz_load = single_bar_model();
    fz_load.loads[0].fz = 10.0;
    assert!(fz_load.validate().iter().any(|issue| issue.code == "document/dimension-mismatch"));

    let mut invalid_dimension = single_bar_model();
    invalid_dimension.dimension = 4;
    assert!(invalid_dimension.validate().iter().any(|issue| issue.code == "document/dimension"));
}

#[test]
fn three_dimensional_csv_and_vtk_contain_z_columns() {
    let directory = temporary_dir("export-3d");
    let model = vertical_bar_3d_model();
    let solution = solve(&model).expect("solve");

    export_csv(&model, &solution, &directory).expect("csv export");
    let nodes = fs::read_to_string(directory.join("nodes.csv")).expect("nodes.csv");
    assert!(nodes.starts_with("id,x,y,z,ux,uy,uz"));
    assert!(nodes.lines().any(|line| line.starts_with("n2,0,0,1000,")), "nodes.csv: {nodes}");

    export_vtk(&model, &solution, &directory).expect("vtk export");
    let vtk = fs::read_to_string(directory.join("model.vtk")).expect("model.vtk");
    assert!(vtk.contains("POINTS 2 double"));
    assert!(vtk.contains("0.0 0.0 1000.0"), "vtk: {vtk}");
    assert!(vtk.contains("VECTORS DISPLACEMENT double"));
}
