//! CSV internal-force table tests.
//!
//! A uniform load on a simply supported beam has closed-form shear and bending
//! moments, so the exported `forces.csv` rows are checked against those values.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cae_model::{ElementKind, ExportFormat, Model, OpenCae, OpenCaeOptions, Units, export_csv_with_stations, solve};

const E: f64 = 210_000.0;
const A: f64 = 100.0;
const I: f64 = 1.0e6;
const L: f64 = 1000.0;
const Q: f64 = -1.0;

fn temporary_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock").as_nanos();
    let directory = std::env::temp_dir().join(format!("ocae-csv-forces-{name}-{}-{unique}", std::process::id()));
    fs::create_dir_all(&directory).expect("create temporary directory");
    directory
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = actual.abs().max(expected.abs()).max(1e-300);
    assert!((actual - expected).abs() <= relative * scale, "expected {expected}, got {actual}");
}

fn simply_supported_beam() -> Model {
    let mut model = Model::new(Units::Millimeter);
    model.add_node("n1", 0.0, 0.0).expect("n1");
    model.add_node("n2", L, 0.0).expect("n2");
    model.add_element_with_loads("b1", "n1", "n2", None, ElementKind::Beam, Some(I), Some(Q), Some(Q)).expect("beam");
    model.set_material(Some("steel".to_string()), E, A);
    model.add_support("n1", true, true).expect("pin");
    model.add_support("n2", false, true).expect("roller");
    model
}

#[test]
fn forces_csv_matches_the_simply_supported_beam_solution() {
    let directory = temporary_dir("analytic");
    let model = simply_supported_beam();
    let solution = solve(&model).expect("solve");
    export_csv_with_stations(&model, &solution, &directory, 11).expect("csv export");

    let text = fs::read_to_string(directory.join("forces.csv")).expect("forces.csv");
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("element,from,to,kind,position,axialForce,shear,moment"));
    let rows: Vec<Vec<f64>> =
        lines.map(|line| line.split(',').skip(4).map(|field| field.parse::<f64>().expect("number")).collect()).collect();
    assert_eq!(rows.len(), 11);

    // Reaction is Q·L/2 = -500 N; the first sampled shear is +500 N.
    assert_close(rows[0][2], -Q * L / 2.0, 1e-12);
    // Midspan bending moment is -Q·L²/8 = 125000 N·mm.
    assert_close(rows[5][0], L / 2.0, 1e-12);
    assert_close(rows[5][3], -Q * L * L / 8.0, 1e-12);
    // Ends carry no bending moment.
    assert_close(rows[0][3], 0.0, 1e-9);
    assert_close(rows[10][3], 0.0, 1e-9);
    // The table is axial-force free for this transverse load.
    for row in &rows {
        assert_close(row[1], 0.0, 1e-12);
    }
}

#[test]
fn export_stations_option_refines_the_force_table_only() {
    let directory = temporary_dir("stations");
    let path = directory.join("beam.ocae");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) }).expect("create");
    session.add_node("n1", 0.0, 0.0, None).expect("n1");
    session.add_node("n2", L, 0.0, None).expect("n2");
    session.set_material(E, A, Some("steel".to_string())).expect("material");
    session.add_element_full("b1", "n1", "n2", None, None, ElementKind::Beam, Some(I), Some(Q)).expect("beam");
    session.add_support("n1", true, true, false).expect("pin");
    session.add_support("n2", false, true, false).expect("roller");

    let target = directory.join("csv");
    let options = cae_model::ExportOptions { scale: 1.0, stations: Some(5) };
    let report = session.export_with_options(ExportFormat::Csv, &target, options).expect("csv export");
    assert_eq!(report.files.len(), 4);
    assert!(report.files.iter().any(|file| file.ends_with("forces.csv")));

    let forces = fs::read_to_string(target.join("forces.csv")).expect("forces.csv");
    assert_eq!(forces.lines().count(), 1 + 5);
    let nodes = fs::read_to_string(target.join("nodes.csv")).expect("nodes.csv");
    assert_eq!(nodes.lines().count(), 1 + 2);
}

#[test]
fn invalid_station_counts_are_rejected() {
    let directory = temporary_dir("invalid");
    let model = simply_supported_beam();
    let solution = solve(&model).expect("solve");
    let error = export_csv_with_stations(&model, &solution, &directory, 1).expect_err("one station");
    assert_eq!(error.code, "analysis/invalid-stations");
    assert!(!directory.join("nodes.csv").exists(), "a rejected export must not write tables");
}
