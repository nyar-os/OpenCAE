use std::{fmt::Write as _, fs, path::Path};

use science_mesh::{Cell, CellKind, Field, Mesh, MeshError};

use crate::{
    CaeError,
    model::{ElementKind, Model, Units},
    solver::{ModalSolution, Solution},
    validate::{Issue, Severity},
};

/// Writes text to `path` through a same-directory temporary file published by
/// `boost-session`, which replaces an existing target with a single rename.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), CaeError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)
            .map_err(|error| CaeError::new("io/write", format!("cannot create directory `{}`: {error}", parent.display())))?;
    }
    let target = if path.parent().is_some_and(|parent| !parent.as_os_str().is_empty()) {
        path.to_path_buf()
    }
    else {
        Path::new(".").join(path)
    };
    boost_session::save_atomic(&target, contents.as_bytes())
        .map_err(|error| CaeError::new("io/write", format!("cannot write `{}`: {error}", target.display())))
}

/// Writes `nodes.csv` and `elements.csv` into `directory`.
///
/// The solution must have been obtained from the same model so that node
/// coordinates and displacement rows line up.
pub fn export_csv(model: &Model, solution: &Solution, directory: &Path) -> Result<(), CaeError> {
    check_alignment(model, solution)?;
    fs::create_dir_all(directory).map_err(|error| {
        CaeError::new("io/write", format!("cannot create export directory `{}`: {error}", directory.display()))
    })?;

    let mut nodes = String::from("id,x,y,z,ux,uy,uz\n");
    for (node, displacement) in model.nodes.iter().zip(&solution.nodes) {
        let _ = writeln!(
            nodes,
            "{},{},{},{},{},{},{}",
            csv_field(&node.id),
            node.x,
            node.y,
            node.z,
            displacement.ux,
            displacement.uy,
            displacement.uz
        );
    }

    let mut elements = String::from("id,from,to,length,axialForce,stress,strain\n");
    for element in &solution.elements {
        let _ = writeln!(
            elements,
            "{},{},{},{},{},{},{}",
            csv_field(&element.id),
            csv_field(&element.from),
            csv_field(&element.to),
            element.length,
            element.axial_force,
            element.stress,
            element.strain
        );
    }

    write_atomic(&directory.join("nodes.csv"), &nodes)?;
    write_atomic(&directory.join("elements.csv"), &elements)?;
    Ok(())
}

/// Writes a legacy ASCII VTK `UNSTRUCTURED_GRID` file with two-node line
/// cells.
///
/// The points, line cells and `DISPLACEMENT` point vector are written by
/// `science-mesh`; the `axial_force` and `stress` cell arrays are appended
/// afterwards because the shared writer carries point data only. Points are
/// always three-dimensional and two-dimensional models keep `z = 0`. The file
/// can be opened directly by ParaView.
///
/// When `path` ends in `.vtk` it is used as the output file, otherwise `path`
/// is treated as a directory and `model.vtk` is created inside it.
pub fn export_vtk(model: &Model, solution: &Solution, path: &Path) -> Result<(), CaeError> {
    check_alignment(model, solution)?;
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("vtk") {
        path.to_path_buf()
    }
    else {
        path.join("model.vtk")
    };

    let points = model.nodes.iter().map(|node| [node.x, node.y, node.z]).collect();
    let mut cells = Vec::with_capacity(model.elements.len());
    for element in &model.elements {
        let from = model
            .nodes
            .iter()
            .position(|node| node.id == element.from)
            .ok_or_else(|| missing_vtk_node(&element.id, &element.from))?;
        let to = model
            .nodes
            .iter()
            .position(|node| node.id == element.to)
            .ok_or_else(|| missing_vtk_node(&element.id, &element.to))?;
        cells.push(Cell::new(CellKind::Line, vec![from, to]));
    }

    let mut displacement = Vec::with_capacity(solution.nodes.len() * 3);
    for node in &solution.nodes {
        displacement.extend([node.ux, node.uy, node.uz]);
    }
    let fields = [Field::vector("DISPLACEMENT", displacement)];

    let mesh = Mesh::new(points, cells);
    let mut text = mesh
        .to_legacy_vtk(&fields)
        .map(|text| text.replacen("OpenMesh", "OpenCAE truss linear static result", 1))
        .map_err(mesh_error)?;

    let _ = writeln!(text, "CELL_DATA {}", solution.elements.len());
    text.push_str("SCALARS axial_force double 1\n");
    text.push_str("LOOKUP_TABLE default\n");
    for element in &solution.elements {
        let _ = writeln!(text, "{:.10e}", element.axial_force);
    }
    text.push_str("SCALARS stress double 1\n");
    text.push_str("LOOKUP_TABLE default\n");
    for element in &solution.elements {
        let _ = writeln!(text, "{:.10e}", element.stress);
    }

    write_atomic(&target, &text)
}

