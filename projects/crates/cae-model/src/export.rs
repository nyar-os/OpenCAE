use std::{fmt::Write as _, fs, path::Path, str::FromStr};

use science_mesh::{Cell, CellKind, Field, Mesh, MeshError};

use crate::{
    CaeError,
    model::{ElementKind, Model, Units},
    solver::{ModalSolution, Solution, solve_internal_forces},
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

/// Writes `nodes.csv`, `elements.csv`, `reactions.csv` and `forces.csv` into
/// `directory`.
///
/// The solution must have been obtained from the same model so that node
/// coordinates and displacement rows line up. `reactions.csv` lists one row per
/// support (`node,fx,fy,fz,mz`) using the signs of the reaction summary.
/// `forces.csv` writes the sampled internal forces at the default 11 stations
/// per element; use [`export_csv_with_stations`] to change the sampling.
pub fn export_csv(model: &Model, solution: &Solution, directory: &Path) -> Result<(), CaeError> {
    export_csv_with_stations(model, solution, directory, 11)
}

/// Writes the four CSV result tables with `stations` sampled points per member.
///
/// `stations` counts both element ends and must be at least `2`; the force table
/// stores one row per station with `position`, `axialForce`, `shear` and
/// `moment`. Truss members report a constant axial force and zero shear and
/// moment. The station count only affects `forces.csv`; the other three tables
/// are unchanged.
pub fn export_csv_with_stations(model: &Model, solution: &Solution, directory: &Path, stations: usize) -> Result<(), CaeError> {
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

    let mut reactions = String::from("node,fx,fy,fz,mz\n");
    for support in &solution.reaction_summary.supports {
        let _ = writeln!(reactions, "{},{},{},{},{}", csv_field(&support.node), support.fx, support.fy, support.fz, support.mz);
    }

    let diagrams = solve_internal_forces(model, stations)?;
    let mut forces = String::from("element,from,to,kind,position,axialForce,shear,moment\n");
    for element in &diagrams.elements {
        for station in &element.stations {
            let _ = writeln!(
                forces,
                "{},{},{},{},{},{},{},{}",
                csv_field(&element.id),
                csv_field(&element.from),
                csv_field(&element.to),
                element.kind.as_str(),
                station.position,
                station.axial_force,
                station.shear,
                station.moment
            );
        }
    }

    write_atomic(&directory.join("nodes.csv"), &nodes)?;
    write_atomic(&directory.join("elements.csv"), &elements)?;
    write_atomic(&directory.join("reactions.csv"), &reactions)?;
    write_atomic(&directory.join("forces.csv"), &forces)?;
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

/// Result quantity an SVG contour export colours the members by.
///
/// The values come from the same reports the CLI and SDK return
/// (`internalForces` for `axial`, `shear` and `moment`, nodal displacement
/// magnitudes for `displacement`), so the legend and the numbers cannot drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContourQuantity {
    /// Axial force, positive in tension.
    Axial,
    /// Transverse shear force.
    Shear,
    /// Bending moment.
    Moment,
    /// Nodal displacement magnitude.
    Displacement,
}

impl ContourQuantity {
    /// Returns the short identifier used by the CLI, the addon and the SDK.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Axial => "axial",
            Self::Shear => "shear",
            Self::Moment => "moment",
            Self::Displacement => "displacement",
        }
    }

    /// Returns the human readable label shown in the SVG legend.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Axial => "axial force",
            Self::Shear => "shear force",
            Self::Moment => "bending moment",
            Self::Displacement => "displacement",
        }
    }

    /// Returns the unit label of the quantity in the document unit system.
    pub fn unit(self, units: Units) -> &'static str {
        match (self, units) {
            (Self::Axial | Self::Shear, _) => "N",
            (Self::Moment, Units::Millimeter) => "N·mm",
            (Self::Moment, Units::Meter) => "N·m",
            (Self::Displacement, Units::Millimeter) => "mm",
            (Self::Displacement, Units::Meter) => "m",
        }
    }
}

