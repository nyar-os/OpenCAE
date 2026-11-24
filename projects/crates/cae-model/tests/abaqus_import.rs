//! Abaqus `.inp` import: the documented subset, loss reporting and errors.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, OpenCae, OpenCaeOptions, Units, import_abaqus, import_abaqus_file, solve};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/abaqus").join(name)
}

fn load(name: &str) -> cae_model::AbaqusImport {
    import_abaqus_file(&fixture(name), Units::Millimeter).expect("import succeeds")
}

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-abaqus-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

#[test]
fn minimal_model_imports_and_solves() {
    let imported = load("minimal.inp");
    let model = &imported.model;

    assert_eq!(model.dimension, 2);
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.elements.len(), 1);
    assert_eq!(model.material.name, "Steel");
    assert_close(model.material.youngs_modulus, 210_000.0, 1e-12);
    assert_close(model.material.area, 100.0, 1e-12);
    assert_eq!(model.supports.len(), 2);
    assert_eq!(model.supports[0].node, "1");
    assert!(model.supports[0].dx && model.supports[0].dy);
    assert_eq!(model.supports[1].node, "2");
    assert!(model.supports[1].dy);
    assert_eq!(model.loads.len(), 1);
    assert_close(model.loads[0].fx, 1000.0, 1e-12);

    assert_eq!(imported.losses.len(), 1, "only Poisson's ratio is unused: {:?}", imported.losses);
    assert_eq!(imported.losses[0].code, "import/abaqus-ignored-material-property");

    let solution = solve(model).expect("solve imported model");
    let tip = solution.nodes.iter().find(|node| node.id == "2").expect("node 2");
    assert_close(tip.ux, 1000.0 * 1000.0 / (210_000.0 * 100.0), 1e-10);
}

#[test]
fn comments_and_keyword_case_are_folded() {
    let imported = load("comments-case.inp");
    assert_eq!(imported.model.elements.len(), 1);
    assert_eq!(imported.model.elements[0].id, "10");
    assert!(
        imported.losses.iter().all(|loss| loss.code != "import/abaqus-unsupported-keyword"),
        "no keyword should be lost: {:?}",
        imported.losses
    );
    assert!(solve(&imported.model).is_ok());
}

#[test]
fn beam_section_and_py_distributed_load_are_mapped() {
    let imported = load("beam-dload.inp");
    let element = &imported.model.elements[0];
    assert_eq!(element.kind, ElementKind::Beam);
    assert_close(element.inertia.expect("beam inertia"), 50.0 * 100.0_f64.powi(3) / 12.0, 1e-12);
    assert_close(element.distributed_load.expect("distributed load"), -2.0, 1e-12);
    assert_close(element.material.as_ref().and_then(|material| material.area).expect("beam area"), 5_000.0, 1e-12);
    assert!(solve(&imported.model).is_ok());
}

#[test]
fn recognised_but_unrepresented_features_are_reported() {
    let imported = load("losses.inp");
    let codes: Vec<&str> = imported.losses.iter().map(|loss| loss.code.as_str()).collect();
    for expected in [
        "import/abaqus-unsupported-keyword",
        "import/abaqus-solid-element",
        "import/abaqus-unsupported-boundary",
        "import/abaqus-ignored-material-property",
    ] {
        assert!(codes.contains(&expected), "missing loss {expected}: {:?}", imported.losses);
    }
    assert!(imported.losses.iter().all(|loss| loss.line >= 1 && !loss.detail.is_empty()));
    assert!(
        imported.losses.iter().any(|loss| loss.keyword == "Instance"),
        "instance transforms must be reported: {:?}",
        imported.losses
    );
    assert_eq!(imported.model.elements.len(), 1, "solid elements are skipped, the truss remains");
}

#[test]
fn malformed_line_reports_its_line_number() {
    let error = import_abaqus_file(&fixture("malformed.inp"), Units::Millimeter).expect_err("must fail");
    assert_eq!(error.code, "import/abaqus-malformed-line");
    assert!(error.message.contains("line 2"), "{}", error.message);
    assert!(error.message.contains("2 or 3 coordinates"), "{}", error.message);
}

#[test]
fn unknown_node_reference_is_an_error_with_the_line() {
    let error = import_abaqus_file(&fixture("unknown-node.inp"), Units::Millimeter).expect_err("must fail");
    assert_eq!(error.code, "import/abaqus-unknown-node");
    assert!(error.message.contains("line 4"), "{}", error.message);
    assert!(error.message.contains("unknown node `2`"), "{}", error.message);
}

#[test]
fn include_is_rejected_instead_of_skipped() {
    let error = import_abaqus_file(&fixture("include.inp"), Units::Millimeter).expect_err("must fail");
    assert_eq!(error.code, "import/abaqus-unsupported-include");
    assert!(error.message.contains("line 1"), "{}", error.message);
}

#[test]
fn inline_unknown_keyword_becomes_a_loss() {
    let imported = import_abaqus(
        "*Node\n1, 0., 0.\n2, 10., 0.\n*Element, type=T3D2\n1, 1, 2\n*Material, name=Steel\n*Elastic\n200000., 0.\n*Truss Section, material=Steel\n10.\n*Surface\nS1, S2\n",
        Units::Millimeter,
    )
    .expect("import succeeds");
    assert!(
        imported.losses.iter().any(|loss| loss.code == "import/abaqus-unsupported-keyword" && loss.keyword == "Surface"),
        "{:?}",
        imported.losses
    );
}

#[test]
fn session_import_writes_a_new_deterministic_document() {
    let directory = temporary_dir("session");
    let first = directory.join("first.ocae");
    let second = directory.join("second.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());

    let report = session.import_abaqus(&fixture("minimal.inp"), &first, Some(Units::Millimeter)).expect("import");
    assert_eq!(report.format, "open-cae-abaqus-import");
    assert_eq!(report.version, 1);
    assert_eq!(report.document.nodes, 2);
    assert_eq!(report.document.elements, 1);
    assert_eq!(report.losses.len(), 1);

    let mut other = OpenCae::new(OpenCaeOptions::default());
    other.import_abaqus(&fixture("minimal.inp"), &second, Some(Units::Millimeter)).expect("import again");
    assert_eq!(fs::read(&first).expect("first file"), fs::read(&second).expect("second file"));

    let error = session.import_abaqus(&fixture("minimal.inp"), &first, None).expect_err("must refuse");
    assert_eq!(error.code, "file/exists");

    let summary = session.summary().expect("open document");
    assert_eq!(summary.nodes, 2);
    fs::remove_dir_all(directory).ok();
}