/// Writes a deterministic Markdown engineering report.
///
/// The report contains the model summary, validation findings, the static
/// solve summary, displacement and reaction tables, member end forces, and
/// modal frequencies when `modal` is provided. All numbers use scientific
/// notation with six fractional digits, so identical inputs always produce
/// byte-identical output.
///
/// When `path` ends in `.md` it is used as the output file, otherwise `path`
/// is treated as a directory and `report.md` is created inside it.
pub fn export_md(
    model: &Model,
    solution: &Solution,
    modal: Option<&ModalSolution>,
    issues: &[Issue],
    path: &Path,
) -> Result<(), CaeError> {
    check_alignment(model, solution)?;
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
        path.to_path_buf()
    }
    else {
        path.join("report.md")
    };

    let mut text = String::new();
    text.push_str("# OpenCAE Analysis Report\n\n");

    text.push_str("## Model Summary\n\n");
    text.push_str("| Item | Value |\n| --- | --- |\n");
    let _ = writeln!(text, "| Document format | `{}` v{} |", model.format, model.version);
    let _ = writeln!(text, "| Dimension | {}D |", model.dimension);
    let _ = writeln!(text, "| Units | {} |", model.units.unwrap_or(Units::Millimeter).as_str());
    let _ = writeln!(text, "| Nodes | {} |", model.nodes.len());
    let _ = writeln!(text, "| Elements | {} |", model.elements.len());
    let _ = writeln!(
        text,
        "| Element types | truss {} / beam {} |",
        model.elements.iter().filter(|element| element.kind == ElementKind::Truss).count(),
        model.elements.iter().filter(|element| element.kind == ElementKind::Beam).count()
    );
    let _ = writeln!(text, "| Support entries | {} |", model.supports.len());
    let _ = writeln!(text, "| Concentrated load entries | {} |", model.loads.len());
    let _ = writeln!(text, "| Material | {} |", model.material.name);
    let _ = writeln!(text, "| E | {} |", format_float(model.material.youngs_modulus));
    let _ = writeln!(text, "| A | {} |", format_float(model.material.area));
    let _ = writeln!(text, "| density | {} |", format_float(model.material.density));
    if let Some(gravity) = model.gravity {
        let _ = writeln!(
            text,
            "| gravity | [{}, {}, {}] |",
            format_float(gravity[0]),
            format_float(gravity[1]),
            format_float(gravity[2])
        );
    }
    else {
        text.push_str("| gravity | not set |\n");
    }

    text.push_str("\n## Validation\n\n");
    if issues.is_empty() {
        text.push_str("Passed, no issues found.\n");
    }
    else {
        text.push_str("| Severity | Code | Message |\n| --- | --- | --- |\n");
        for issue in issues {
            let severity = match issue.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Notice => "notice",
            };
            let _ = writeln!(text, "| {severity} | `{}` | {} |", issue.code, escape_markdown(&issue.message));
        }
    }

    text.push_str("\n## Solution Summary\n\n");
    text.push_str("| Item | Value |\n| --- | --- |\n");
    let _ = writeln!(
        text,
        "| Max displacement | {} (node `{}`, component norm) |",
        format_float(solution.max_displacement.magnitude),
        solution.max_displacement.id
    );
    let _ = writeln!(
        text,
        "| Max axial force | {} (element `{}`) |",
        format_float(solution.max_axial_force.axial_force),
        solution.max_axial_force.element
    );
    let _ = writeln!(
        text,
        "| Max bending moment | {} (element `{}`) |",
        format_float(solution.max_moment.moment),
        solution.max_moment.element
    );
    let _ = writeln!(text, "| Total strain energy | {} |", format_float(solution.strain_energy));

    text.push_str("\n## Nodal Displacements\n\n");
    text.push_str("| Node | ux | uy | uz | rz | Magnitude |\n| --- | --- | --- | --- | --- | --- |\n");
    for node in &solution.nodes {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {} | {} |",
            escape_markdown(&node.id),
            format_float(node.ux),
            format_float(node.uy),
            format_float(node.uz),
            format_float(node.rz),
            format_float(node.magnitude)
        );
    }

    text.push_str("\n## Nodal Reactions\n\n");
    text.push_str("| Node | fx | fy | fz | mz |\n| --- | --- | --- | --- | --- |\n");
    for reaction in &solution.reactions {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {} |",
            escape_markdown(&reaction.node),
            format_float(reaction.fx),
            format_float(reaction.fy),
            format_float(reaction.fz),
            format_float(reaction.mz)
        );
    }

    text.push_str("\n## Element Results\n\n");
    text.push_str(
        "| Element | Type | Length | Axial force | Stress | maxAbsMoment | n1 | v1 | m1 | n2 | v2 | m2 |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for element in &solution.elements {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            escape_markdown(&element.id),
            element.kind.as_str(),
            format_float(element.length),
            format_float(element.axial_force),
            format_float(element.stress),
            format_float(element.max_abs_moment),
            format_float(element.end_forces.n1),
            format_float(element.end_forces.v1),
            format_float(element.end_forces.m1),
            format_float(element.end_forces.n2),
            format_float(element.end_forces.v2),
            format_float(element.end_forces.m2)
        );
    }

    text.push_str("\n## Modal Results\n\n");
    match modal {
        Some(modal) => {
            let _ = writeln!(
                text,
                "Requested {} modes, returned {}, skipped {} zero/negative eigenvalues.\n",
                modal.requested,
                modal.modes.len(),
                modal.skipped
            );
            if modal.modes.is_empty() {
                text.push_str("No positive eigenvalues available.\n");
            }
            else {
                text.push_str("| Mode | Frequency (Hz) | Eigenvalue | Max amplitude |\n| --- | --- | --- | --- |\n");
                for (index, mode) in modal.modes.iter().enumerate() {
                    let _ = writeln!(
                        text,
                        "| {} | {} | {} | {} |",
                        index + 1,
                        format_float(mode.frequency_hz),
                        format_float(mode.eigenvalue),
                        format_float(mode.max_amplitude)
                    );
                }
            }
        }
        None => {
            text.push_str("Not run (requires a positive density).\n");
        }
    }

    write_atomic(&target, &text)
}

fn format_float(value: f64) -> String {
    format!("{value:.6e}")
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn check_alignment(model: &Model, solution: &Solution) -> Result<(), CaeError> {
    if model.nodes.len() != solution.nodes.len()
        || model.nodes.iter().zip(&solution.nodes).any(|(node, result)| node.id != result.id)
    {
        return Err(CaeError::new("export/mismatch", "solution nodes do not match the model nodes; cannot export"));
    }
    if model.elements.len() != solution.elements.len() {
        return Err(CaeError::new("export/mismatch", "solution element count does not match the model; cannot export"));
    }
    Ok(())
}

fn missing_vtk_node(element: &str, node: &str) -> CaeError {
    CaeError::new("export/invalid-model", format!("VTK export cannot find node `{node}` for element `{element}`"))
}

fn mesh_error(error: MeshError) -> CaeError {
    CaeError::new(error.code(), error.message())
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    }
    else {
        value.to_string()
    }
}