impl FromStr for ContourQuantity {
    type Err = CaeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "axial" => Ok(Self::Axial),
            "shear" => Ok(Self::Shear),
            "moment" => Ok(Self::Moment),
            "displacement" => Ok(Self::Displacement),
            other => Err(CaeError::new(
                "export/invalid-contour",
                format!(
                    "unsupported contour quantity `{other}`; valid values are `axial`, `shear`, `moment` or `displacement`"
                ),
            )),
        }
    }
}

/// Writes a deterministic, self-contained SVG drawing of the model and its
/// deformed shape.
///
/// Every node is projected onto the global X–Y plane, so a three-dimensional
/// model is drawn as its XY projection with Z ignored. The undeformed members
/// are thin grey lines, the deformed shape after a static solve is a blue
/// polyline scaled by `scale`, nodes are black dots, supported nodes are green
/// triangles and nodal loads are red arrows. The `viewBox` is fixed at
/// 960 × 720 and fitted to the projected model with padding; a title and a
/// legend naming the scale frame the drawing. The file contains no timestamps,
/// so repeated exports of the same model are byte-identical.
///
/// `scale` must be finite and positive. A model with no nodes, or a solution
/// that does not match the model, is rejected with a stable error instead of
/// writing a meaningless drawing.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise `path`
/// is treated as a directory and `model.svg` is created inside it.
pub fn export_svg(model: &Model, solution: &Solution, scale: f64, path: &Path) -> Result<(), CaeError> {
    export_svg_with_contour(model, solution, scale, None, 11, path)
}

