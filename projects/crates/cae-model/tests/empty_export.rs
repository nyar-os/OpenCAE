//! Empty-document behaviour of every export format.

use std::{fs, path::PathBuf};

use cae_model::{ExportFormat, ExportOptions, OpenCae, OpenCaeOptions, Units};

#[test]
fn empty_model_exports_report_clean_errors_or_write_content() {
    let directory = PathBuf::from(std::env::temp_dir()).join(format!("open-cae-empty-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("the temp directory exists");
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session
        .create(
            &directory.join("empty.ocae"),
            OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) },
        )
        .expect("an empty model is created");
    for (format, name) in [
        (ExportFormat::Csv, "csv"),
        (ExportFormat::Vtk, "empty.vtk"),
        (ExportFormat::Md, "empty.md"),
        (ExportFormat::Html, "empty.html"),
        (ExportFormat::Svg, "empty.svg"),
    ] {
        let target = directory.join(name);
        let error = session
            .export_with_options(format, &target, ExportOptions::default())
            .expect_err("an unsolved model cannot be exported");
        assert!(error.message.contains("at least 1 node"), "{}: {}", format.as_str(), error.message);
    }
    let _ = fs::remove_dir_all(&directory);
}
