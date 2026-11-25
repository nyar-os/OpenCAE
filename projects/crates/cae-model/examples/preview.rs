//! Preview driver: solves a demo truss and exports the SVG drawings.

use std::path::Path;

use cae_model::{ContourQuantity, ExportFormat, ExportOptions, OpenCae, OpenCaeOptions, Units};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "preview".into());
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
    session.set_material(210_000.0, 100.0, Some("steel".to_string()))?;
    session.add_support("n1", true, true, false)?;
    session.add_support("n2", true, true, false)?;
    session.add_load("n3", None, Some(-1000.0), None)?;

    // Default scale: auto so the deformed shape visibly separates from the
    // undeformed outline instead of lying on top of it.
    session.export_with_options(ExportFormat::Svg, &out.join("truss.svg"), ExportOptions::default())?;
    session.export_with_options(
        ExportFormat::Svg,
        &out.join("truss-axial.svg"),
        ExportOptions { contour: Some(ContourQuantity::Axial), ..Default::default() },
    )?;
    println!("done");
    Ok(())
}