/// Writes a deterministic SVG drawing that can colour the members by a result
/// quantity instead of drawing a plain deformed shape.
///
/// With `contour == None` the output is exactly the drawing of
/// [`export_svg`]. With a quantity the members of the deformed model are drawn
/// as coloured segments: blue is the minimum, white is zero and red is the
/// maximum, using the sampled values at `stations` points per member. A colour
/// bar below the model names the quantity, its unit and the sampled minimum and
/// maximum, so the drawing and the numbers of `internalForces` (or the nodal
/// displacement magnitudes) agree. `stations` counts both element ends and must
/// be at least `2`; it only affects a contour export.
pub fn export_svg_with_contour(
    model: &Model,
    solution: &Solution,
    scale: f64,
    contour: Option<ContourQuantity>,
    stations: usize,
    path: &Path,
) -> Result<(), CaeError> {
    if model.nodes.is_empty() {
        return Err(CaeError::new("export/no-solution", "the model has no nodes, so there is no shape to draw"));
    }
    check_alignment(model, solution)?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(CaeError::new(
            "export/invalid-scale",
            format!("the deformed-shape scale must be a finite positive number, but is {scale}"),
        ));
    }
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join("model.svg")
    };

    let undeformed: Vec<(f64, f64)> = model.nodes.iter().map(|node| (node.x, node.y)).collect();
    let deformed: Vec<(f64, f64)> = model
        .nodes
        .iter()
        .zip(&solution.nodes)
        .map(|(node, result)| (node.x + scale * result.ux, node.y + scale * result.uy))
        .collect();
    let series = contour.map(|quantity| contour_series(model, solution, &deformed, quantity, stations)).transpose()?;

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &(x, y) in undeformed.iter().chain(&deformed) {
        if !x.is_finite() || !y.is_finite() {
            return Err(CaeError::new("export/invalid-model", "node coordinates must be finite before an SVG can be drawn"));
        }
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let mut width = max_x - min_x;
    let mut height = max_y - min_y;
    if width <= 0.0 {
        width = 1.0;
        min_x -= 0.5;
    }
    if height <= 0.0 {
        height = 1.0;
        min_y -= 0.5;
    }
    let max_y = min_y + height;

    const VIEW_WIDTH: f64 = 960.0;
    const VIEW_HEIGHT: f64 = 720.0;
    const SIDE_MARGIN: f64 = 40.0;
    const TOP_MARGIN: f64 = 60.0;
    const PLOT_BOTTOM: f64 = 630.0;
    let available_width = VIEW_WIDTH - 2.0 * SIDE_MARGIN;
    let available_height = PLOT_BOTTOM - TOP_MARGIN;
    let factor = (available_width / width).min(available_height / height);
    let origin_x = SIDE_MARGIN + (available_width - width * factor) / 2.0;
    let origin_y = TOP_MARGIN + (available_height - height * factor) / 2.0;
    let project = |x: f64, y: f64| (origin_x + (x - min_x) * factor, origin_y + (max_y - y) * factor);

    let scale_label = format!("{scale:.2}");
    let heading = if contour.is_some() { "OpenCAE model and contour" } else { "OpenCAE model and deformed shape" };
    let mut text = String::new();
    text.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        text,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {VIEW_WIDTH} {VIEW_HEIGHT}\" width=\"960\" height=\"720\" \
         role=\"img\" aria-labelledby=\"ocae-svg-title\">"
    );
    let _ = writeln!(text, "<title id=\"ocae-svg-title\">{heading}</title>");
    text.push_str("<rect x=\"0\" y=\"0\" width=\"960\" height=\"720\" fill=\"#ffffff\"/>\n");
    let _ = writeln!(
        text,
        "<text x=\"480\" y=\"30\" text-anchor=\"middle\" font-family=\"system-ui, sans-serif\" font-size=\"20\" \
         fill=\"#111827\">{heading}</text>"
    );
    text.push_str(
        "<defs>\n<marker id=\"ocae-load-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" \
         markerHeight=\"6\" orient=\"auto\" markerUnits=\"strokeWidth\">\n<path d=\"M0,0 L10,5 L0,10 z\" \
         fill=\"#dc2626\"/>\n</marker>\n</defs>\n",
    );
    text.push_str("<g stroke-linecap=\"round\">\n");

    let mut endpoints = Vec::with_capacity(model.elements.len());
    for element in &model.elements {
        let from = model
            .nodes
            .iter()
            .position(|node| node.id == element.from)
            .ok_or_else(|| missing_svg_node(&element.id, &element.from))?;
        let to = model
            .nodes
            .iter()
            .position(|node| node.id == element.to)
            .ok_or_else(|| missing_svg_node(&element.id, &element.to))?;
        endpoints.push((from, to));
    }
    for &(from, to) in &endpoints {
        let (x1, y1) = project(undeformed[from].0, undeformed[from].1);
        let (x2, y2) = project(undeformed[to].0, undeformed[to].1);
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#9ca3af\" stroke-width=\"1\"/>"
        );
    }
    if let Some(series) = &series {
        let limit = series.min.abs().max(series.max.abs());
        for segment in &series.segments {
            let (x1, y1) = project(segment.x1, segment.y1);
            let (x2, y2) = project(segment.x2, segment.y2);
            let color = contour_color(segment.value, limit);
            let _ = writeln!(
                text,
                "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{color}\" stroke-width=\"4\"/>"
            );
        }
    }
    else {
        for &(from, to) in &endpoints {
            let (x1, y1) = project(deformed[from].0, deformed[from].1);
            let (x2, y2) = project(deformed[to].0, deformed[to].1);
            let _ = writeln!(
                text,
                "<polyline points=\"{x1:.2},{y1:.2} {x2:.2},{y2:.2}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\"/>"
            );
        }
    }
    for &(x, y) in &undeformed {
        let (cx, cy) = project(x, y);
        let _ = writeln!(text, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"3\" fill=\"#111827\"/>");
    }
    for support in &model.supports {
        let constrained = support.dx
            || support.dy
            || support.dz
            || support.rz
            || support.spring.is_some()
            || support.direction.is_some()
            || support.prescribed.is_some();
        if !constrained {
            continue;
        }
        let Some(index) = model.nodes.iter().position(|node| node.id == support.node)
        else {
            continue;
        };
        let (cx, cy) = project(undeformed[index].0, undeformed[index].1);
        let _ = writeln!(
            text,
            "<polygon points=\"{cx:.2},{cy:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"#16a34a\"/>",
            cx - 7.0,
            cy + 11.0,
            cx + 7.0,
            cy + 11.0
        );
    }
    for load in &model.loads {
        let magnitude = (load.fx * load.fx + load.fy * load.fy).sqrt();
        if magnitude <= 0.0 || !magnitude.is_finite() {
            continue;
        }
        let Some(index) = model.nodes.iter().position(|node| node.id == load.node)
        else {
            continue;
        };
        let (cx, cy) = project(undeformed[index].0, undeformed[index].1);
        let direction_x = load.fx / magnitude;
        let direction_y = -load.fy / magnitude;
        let arrow_length = 34.0;
        let x1 = cx - direction_x * arrow_length;
        let y1 = cy - direction_y * arrow_length;
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{cx:.2}\" y2=\"{cy:.2}\" stroke=\"#dc2626\" \
             stroke-width=\"2\" marker-end=\"url(#ocae-load-arrow)\"/>"
        );
    }
    text.push_str("</g>\n");

    if let (Some(quantity), Some(series)) = (contour, &series) {
        write_contour_legend(&mut text, quantity, series, model.units.unwrap_or(Units::Millimeter));
    }
    let legend_y = if contour.is_some() { 700.0 } else { 686.0 };
    let legend_text_y = legend_y + 4.0;
    text.push_str("<g font-family=\"system-ui, sans-serif\" font-size=\"13\" fill=\"#111827\">\n");
    let _ = writeln!(
        text,
        "<line x1=\"40\" y1=\"{legend_y}\" x2=\"70\" y2=\"{legend_y}\" stroke=\"#9ca3af\" stroke-width=\"1\"/>\n\
         <text x=\"78\" y=\"{legend_text_y}\">undeformed</text>"
    );
    let deformed_label = if contour.is_some() { "contour" } else { "deformed" };
    let _ = writeln!(
        text,
        "<line x1=\"190\" y1=\"{legend_y}\" x2=\"220\" y2=\"{legend_y}\" stroke=\"#2563eb\" stroke-width=\"2\"/>\n\
         <text x=\"228\" y=\"{legend_text_y}\">{deformed_label} (scale {scale_label})</text>"
    );
    let _ = writeln!(
        text,
        "<polygon points=\"400,{:.2} 392,{:.2} 408,{:.2}\" fill=\"#16a34a\"/>\n\
         <text x=\"416\" y=\"{legend_text_y}\">support</text>",
        legend_y - 6.0,
        legend_y + 8.0,
        legend_y + 8.0
    );
    let _ = writeln!(
        text,
        "<line x1=\"520\" y1=\"{legend_y}\" x2=\"550\" y2=\"{legend_y}\" stroke=\"#dc2626\" stroke-width=\"2\" \
         marker-end=\"url(#ocae-load-arrow)\"/>\n<text x=\"558\" y=\"{legend_text_y}\">load</text>"
    );
    let _ = writeln!(
        text,
        "<text x=\"920\" y=\"{legend_text_y}\" text-anchor=\"end\">projection: XY (Z ignored)</text>\n</g>\n</svg>\n"
    );

    write_atomic(&target, &text)
}

