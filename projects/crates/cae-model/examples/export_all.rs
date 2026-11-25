//! Development driver: solves a demo model and exports every drawing kind so
//! each sheet can be rendered and inspected before release.

use std::path::Path;

use cae_model::{ContourQuantity, ExportFormat, ExportOptions, OpenCae, OpenCaeOptions, Units};

fn export(session: &OpenCae, out: &Path, format: ExportFormat, name: &str, options: ExportOptions) {
    match session.export_with_options(format, &out.join(name), options) {
        Ok(report) => println!("OK {name}: {:?}", report.files),
        Err(err) => println!("ERR {name}: {}", err.message),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "export-all".into());
    let out = Path::new(&out);
    std::fs::create_dir_all(out)?;

    let path = out.join("demo.ocae");
    let _ = std::fs::remove_file(&path);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.create(&path, OpenCaeOptions { units: Some(Units::Millimeter), dimension: Some(2) })?;
    session.add_node("n1", -500.0, 0.0, None)?;
    session.add_node("n2", 500.0, 0.0, None)?;
    session.add_node("n3", 0.0, 500.0, None)?;
    session.add_element("e1", "n1", "n3", None, None)?;
    session.add_element("e2", "n2", "n3", None, None)?;
    // 7.85e-9 t/mm³ is steel, so the modal sheet has a positive density.
    session.set_material_full(210_000.0, 100.0, 7.85e-9, Some("steel".to_string()))?;
    session.add_support("n1", true, true, false)?;
    session.add_support("n2", true, true, false)?;
    session.add_load("n3", None, Some(-1000.0), None)?;
    session.add_combination("D", 1.0, 0.0)?;
    session.add_combination("D+L", 1.5, 0.0)?;

    let default = ExportOptions::default();
    export(&session, out, ExportFormat::Svg, "truss.svg", default);
    export(
        &session,
        out,
        ExportFormat::Svg,
        "truss-axial.svg",
        ExportOptions { contour: Some(ContourQuantity::Axial), ..default },
    );
    for (name, options) in [
        ("forces.svg", ExportOptions { drawing: Some("forces".parse()?), ..default }),
        ("shear.svg", ExportOptions { drawing: Some("shear".parse()?), ..default }),
        ("moment.svg", ExportOptions { drawing: Some("moment".parse()?), ..default }),
        ("displacements.svg", ExportOptions { drawing: Some("displacement".parse()?), ..default }),
        ("reactions.svg", ExportOptions { drawing: Some("reactions".parse()?), ..default }),
        ("overlay.svg", ExportOptions { drawing: Some("overlay".parse()?), ..default }),
        ("modes.svg", ExportOptions { drawing: Some("modal".parse()?), ..default }),
        ("envelope.svg", ExportOptions { drawing: Some("envelope".parse()?), ..default }),
        ("summary.svg", ExportOptions { drawing: Some("summary".parse()?), ..default }),
    ] {
        export(&session, out, ExportFormat::Svg, name, options);
    }
    export(&session, out, ExportFormat::Pdf, "model.pdf", default);
    export(&session, out, ExportFormat::Pdf, "summary.pdf", ExportOptions { drawing: Some("summary".parse()?), ..default });
    export(&session, out, ExportFormat::Csv, "csv", default);
    export(&session, out, ExportFormat::Vtk, "demo.vtk", default);
    export(&session, out, ExportFormat::Md, "demo.md", default);
    export(&session, out, ExportFormat::Html, "demo.html", default);

    Ok(())
}