/// One straight coloured piece of a contour drawing, in model coordinates.
struct ContourSegment {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    value: f64,
}

/// Coloured segments plus the sampled minimum and maximum of a contour.
struct ContourSeries {
    segments: Vec<ContourSegment>,
    min: f64,
    max: f64,
}

/// Samples `quantity` at `stations` points per member and builds the segments
/// along the deformed centreline.
///
/// Each segment is coloured by the endpoint with the larger magnitude, so a
/// peak at a station always colours the adjacent segment instead of being
/// averaged away. `min` and `max` are the sampled extremes, which are exactly
/// the values printed in the legend.
fn contour_series(
    model: &Model,
    solution: &Solution,
    deformed: &[(f64, f64)],
    quantity: ContourQuantity,
    stations: usize,
) -> Result<ContourSeries, CaeError> {
    let stations = stations.max(2);
    let mut segments = Vec::new();
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut displacement_extremes = None;
    let mut force_extremes = None;
    let mut push = |x1: f64, y1: f64, x2: f64, y2: f64, value: f64| {
        min = min.min(value);
        max = max.max(value);
        segments.push(ContourSegment { x1, y1, x2, y2, value });
    };

    if quantity == ContourQuantity::Displacement {
        let magnitudes: Vec<f64> = solution.nodes.iter().map(|node| node.magnitude).collect();
        displacement_extremes = Some((
            magnitudes.iter().copied().fold(f64::INFINITY, f64::min),
            magnitudes.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        ));
        let divisions = stations - 1;
        for element in &model.elements {
            let from = model
                .nodes
                .iter()
                .position(|node| node.id == element.from)
                .ok_or_else(|| missing_svg_node(&element.id, &element.from))?;
            let to = model
                .nodes
                .iter()
                .position(|node| node.id == element.to)
                .ok_or_else(|| missing_svg_node(&element.id, &element.to))?;
            for index in 0..divisions {
                let start = index as f64 / divisions as f64;
                let end = (index + 1) as f64 / divisions as f64;
                let (x1, y1) = interpolate(deformed[from], deformed[to], start);
                let (x2, y2) = interpolate(deformed[from], deformed[to], end);
                let value = interpolate_value(magnitudes[from], magnitudes[to], start)
                    .abs()
                    .max(interpolate_value(magnitudes[from], magnitudes[to], end).abs());
                push(x1, y1, x2, y2, value);
            }
        }
    }
    else {
        let forces = solve_internal_forces(model, stations)?;
        let mut extremes: Option<(f64, f64)> = None;
        for (element, diagram) in model.elements.iter().zip(&forces.elements) {
            let from = model
                .nodes
                .iter()
                .position(|node| node.id == element.from)
                .ok_or_else(|| missing_svg_node(&element.id, &element.from))?;
            let to = model
                .nodes
                .iter()
                .position(|node| node.id == element.to)
                .ok_or_else(|| missing_svg_node(&element.id, &element.to))?;
            if !diagram.length.is_finite() || diagram.length <= 0.0 {
                continue;
            }
            let mut low = f64::INFINITY;
            let mut high = f64::NEG_INFINITY;
            for station in &diagram.stations {
                let value = station_value(station, quantity);
                low = low.min(value);
                high = high.max(value);
            }
            extremes = Some(match extremes {
                None => (low, high),
                Some((minimum, maximum)) => (minimum.min(low), maximum.max(high)),
            });
            for window in diagram.stations.windows(2) {
                let start = (window[0].position / diagram.length).clamp(0.0, 1.0);
                let end = (window[1].position / diagram.length).clamp(0.0, 1.0);
                let (first, second) = (station_value(&window[0], quantity), station_value(&window[1], quantity));
                let value = if first.abs() >= second.abs() { first } else { second };
                let (x1, y1) = interpolate(deformed[from], deformed[to], start);
                let (x2, y2) = interpolate(deformed[from], deformed[to], end);
                push(x1, y1, x2, y2, value);
            }
        }
        force_extremes = extremes;
    }

    if segments.is_empty() {
        min = 0.0;
        max = 0.0;
    }
    if let Some((low, high)) = displacement_extremes.or(force_extremes) {
        min = low;
        max = high;
    }
    Ok(ContourSeries { segments, min, max })
}

fn station_value(station: &crate::solver::SectionForces, quantity: ContourQuantity) -> f64 {
    match quantity {
        ContourQuantity::Axial => station.axial_force,
        ContourQuantity::Shear => station.shear,
        ContourQuantity::Moment => station.moment,
        ContourQuantity::Displacement => 0.0,
    }
}

fn interpolate(start: (f64, f64), end: (f64, f64), fraction: f64) -> (f64, f64) {
    (start.0 + (end.0 - start.0) * fraction, start.1 + (end.1 - start.1) * fraction)
}

fn interpolate_value(start: f64, end: f64, fraction: f64) -> f64 {
    start + (end - start) * fraction
}

/// Maps a value onto a blue–white–red diverging scale normalised by `limit`.
fn contour_color(value: f64, limit: f64) -> String {
    const NEGATIVE: (f64, f64, f64) = (29.0, 78.0, 216.0);
    const NEUTRAL: (f64, f64, f64) = (248.0, 250.0, 252.0);
    const POSITIVE: (f64, f64, f64) = (185.0, 28.0, 28.0);
    let ratio = if limit.is_finite() && limit > 0.0 { (value / limit).clamp(-1.0, 1.0) } else { 0.0 };
    let (from, to, t) = if ratio < 0.0 { (NEGATIVE, NEUTRAL, 1.0 + ratio) } else { (NEUTRAL, POSITIVE, ratio) };
    let blend = |a: f64, b: f64| (a + (b - a) * t).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", blend(from.0, to.0), blend(from.1, to.1), blend(from.2, to.2))
}

fn format_contour_value(value: f64) -> String {
    let magnitude = value.abs();
    if value == 0.0 {
        "0".to_string()
    }
    else if magnitude >= 1.0e7 || magnitude < 1.0e-3 {
        format!("{value:.3e}")
    }
    else if magnitude < 1.0 {
        format!("{value:.4}")
    }
    else {
        format!("{value:.3}")
    }
}

fn write_contour_legend(text: &mut String, quantity: ContourQuantity, series: &ContourSeries, units: Units) {
    const BAR_LEFT: f64 = 600.0;
    const BAR_RIGHT: f64 = 920.0;
    const BAR_TOP: f64 = 644.0;
    const BAR_HEIGHT: f64 = 12.0;
    const STEPS: usize = 12;
    let limit = series.min.abs().max(series.max.abs());
    // Solver round-off leaves values such as -1e-10 where the analytical value
    // is zero; snapping them keeps the legend readable without hiding a real
    // extreme (the tolerance is relative to the largest sampled magnitude).
    let snap = |value: f64| if limit > 0.0 && value.abs() <= 1.0e-12 * limit { 0.0 } else { value };
    let minimum = snap(series.min);
    let maximum = snap(series.max);
    let _ = writeln!(
        text,
        "<g font-family=\"system-ui, sans-serif\" font-size=\"13\" fill=\"#111827\">\n\
         <text x=\"{BAR_LEFT}\" y=\"638\">contour: {} ({})</text>",
        quantity.label(),
        quantity.unit(units)
    );
    let width = (BAR_RIGHT - BAR_LEFT) / STEPS as f64;
    for step in 0..STEPS {
        let ratio = -1.0 + 2.0 * step as f64 / (STEPS - 1) as f64;
        let color = contour_color(ratio * limit, limit);
        let x = BAR_LEFT + step as f64 * width;
        let _ = writeln!(
            text,
            "<rect x=\"{x:.2}\" y=\"{BAR_TOP}\" width=\"{width:.2}\" height=\"{BAR_HEIGHT}\" fill=\"{color}\"/>"
        );
    }
    let _ = writeln!(text, "<text x=\"{BAR_LEFT}\" y=\"676\" font-size=\"11\">{}</text>", format_contour_value(minimum));
    if minimum < 0.0 && maximum > 0.0 {
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"676\" font-size=\"11\" text-anchor=\"middle\">0</text>",
            (BAR_LEFT + BAR_RIGHT) / 2.0
        );
    }
    let _ = writeln!(
        text,
        "<text x=\"{BAR_RIGHT}\" y=\"676\" font-size=\"11\" text-anchor=\"end\">{}</text>\n</g>",
        format_contour_value(maximum)
    );
}

fn missing_svg_node(element: &str, node: &str) -> CaeError {
    CaeError::new("export/invalid-model", format!("SVG export cannot find node `{node}` for element `{element}`"))
}

/// Writes a self-contained, deterministic HTML analysis report.
///
/// The report mirrors the Markdown sections (model summary, validation,
/// solution, nodal and element results and modal data) and embeds its CSS so the
/// single file can be opened in any browser. It contains no timestamps, so
/// repeated exports of the same model are byte-identical.
///
/// When `path` ends in `.html` or `.htm` it is used as the output file,
/// otherwise `path` is treated as a directory and `report.html` is created
/// inside it.
pub fn export_html(
    model: &Model,
    solution: &Solution,
    modal: Option<&ModalSolution>,
    issues: &[Issue],
    path: &Path,
) -> Result<(), CaeError> {
    check_alignment(model, solution)?;
    let extension = path.extension().and_then(|extension| extension.to_str());
    let target =
        if extension == Some("html") || extension == Some("htm") { path.to_path_buf() } else { path.join("report.html") };

    let mut text = String::new();
    text.push_str(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>OpenCAE Analysis Report</title>\n<style>\n\
         body{font-family:system-ui,-apple-system,'Segoe UI',sans-serif;margin:2rem;color:#1b1f24;background:#fff}\n\
         h1{font-size:1.6rem;margin-bottom:1.5rem}\n\
         h2{font-size:1.15rem;margin-top:2rem;border-bottom:1px solid #d8dee4;padding-bottom:.3rem}\n\
         table{border-collapse:collapse;margin:.75rem 0;font-size:.9rem}\n\
         th,td{border:1px solid #d8dee4;padding:.35rem .6rem;text-align:left}\n\
         th{background:#f3f5f7}\ncode{font-family:ui-monospace,SFMono-Regular,Consolas,monospace}\n\
         .passed{color:#17691f}.error{color:#a4161a}.warning{color:#8a5a00}.notice{color:#3b5bdb}\n\
         </style>\n</head>\n<body>\n<h1>OpenCAE Analysis Report</h1>\n",
    );

    text.push_str("<section>\n<h2>Model Summary</h2>\n<table>\n<tr><th>Item</th><th>Value</th></tr>\n");
    let _ = writeln!(
        text,
        "<tr><td>Document format</td><td><code>{} v{}</code></td></tr>",
        escape_html(&model.format),
        model.version
    );
    let _ = writeln!(text, "<tr><td>Dimension</td><td>{}D</td></tr>", model.dimension);
    let _ = writeln!(text, "<tr><td>Units</td><td>{}</td></tr>", model.units.unwrap_or(Units::Millimeter).as_str());
    let _ = writeln!(text, "<tr><td>Nodes</td><td>{}</td></tr>", model.nodes.len());
    let _ = writeln!(text, "<tr><td>Elements</td><td>{}</td></tr>", model.elements.len());
    let _ = writeln!(
        text,
        "<tr><td>Element types</td><td>truss {} / beam {}</td></tr>",
        model.elements.iter().filter(|element| element.kind == ElementKind::Truss).count(),
        model.elements.iter().filter(|element| element.kind == ElementKind::Beam).count()
    );
    let _ = writeln!(text, "<tr><td>Support entries</td><td>{}</td></tr>", model.supports.len());
    let _ = writeln!(text, "<tr><td>Concentrated load entries</td><td>{}</td></tr>", model.loads.len());
    let _ = writeln!(text, "<tr><td>Material</td><td>{}</td></tr>", escape_html(&model.material.name));
    let _ = writeln!(text, "<tr><td>E</td><td>{}</td></tr>", format_float(model.material.youngs_modulus));
    let _ = writeln!(text, "<tr><td>A</td><td>{}</td></tr>", format_float(model.material.area));
    let _ = writeln!(text, "<tr><td>density</td><td>{}</td></tr>", format_float(model.material.density));
    match model.gravity {
        Some(gravity) => {
            let _ = writeln!(
                text,
                "<tr><td>gravity</td><td>[{}, {}, {}]</td></tr>",
                format_float(gravity[0]),
                format_float(gravity[1]),
                format_float(gravity[2])
            );
        }
        None => text.push_str("<tr><td>gravity</td><td>not set</td></tr>\n"),
    }
    text.push_str("</table>\n</section>\n");

    text.push_str("<section>\n<h2>Validation</h2>\n");
    if issues.is_empty() {
        text.push_str("<p class=\"passed\">Passed, no issues found.</p>\n");
    }
    else {
        text.push_str("<table>\n<tr><th>Severity</th><th>Code</th><th>Message</th></tr>\n");
        for issue in issues {
            let severity = match issue.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Notice => "notice",
            };
            let _ = writeln!(
                text,
                "<tr><td class=\"{severity}\">{severity}</td><td><code>{}</code></td><td>{}</td></tr>",
                escape_html(&issue.code),
                escape_html(&issue.message)
            );
        }
        text.push_str("</table>\n");
    }
    text.push_str("</section>\n");

    text.push_str("<section>\n<h2>Solution Summary</h2>\n<table>\n<tr><th>Item</th><th>Value</th></tr>\n");
    let _ = writeln!(
        text,
        "<tr><td>Max displacement</td><td>{} (node <code>{}</code>, component norm)</td></tr>",
        format_float(solution.max_displacement.magnitude),
        escape_html(&solution.max_displacement.id)
    );
    let _ = writeln!(
        text,
        "<tr><td>Max axial force</td><td>{} (element <code>{}</code>)</td></tr>",
        format_float(solution.max_axial_force.axial_force),
        escape_html(&solution.max_axial_force.element)
    );
    let _ = writeln!(
        text,
        "<tr><td>Max bending moment</td><td>{} (element <code>{}</code>)</td></tr>",
        format_float(solution.max_moment.moment),
        escape_html(&solution.max_moment.element)
    );
    let _ = writeln!(text, "<tr><td>Total strain energy</td><td>{}</td></tr>", format_float(solution.strain_energy));
    text.push_str("</table>\n</section>\n");

    text.push_str(
        "<section>\n<h2>Nodal Displacements</h2>\n<table>\n\
         <tr><th>Node</th><th>ux</th><th>uy</th><th>uz</th><th>rz</th><th>Magnitude</th></tr>\n",
    );
    for node in &solution.nodes {
        let _ = writeln!(
            text,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&node.id),
            format_float(node.ux),
            format_float(node.uy),
            format_float(node.uz),
            format_float(node.rz),
            format_float(node.magnitude)
        );
    }
    text.push_str("</table>\n</section>\n");

    text.push_str(
        "<section>\n<h2>Nodal Reactions</h2>\n<table>\n<tr><th>Node</th><th>fx</th><th>fy</th><th>fz</th><th>mz</th></tr>\n",
    );
    for reaction in &solution.reactions {
        let _ = writeln!(
            text,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&reaction.node),
            format_float(reaction.fx),
            format_float(reaction.fy),
            format_float(reaction.fz),
            format_float(reaction.mz)
        );
    }
    text.push_str("</table>\n</section>\n");

    text.push_str(
        "<section>\n<h2>Element Results</h2>\n<table>\n<tr><th>Element</th><th>Type</th><th>Length</th>\
         <th>Axial force</th><th>Stress</th><th>maxAbsMoment</th><th>n1</th><th>v1</th><th>m1</th>\
         <th>n2</th><th>v2</th><th>m2</th></tr>\n",
    );
    for element in &solution.elements {
        let _ = writeln!(
            text,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>\
             <td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&element.id),
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
    text.push_str("</table>\n</section>\n");

    text.push_str("<section>\n<h2>Modal Results</h2>\n");
    match modal {
        Some(modal) => {
            let _ = writeln!(
                text,
                "<p>Requested {} modes, returned {}, skipped {} zero/negative eigenvalues.</p>",
                modal.requested,
                modal.modes.len(),
                modal.skipped
            );
            if modal.modes.is_empty() {
                text.push_str("<p>No positive eigenvalues available.</p>\n");
            }
            else {
                text.push_str(
                    "<table>\n<tr><th>Mode</th><th>Frequency (Hz)</th><th>Eigenvalue</th><th>Max amplitude</th></tr>\n",
                );
                for (index, mode) in modal.modes.iter().enumerate() {
                    let _ = writeln!(
                        text,
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        index + 1,
                        format_float(mode.frequency_hz),
                        format_float(mode.eigenvalue),
                        format_float(mode.max_amplitude)
                    );
                }
                text.push_str("</table>\n");
            }
        }
        None => text.push_str("<p>Not run (requires a positive density).</p>\n"),
    }
    text.push_str("</section>\n</body>\n</html>\n");

    write_atomic(&target, &text)
}

fn escape_html(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
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

pub(crate) fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    }
    else {
        value.to_string()
    }
}
