use std::{fmt::Write as _, fs, path::Path, str::FromStr};

use science_mesh::{Cell, CellKind, Field, Mesh, MeshError};

use crate::{
    CaeError,
    envelope::{EnvelopeElement, EnvelopeExtreme, EnvelopeReport},
    model::{ElementKind, Model, Support, Units},
    solver::{ElementDiagram, ModalSolution, Solution, solve_internal_forces},
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
///
/// For documents that contain `beam3d` members the tables gain the extra
/// columns `rx,ry` (`nodes.csv`), `mx,my` (`reactions.csv`) and
/// `torque,shearZ,momentY` (`forces.csv`); two-dimensional exports are
/// unchanged.
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

    // `beam3d` documents carry rotational components that the two-dimensional
    // tables never see; they are appended as extra columns only when present,
    // so existing truss and beam exports stay byte-identical.
    let spatial_nodes = solution.nodes.iter().any(|displacement| displacement.rx.is_some());
    let mut nodes = String::from("id,x,y,z,ux,uy,uz");
    if spatial_nodes {
        nodes.push_str(",rx,ry");
    }
    nodes.push('\n');
    for (node, displacement) in model.nodes.iter().zip(&solution.nodes) {
        let _ = writeln!(
            nodes,
            "{},{},{},{},{},{},{}{}",
            csv_field(&node.id),
            node.x,
            node.y,
            node.z,
            displacement.ux,
            displacement.uy,
            displacement.uz,
            if spatial_nodes {
                format!(",{},{}", displacement.rx.unwrap_or_default(), displacement.ry.unwrap_or_default())
            }
            else {
                String::new()
            }
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

    let spatial_reactions = solution.reaction_summary.supports.iter().any(|support| support.mx.is_some());
    let mut reactions = String::from("node,fx,fy,fz,mz");
    if spatial_reactions {
        reactions.push_str(",mx,my");
    }
    reactions.push('\n');
    for support in &solution.reaction_summary.supports {
        let _ = writeln!(
            reactions,
            "{},{},{},{},{}{}",
            csv_field(&support.node),
            support.fx,
            support.fy,
            support.fz,
            support.mz,
            if spatial_reactions {
                format!(",{},{}", support.mx.unwrap_or_default(), support.my.unwrap_or_default())
            }
            else {
                String::new()
            }
        );
    }

    let diagrams = solve_internal_forces(model, stations)?;
    let spatial_forces =
        diagrams.elements.iter().any(|element| element.stations.iter().any(|station| station.torque.is_some()));
    let mut forces = String::from("element,from,to,kind,position,axialForce,shear,moment");
    if spatial_forces {
        forces.push_str(",torque,shearZ,momentY");
    }
    forces.push('\n');
    for element in &diagrams.elements {
        for station in &element.stations {
            let _ = writeln!(
                forces,
                "{},{},{},{},{},{},{},{}{}",
                csv_field(&element.id),
                csv_field(&element.from),
                csv_field(&element.to),
                element.kind.as_str(),
                station.position,
                station.axial_force,
                station.shear,
                station.moment,
                if spatial_forces {
                    format!(
                        ",{},{},{}",
                        station.torque.unwrap_or_default(),
                        station.shear_z.unwrap_or_default(),
                        station.moment_y.unwrap_or_default()
                    )
                }
                else {
                    String::new()
                }
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
        "| Element types | truss {} / beam {} / beam3d {} |",
        model.elements.iter().filter(|element| element.kind == ElementKind::Truss).count(),
        model.elements.iter().filter(|element| element.kind == ElementKind::Beam).count(),
        model.elements.iter().filter(|element| element.kind == ElementKind::Beam3D).count()
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

    // `beam3d` results carry rotational components that the two-dimensional
    // tables never see; they are appended as extra columns only when present.
    let spatial_nodes = solution.nodes.iter().any(|node| node.rx.is_some());
    text.push_str("\n## Nodal Displacements\n\n");
    text.push_str(if spatial_nodes {
        "| Node | ux | uy | uz | rx | ry | rz | Magnitude |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n"
    }
    else {
        "| Node | ux | uy | uz | rz | Magnitude |\n| --- | --- | --- | --- | --- | --- |\n"
    });
    for node in &solution.nodes {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {}{} | {} |",
            escape_markdown(&node.id),
            format_float(node.ux),
            format_float(node.uy),
            format_float(node.uz),
            if spatial_nodes {
                format!("{} | {} | ", node.rx.unwrap_or_default(), node.ry.unwrap_or_default())
            }
            else {
                String::new()
            },
            format_float(node.rz),
            format_float(node.magnitude)
        );
    }

    let spatial_reactions = solution.reactions.iter().any(|reaction| reaction.mx.is_some());
    text.push_str("\n## Nodal Reactions\n\n");
    text.push_str(if spatial_reactions {
        "| Node | fx | fy | fz | mx | my | mz |\n| --- | --- | --- | --- | --- | --- | --- |\n"
    }
    else {
        "| Node | fx | fy | fz | mz |\n| --- | --- | --- | --- | --- |\n"
    });
    for reaction in &solution.reactions {
        let _ = writeln!(
            text,
            "| {} | {} | {} | {} | {}{} |",
            escape_markdown(&reaction.node),
            format_float(reaction.fx),
            format_float(reaction.fy),
            format_float(reaction.fz),
            if spatial_reactions {
                format!("{} | {} | ", reaction.mx.unwrap_or_default(), reaction.my.unwrap_or_default())
            }
            else {
                String::new()
            },
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

/// Engineering drawing kind an SVG export can produce instead of the plain
/// deformed-shape or contour drawing.
///
/// Every kind is a dedicated multi-panel or annotated drawing built from the
/// same solver reports the CSV and JSON results use, so the printed numbers
/// cannot drift from the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingKind {
    /// Shear `V(x)` and bending-moment `M(x)` diagrams per beam-like member.
    Forces,
    /// Filled shear-force `V(x)` diagram drawn on the member axis, with end
    /// values and the extreme labelled. Truss members are skipped because a
    /// two-force member carries no shear.
    Shear,
    /// Filled bending-moment `M(x)` diagram drawn on the member axis, with
    /// end values and the extreme labelled. Truss members are skipped.
    Moment,
    /// Nodal displacement vectors drawn at the deformed nodes with their
    /// magnitudes labelled.
    Displacement,
    /// Support reaction vectors drawn at the supported nodes with their
    /// magnitudes and reaction moments labelled.
    Reactions,
    /// Undeformed skeleton overlaid with the deformed centreline of the
    /// static solution, without arrows or labels on every node.
    Overlay,
    /// Mode shape panels of the lowest natural frequencies.
    Modal,
    /// Min/max envelope bars of the axial force, shear and moment of every
    /// member across the solved load combinations.
    Envelope,
    /// One-page result summary: model size, load cases, extreme results, a
    /// thumbnail sketch of the model and the shared title block.
    Summary,
}

impl DrawingKind {
    /// Returns the short identifier used by the CLI, the addon and the SDK.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Forces => "forces",
            Self::Shear => "shear",
            Self::Moment => "moment",
            Self::Displacement => "displacement",
            Self::Reactions => "reactions",
            Self::Overlay => "overlay",
            Self::Modal => "modal",
            Self::Envelope => "envelope",
            Self::Summary => "summary",
        }
    }

    /// Returns the title drawn at the top of the SVG.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Forces => "OpenCAE shear and moment diagrams",
            Self::Shear => "OpenCAE shear force diagram",
            Self::Moment => "OpenCAE bending moment diagram",
            Self::Displacement => "OpenCAE nodal displacement vectors",
            Self::Reactions => "OpenCAE support reaction vectors",
            Self::Overlay => "OpenCAE deformation overlay",
            Self::Modal => "OpenCAE mode shapes",
            Self::Envelope => "OpenCAE force envelope",
            Self::Summary => "OpenCAE analysis summary",
        }
    }

    /// Returns the drawing number printed in the title block.
    pub const fn number(self) -> &'static str {
        match self {
            Self::Forces => "OCAE-DRW-01",
            Self::Shear => "OCAE-DRW-08",
            Self::Moment => "OCAE-DRW-09",
            Self::Displacement => "OCAE-DRW-02",
            Self::Reactions => "OCAE-DRW-03",
            Self::Overlay => "OCAE-DRW-04",
            Self::Modal => "OCAE-DRW-05",
            Self::Envelope => "OCAE-DRW-06",
            Self::Summary => "OCAE-DRW-07",
        }
    }

    /// Default file name used when the export target is a directory.
    pub const fn default_file_name(self) -> &'static str {
        match self {
            Self::Forces => "forces.svg",
            Self::Shear => "shear.svg",
            Self::Moment => "moment.svg",
            Self::Displacement => "displacements.svg",
            Self::Reactions => "reactions.svg",
            Self::Overlay => "overlay.svg",
            Self::Modal => "modes.svg",
            Self::Envelope => "envelope.svg",
            Self::Summary => "summary.svg",
        }
    }
}

impl FromStr for DrawingKind {
    type Err = CaeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "forces" => Ok(Self::Forces),
            "shear" => Ok(Self::Shear),
            "moment" => Ok(Self::Moment),
            "displacement" => Ok(Self::Displacement),
            "reactions" => Ok(Self::Reactions),
            "overlay" => Ok(Self::Overlay),
            "modal" => Ok(Self::Modal),
            "envelope" => Ok(Self::Envelope),
            "summary" => Ok(Self::Summary),
            other => Err(CaeError::new(
                "export/invalid-drawing",
                format!(
                    "unsupported drawing kind `{other}`; valid values are `forces`, `shear`, `moment`, `displacement`, `reactions`, `overlay`, `modal`, `envelope` or `summary`"
                ),
            )),
        }
    }
}

// --- shared sheet layout -------------------------------------------------

/// Width of every exported sheet in SVG user units, equal to the PDF page
/// width in points.
const SHEET_WIDTH: f64 = 960.0;
/// Height of every exported sheet in SVG user units.
const SHEET_HEIGHT: f64 = 720.0;
/// The single font family used by every sheet, so headings, table cells,
/// labels and notes share one typographic voice.
const FONT_FAMILY: &str = "Inter, 'Segoe UI', system-ui, -apple-system, sans-serif";
/// Three sizes, one per hierarchy level: sheet heading, body/table text and
/// annotations/footnotes. Panels may interpolate between them, never invent a
/// fourth voice.
const FONT_TITLE: f64 = 19.0;
const FONT_BODY: f64 = 11.5;
const FONT_LABEL: f64 = 10.0;
const FONT_NOTE: f64 = 9.0;
/// Inner page border, inset from the page edge.
const FRAME_LEFT: f64 = 12.0;
const FRAME_TOP: f64 = 12.0;
const FRAME_RIGHT: f64 = 948.0;
const FRAME_BOTTOM: f64 = 708.0;
/// Plot area inside the frame. Content is centred in it and the shared margin
/// keeps the model clear of the frame on every side.
const PLOT_LEFT: f64 = 80.0;
const PLOT_RIGHT: f64 = 880.0;
const PLOT_TOP: f64 = 62.0;
const PLOT_BOTTOM: f64 = 592.0;
/// Top of the title block occupying the bottom strip of the frame. The block
/// reserves a fixed label row, value row and a two-line note band, so no note
/// can spill past the frame.
const TITLE_BLOCK_TOP: f64 = 622.0;
/// Geometry viewport inside the plot area. The extra inset keeps support
/// symbols and node labels away from the frame, not merely the node dots.
const VIEW_LEFT: f64 = 96.0;
const VIEW_RIGHT: f64 = 864.0;
const VIEW_TOP: f64 = 78.0;
const VIEW_BOTTOM: f64 = 584.0;
/// Baseline of the single legend row drawn above the title block.
const LEGEND_BASELINE: f64 = 608.0;
/// Left, right and vertical centre of the title block cells. The drawing name
/// cell is the widest, the numeric cells share the remaining width.
const TITLE_SPLITS: [f64; 4] = [470.0, 620.0, 726.0, 844.0];
/// Title block rows: label baseline, value baseline and the divider above the
/// note band.
const TITLE_LABEL_BASELINE: f64 = 637.0;
const TITLE_VALUE_BASELINE: f64 = 657.0;
const TITLE_NOTE_DIVIDER: f64 = 667.0;
/// The two note baselines inside the title block. Both plus the font descent
/// stay above `FRAME_BOTTOM`.
const TITLE_NOTE_BASELINES: [f64; 2] = [684.0, 701.0];

/// Static text of the drawing title block.
struct SheetMeta<'a> {
    title: &'a str,
    number: &'a str,
    units: &'static str,
    scale: String,
    load_case: String,
}

impl<'a> SheetMeta<'a> {
    fn new(model: &Model, title: &'a str, number: &'a str, scale: impl Into<String>) -> Self {
        Self {
            title,
            number,
            units: unit_label(model.units.unwrap_or(Units::Millimeter)),
            scale: scale.into(),
            load_case: if model.combinations.is_empty() {
                "static".to_string()
            }
            else {
                format!("{} combinations", model.combinations.len())
            },
        }
    }
}

/// Units printed in the title block; forces are always newtons.
fn unit_label(units: Units) -> &'static str {
    match units {
        Units::Millimeter => "N · mm",
        Units::Meter => "N · m",
    }
}

/// Writes the XML header, white page, page border and drawing title, and opens
/// the shared content group.
fn write_sheet_open(text: &mut String, meta: &SheetMeta<'_>) {
    text.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        text,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {SHEET_WIDTH} {SHEET_HEIGHT}\" width=\"960\" height=\"720\" \
         font-family=\"{FONT_FAMILY}\" role=\"img\" aria-labelledby=\"ocae-svg-title\">"
    );
    let _ = writeln!(text, "<title id=\"ocae-svg-title\">{}</title>", meta.title);
    text.push_str("<rect x=\"0\" y=\"0\" width=\"960\" height=\"720\" fill=\"#ffffff\"/>\n");
    let _ = writeln!(
        text,
        "<rect x=\"{FRAME_LEFT}\" y=\"{FRAME_TOP}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#111827\" \
         stroke-width=\"1.6\"/>",
        FRAME_RIGHT - FRAME_LEFT,
        FRAME_BOTTOM - FRAME_TOP,
    );
    let _ = writeln!(
        text,
        "<text x=\"480\" y=\"40\" text-anchor=\"middle\" font-size=\"{FONT_TITLE}\" font-weight=\"600\" \
         fill=\"#111827\">{}</text>",
        meta.title
    );
    let _ = writeln!(
        text,
        "<line x1=\"{PLOT_LEFT}\" y1=\"48\" x2=\"{PLOT_RIGHT}\" y2=\"48\" stroke=\"#9ca3af\" stroke-width=\"0.8\"/>"
    );
    text.push_str("<g>\n");
}

/// Approximate advance width of `text` in the shared font at `size` points.
///
/// The layout only needs a stable estimate to keep text inside cells and
/// labels from colliding, so a fixed average advance avoids measuring glyphs.
fn text_width(text: &str, size: f64) -> f64 {
    text.chars().count() as f64 * size * 0.56
}

/// Shrinks `preferred` until `text` fits `max_width`, never below `minimum`.
fn fit_font_size(text: &str, max_width: f64, preferred: f64, minimum: f64) -> f64 {
    let estimated = text_width(text, preferred);
    if estimated <= max_width || estimated <= 0.0 {
        preferred
    }
    else {
        (preferred * max_width / estimated).clamp(minimum, preferred)
    }
}

/// Writes one title-block cell: a small label and the value fitted into the
/// cell width so a long drawing name cannot cross the cell divider.
fn write_title_cell(text: &mut String, x: f64, cell_width: f64, label: &str, value: &str) {
    let _ = writeln!(
        text,
        "<text x=\"{x:.2}\" y=\"{TITLE_LABEL_BASELINE}\" font-size=\"7.5\" letter-spacing=\"0.6\" \
         fill=\"#6b7280\">{label}</text>"
    );
    let size = fit_font_size(value, cell_width - 4.0, 12.5, 8.0);
    let _ = writeln!(
        text,
        "<text x=\"{x:.2}\" y=\"{TITLE_VALUE_BASELINE}\" font-size=\"{size:.2}\" fill=\"#111827\">{}</text>",
        xml_escape(value)
    );
}

/// Closes the content group and draws the title block with the drawing name,
/// drawing number, units, scale, load case and up to two note lines.
///
/// Every row has a fixed baseline and every string is fitted to its cell, so
/// no comment can leave the frame or overprint a divider.
fn write_sheet_close(text: &mut String, meta: &SheetMeta<'_>, notes: &[&str]) {
    text.push_str("</g>\n");
    let _ = writeln!(
        text,
        "<rect x=\"{FRAME_LEFT}\" y=\"{TITLE_BLOCK_TOP}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\" \
         stroke=\"#111827\" stroke-width=\"1.2\"/>",
        FRAME_RIGHT - FRAME_LEFT,
        FRAME_BOTTOM - TITLE_BLOCK_TOP,
    );
    for split in TITLE_SPLITS {
        let _ = writeln!(
            text,
            "<line x1=\"{split}\" y1=\"{TITLE_BLOCK_TOP}\" x2=\"{split}\" y2=\"{TITLE_NOTE_DIVIDER}\" stroke=\"#111827\" \
             stroke-width=\"0.8\"/>"
        );
    }
    let _ = writeln!(
        text,
        "<line x1=\"{FRAME_LEFT}\" y1=\"{TITLE_NOTE_DIVIDER}\" x2=\"{FRAME_RIGHT}\" y2=\"{TITLE_NOTE_DIVIDER}\" \
         stroke=\"#111827\" stroke-width=\"0.8\"/>"
    );
    let cells = [
        (FRAME_LEFT + 8.0, TITLE_SPLITS[0] - FRAME_LEFT - 16.0, "DRAWING", meta.title),
        (TITLE_SPLITS[0] + 8.0, TITLE_SPLITS[1] - TITLE_SPLITS[0] - 16.0, "DWG NO.", meta.number),
        (TITLE_SPLITS[1] + 8.0, TITLE_SPLITS[2] - TITLE_SPLITS[1] - 16.0, "UNITS", meta.units),
        (TITLE_SPLITS[2] + 8.0, TITLE_SPLITS[3] - TITLE_SPLITS[2] - 16.0, "SCALE", meta.scale.as_str()),
        (TITLE_SPLITS[3] + 8.0, FRAME_RIGHT - TITLE_SPLITS[3] - 16.0, "LOAD CASE", meta.load_case.as_str()),
    ];
    for (x, width, label, value) in cells {
        write_title_cell(text, x, width, label, value);
    }
    let note_width = FRAME_RIGHT - FRAME_LEFT - 16.0;
    for (index, note) in notes.iter().take(TITLE_NOTE_BASELINES.len()).enumerate() {
        let size = fit_font_size(note, note_width, FONT_NOTE, 7.0);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{size:.2}\" fill=\"#4b5563\">{}</text>",
            FRAME_LEFT + 8.0,
            TITLE_NOTE_BASELINES[index],
            xml_escape(note)
        );
    }
    text.push_str("</svg>\n");
}

/// Affine map from model coordinates to the shared geometry viewport.
struct Projection {
    min_x: f64,
    max_y: f64,
    origin_x: f64,
    origin_y: f64,
    factor: f64,
}

impl Projection {
    /// Fits every point into the viewport with a uniform scale, centred and
    /// with the shared insets, so the drawing is neither clipped nor pushed
    /// into a corner.
    fn fit(points: Vec<(f64, f64)>) -> Result<Self, CaeError> {
        Self::fit_in(points, VIEW_LEFT, VIEW_TOP, VIEW_RIGHT, VIEW_BOTTOM)
    }

    /// Fits every point into an arbitrary viewport. Sheets that reserve a
    /// legend band pass a smaller bottom edge.
    fn fit_in(points: Vec<(f64, f64)>, left: f64, top: f64, right: f64, bottom: f64) -> Result<Self, CaeError> {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (x, y) in points {
            if !x.is_finite() || !y.is_finite() {
                return Err(CaeError::new(
                    "export/invalid-model",
                    "node coordinates must be finite before an SVG can be drawn",
                ));
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
        let available_width = right - left;
        let available_height = bottom - top;
        let factor = (available_width / width).min(available_height / height);
        let origin_x = left + (available_width - width * factor) / 2.0;
        let origin_y = top + (available_height - height * factor) / 2.0;
        Ok(Self { min_x, max_y, origin_x, origin_y, factor })
    }

    fn project(&self, x: f64, y: f64) -> (f64, f64) {
        (self.origin_x + (x - self.min_x) * self.factor, self.origin_y + (self.max_y - y) * self.factor)
    }
}

/// Attributes that draw a white halo behind a label so it stays readable over
/// member lines and symbols.
const LABEL_HALO: &str = "paint-order=\"stroke\" stroke=\"#ffffff\" stroke-width=\"3\" stroke-linejoin=\"round\"";

/// Returns `true` when the support actually constrains a degree of freedom.
fn support_is_constrained(support: &Support) -> bool {
    support.dx
        || support.dy
        || support.dz
        || support.rx
        || support.ry
        || support.rz
        || support.spring.is_some()
        || support.direction.is_some()
        || support.prescribed.is_some()
}

/// Writes a deterministic SVG sheet of axial, shear and bending-moment
/// diagrams.
///
/// One panel per member (in model element order) shows the `N(x)` curve, and
/// for beam-like members also the `V(x)` curve above the `M(x)` curve, all
/// sampled at `stations` points per member from the same
/// [`solve_internal_forces`] report that fills `forces.csv`. Truss panels
/// carry the axial diagram only, because a two-force member has no shear or
/// bending. The member axis runs from the `from` node (`x = 0`) to the `to`
/// node (`x = L`). Each curve marks the largest-absolute value with its
/// magnitude and position, and every zero crossing with a green marker and
/// its position; a curve that is uniformly zero is labelled `{name} ≡ 0`
/// instead of degenerating into an unmarked flat line. Signs follow the
/// documented convention printed in the sheet footer: `dV/dx = q` with the
/// local `+y` load intensity, `dM/dx = V` and sagging positive. `beam3d`
/// panels plot the local `y` shear and the local `z` bending moment. Values
/// are labelled with four significant digits. A model without any member is
/// rejected with `export/no-beam-diagram` instead of writing an empty sheet.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise `path`
/// is treated as a directory and `forces.svg` is created inside it.
pub fn export_forces_svg(model: &Model, stations: usize, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Forces.default_file_name())
    };
    write_atomic(&target, &forces_svg_text(model, stations)?)
}

/// Builds the force-diagram sheet as SVG text without touching the file
/// system, so the same bytes back the SVG and the PDF export.
pub fn forces_svg_text(model: &Model, stations: usize) -> Result<String, CaeError> {
    let diagrams = solve_internal_forces(model, stations)?;
    let panels: Vec<&ElementDiagram> = diagrams.elements.iter().collect();
    if panels.is_empty() {
        return Err(CaeError::new("export/no-beam-diagram", "the model has no members, so there is no force diagram to draw"));
    }

    let columns = if panels.len() == 1 { 1 } else { 2 };
    let rows = panels.len().div_ceil(columns);
    let panel_width = (PLOT_RIGHT - PLOT_LEFT) / columns as f64;
    let slot_height = PLOT_BOTTOM - PLOT_TOP;
    // Fill the plot area: a short model would otherwise leave its panels
    // floating in a page of white margin.
    let panel_height = slot_height / rows as f64;
    let grid_top = PLOT_TOP;
    let length_unit = match model.units.unwrap_or(Units::Millimeter) {
        Units::Millimeter => "mm",
        Units::Meter => "m",
    };
    let moment_unit = ContourQuantity::Moment.unit(model.units.unwrap_or(Units::Millimeter));
    let meta = SheetMeta::new(model, DrawingKind::Forces.title(), DrawingKind::Forces.number(), format!("{stations} stations"));

    let mut text = String::new();
    write_sheet_open(&mut text, &meta);

    for (index, panel) in panels.iter().enumerate() {
        if !panel.length.is_finite() || panel.length <= 0.0 {
            continue;
        }
        let column = index % columns;
        let row = index / columns;
        let px = PLOT_LEFT + column as f64 * panel_width;
        let py = grid_top + row as f64 * panel_height;
        let plot_left = px + 64.0;
        let plot_right = px + panel_width - 24.0;
        let beam_like = panel.kind != ElementKind::Truss;
        // The station axis at the panel bottom is shared by every curve.
        let station_axis = py + panel_height - 26.0;
        let (half_height, axial_baseline, shear_baseline, moment_baseline, axial_label_y) = if beam_like {
            // Three stacked beam curves share the panel with their axis labels
            // and the station axis. The baselines are laid out upwards from the
            // axis so the moment curve can never run into the ticks.
            let half = ((panel_height - 160.0) / 6.0).clamp(20.0, 63.0);
            let moment_baseline = station_axis - 16.0 - half;
            let shear_baseline = moment_baseline - 2.0 * half - 26.0;
            let axial_baseline = shear_baseline - 2.0 * half - 26.0;
            (half, axial_baseline, shear_baseline, moment_baseline, None)
        }
        else {
            // A truss panel carries one curve. Anchor the zero line on the
            // side opposite the force, so a one-signed diagram fills the panel
            // instead of hugging a centre line.
            let (minimum, maximum) = panel
                .stations
                .iter()
                .map(|station| station.axial_force)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| (low.min(value), high.max(value)));
            let band_top = py + 68.0;
            let band_bottom = station_axis - 12.0;
            let (baseline, half) = if maximum <= 0.0 && minimum < 0.0 {
                (band_top, band_bottom - band_top)
            }
            else if minimum >= 0.0 && maximum > 0.0 {
                (band_bottom, band_bottom - band_top)
            }
            else {
                ((band_top + band_bottom) / 2.0, (band_bottom - band_top) / 2.0)
            };
            (half, baseline, 0.0, 0.0, Some(py + 50.0))
        };

        let _ = writeln!(
            text,
            "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"none\" stroke=\"#e5e7eb\" \
             stroke-width=\"1\"/>",
            px + 4.0,
            py + 4.0,
            panel_width - 8.0,
            panel_height - 8.0,
        );
        let title = format!(
            "{} · {} → {} · L = {} {}",
            panel.id,
            panel.from,
            panel.to,
            format_contour_value(panel.length),
            length_unit
        );
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"13\" font-weight=\"600\" fill=\"#111827\">{}</text>",
            px + 10.0,
            py + 20.0,
            xml_escape(&title),
        );
        // One placer per panel: labels stay inside their own panel, clear of
        // the panel border, the title and every plotted curve.
        let mut placer = LabelPlacer::within(px + 8.0, px + panel_width - 8.0, py + 17.0, py + panel_height - 8.0);
        placer.block_box((px + 4.0, py + 4.0, px + panel_width - 4.0, py + panel_height - 4.0));
        placer.reserve((px + 10.0, py + 8.0, px + 10.0 + text_width(&title, 13.0) + 4.0, py + 23.0));

        // The station axis at the panel bottom is shared by all three curves,
        // so the 0/L labels never land inside a plotted curve.
        placer.block_line((plot_left, station_axis), (plot_right, station_axis));
        let _ = writeln!(
            text,
            "<line x1=\"{plot_left:.2}\" y1=\"{station_axis:.2}\" x2=\"{plot_right:.2}\" y2=\"{station_axis:.2}\" \
             stroke=\"#d1d5db\" stroke-width=\"1\"/>"
        );
        for step in 0..=4 {
            let x = plot_left + (plot_right - plot_left) * step as f64 / 4.0;
            let _ = writeln!(
                text,
                "<line x1=\"{x:.2}\" y1=\"{station_axis:.2}\" x2=\"{x:.2}\" y2=\"{:.2}\" stroke=\"#9ca3af\" \
                 stroke-width=\"1\"/>",
                station_axis + 4.0
            );
        }
        placer.reserve((plot_left - 1.0, station_axis + 5.0, plot_left + 9.0, station_axis + 18.0));
        let _ = writeln!(
            text,
            "<text x=\"{plot_left:.2}\" y=\"{:.2}\" font-size=\"10\" fill=\"#6b7280\">0</text>",
            station_axis + 17.0
        );
        placer.reserve((plot_right - 9.0, station_axis + 5.0, plot_right + 1.0, station_axis + 18.0));
        let _ = writeln!(
            text,
            "<text x=\"{plot_right:.2}\" y=\"{:.2}\" font-size=\"10\" fill=\"#6b7280\" text-anchor=\"end\">L</text>",
            station_axis + 17.0
        );

        let frame = CurveFrame { plot_left, plot_right, half_height };
        let mut labels = Vec::new();
        labels.extend(write_force_curve(
            &mut text,
            &mut placer,
            &frame,
            axial_baseline,
            panel,
            &CurveStyle { name: "N", axis_label: "N(x) [N]", color: "#0891b2", label_y: axial_label_y },
            |station| station.axial_force,
        ));
        if beam_like {
            labels.extend(write_force_curve(
                &mut text,
                &mut placer,
                &frame,
                shear_baseline,
                panel,
                &CurveStyle { name: "V", axis_label: "V(x) [N]", color: "#2563eb", label_y: None },
                |station| station.shear,
            ));
            let moment_axis_label = format!("M(x) [{moment_unit}]");
            labels.extend(write_force_curve(
                &mut text,
                &mut placer,
                &frame,
                moment_baseline,
                panel,
                &CurveStyle { name: "M", axis_label: &moment_axis_label, color: "#b91c1c", label_y: None },
                |station| station.moment,
            ));
        }
        for label in labels {
            let placed = placer.place_near(label.anchor, label_width(&label.text), &LABEL_DIRECTIONS);
            write_leader(&mut text, placed.leader);
            let _ = writeln!(
                text,
                "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{FONT_LABEL}\" fill=\"{}\">{}</text>",
                placed.x,
                placed.y,
                label.colour,
                xml_escape(&label.text)
            );
        }
    }

    // Legend on the reserved band above the title block, outside every panel.
    let legend_y = LEGEND_BASELINE;
    text.push_str("<g font-size=\"12\" fill=\"#111827\">\n");
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"{PLOT_LEFT}\" y1=\"{legend_y}\" x2=\"106\" y2=\"{legend_y}\" stroke=\"#0891b2\" \
         stroke-width=\"2\"/><text x=\"114\" y=\"{legend_y}\">N(x)</text>"
    );
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"210\" y1=\"{legend_y}\" x2=\"236\" y2=\"{legend_y}\" stroke=\"#2563eb\" \
         stroke-width=\"2\"/><text x=\"244\" y=\"{legend_y}\">V(x)</text>"
    );
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"330\" y1=\"{legend_y}\" x2=\"356\" y2=\"{legend_y}\" stroke=\"#b91c1c\" \
         stroke-width=\"2\"/><text x=\"364\" y=\"{legend_y}\">M(x)</text>"
    );
    let _ = writeln!(
        text,
        "<circle data-legend=\"1\" cx=\"470\" cy=\"{legend_y}\" r=\"3.5\" fill=\"#16a34a\"/>\
         <text x=\"478\" y=\"{legend_y}\">zero crossing</text>"
    );
    let _ = writeln!(
        text,
        "<circle data-legend=\"1\" cx=\"600\" cy=\"{legend_y}\" r=\"3\" fill=\"#111827\"/>\
         <text x=\"608\" y=\"{legend_y}\">extreme</text>"
    );
    text.push_str("</g>\n");

    write_sheet_close(
        &mut text,
        &meta,
        &[
            "sign convention: dV/dx = q (local +y), dM/dx = V, sagging positive; beam3d panels plot the local y shear and local z bending moment.",
            "extremes and zero crossings are labelled with four significant digits; truss panels carry the axial diagram only.",
        ],
    );

    Ok(text)
}

/// Writes a deterministic SVG sheet of the shear force `V(x)` diagram drawn on
/// the member axes.
///
/// Only members with rotational stiffness (`beam` and `beam3d`) carry shear, so
/// truss bars are skipped and named on the sheet. Each beam member keeps its
/// axis as the zero line and the sampled shear is drawn as filled polygons on
/// the local `+y` side: tension-side values positive (blue) and negative
/// values red. The axis end values and the extreme station are labelled from
/// the same [`solve_internal_forces`] report that fills `forces.csv`, so the
/// drawing and the numbers cannot drift.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise `path`
/// is treated as a directory and `shear.svg` is created inside it.
pub fn export_shear_svg(model: &Model, stations: usize, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Shear.default_file_name())
    };
    write_atomic(&target, &shear_svg_text(model, stations)?)
}

/// Builds the shear-force sheet as SVG text without touching the file system,
/// so the same bytes back the SVG and the PDF export.
pub fn shear_svg_text(model: &Model, stations: usize) -> Result<String, CaeError> {
    beam_diagram_svg_text(model, stations, BeamDiagramQuantity::Shear)
}

/// Writes a deterministic SVG sheet of the bending moment `M(x)` diagram drawn
/// on the member axes.
///
/// The layout matches [`export_shear_svg`]: only `beam` and `beam3d` members are
/// drawn, sagging moments are positive and plotted on the local `+y` side,
/// negative moments are drawn on the opposite side in red, and both member end
/// values plus the extreme station are labelled from the solved internal
/// forces.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise `path`
/// is treated as a directory and `moment.svg` is created inside it.
pub fn export_moment_svg(model: &Model, stations: usize, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Moment.default_file_name())
    };
    write_atomic(&target, &moment_svg_text(model, stations)?)
}

/// Builds the bending-moment sheet as SVG text without touching the file
/// system, so the same bytes back the SVG and the PDF export.
pub fn moment_svg_text(model: &Model, stations: usize) -> Result<String, CaeError> {
    beam_diagram_svg_text(model, stations, BeamDiagramQuantity::Moment)
}

/// Which internal force the beam diagram sheet plots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BeamDiagramQuantity {
    Shear,
    Moment,
}

impl BeamDiagramQuantity {
    fn symbol(self) -> &'static str {
        match self {
            Self::Shear => "V",
            Self::Moment => "M",
        }
    }

    fn drawn_title(self) -> &'static str {
        match self {
            Self::Shear => "OpenCAE shear force diagram",
            Self::Moment => "OpenCAE bending moment diagram",
        }
    }

    fn value(self, station: &crate::solver::SectionForces) -> f64 {
        match self {
            Self::Shear => station.shear,
            Self::Moment => station.moment,
        }
    }
}

/// Builds a filled shear or moment diagram on the member axes.
///
/// The projection reserves room for the largest excursion on every side, so a
/// fill can never leave the plot area even at the model edge. Truss members are
/// drawn dashed and listed in a status line instead of being given a diagram.
fn beam_diagram_svg_text(model: &Model, stations: usize, quantity: BeamDiagramQuantity) -> Result<String, CaeError> {
    if model.elements.is_empty() {
        return Err(CaeError::new("export/no-beam-diagram", "the model has no members, so there is no diagram to draw"));
    }
    let diagrams = solve_internal_forces(model, stations)?;
    let units = model.units.unwrap_or(Units::Millimeter);
    let moment_unit = ContourQuantity::Moment.unit(units);
    let length_unit = match units {
        Units::Millimeter => "mm",
        Units::Meter => "m",
    };
    let value_unit = match quantity {
        BeamDiagramQuantity::Shear => "N",
        BeamDiagramQuantity::Moment => moment_unit,
    };

    let beam_like = |kind: ElementKind| matches!(kind, ElementKind::Beam | ElementKind::Beam3D);
    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let project_point = |x: f64, y: f64, z: f64| (x + oblique_x * z, y + oblique_y * z);
    let geometry: Vec<(f64, f64)> = model.nodes.iter().map(|node| project_point(node.x, node.y, node.z)).collect();
    // The diagram excursion is large enough to fill the page; the fit reserves
    // exactly the horizontal and vertical part each member's normal needs, so
    // the fill cannot leave the viewport even on a diagonal or 3D member.
    const MAX_OFFSET: f64 = 165.0;
    let mut max_normal_x: f64 = 0.0;
    let mut max_normal_y: f64 = 0.0;
    for element in model.elements.iter().filter(|element| beam_like(element.kind)) {
        let Some(from) = model.nodes.iter().find(|node| node.id == element.from)
        else {
            continue;
        };
        let Some(to) = model.nodes.iter().find(|node| node.id == element.to)
        else {
            continue;
        };
        let (ax, ay) = project_point(from.x, from.y, from.z);
        let (bx, by) = project_point(to.x, to.y, to.z);
        let (dx, dy) = (bx - ax, by - ay);
        let length = (dx * dx + dy * dy).sqrt();
        if length > 0.0 && length.is_finite() {
            max_normal_x = max_normal_x.max((dy / length).abs());
            max_normal_y = max_normal_y.max((dx / length).abs());
        }
    }
    let reserve_x = MAX_OFFSET * max_normal_x + 24.0;
    let reserve_y = MAX_OFFSET * max_normal_y + 24.0;
    let projection = Projection::fit_in(
        geometry.clone(),
        VIEW_LEFT + reserve_x,
        VIEW_TOP + reserve_y,
        VIEW_RIGHT - reserve_x,
        VIEW_BOTTOM - reserve_y,
    )?;
    let project = |x: f64, y: f64| projection.project(x, y);

    let limit = diagrams
        .elements
        .iter()
        .filter(|diagram| beam_like(diagram.kind))
        .flat_map(|diagram| diagram.stations.iter())
        .map(|station| quantity.value(station).abs())
        .fold(0.0, f64::max);
    // End forces recovered from the element stiffness matrix carry round-off
    // such as 3e-11 on a support that is analytically zero; snap it before it
    // reaches a printed label.
    let snap = move |value: f64| if limit > 0.0 && value.abs() <= 1.0e-9 * limit { 0.0 } else { value };
    let mut skipped = Vec::new();
    for diagram in &diagrams.elements {
        if !beam_like(diagram.kind) {
            skipped.push(diagram.id.as_str());
        }
    }
    let meta = SheetMeta::new(
        model,
        quantity.drawn_title(),
        match quantity {
            BeamDiagramQuantity::Shear => DrawingKind::Shear.number(),
            BeamDiagramQuantity::Moment => DrawingKind::Moment.number(),
        },
        "schematic",
    );

    let mut text = String::new();
    write_sheet_open(&mut text, &meta);

    let status = if skipped.is_empty() {
        "all members are beam-like; the filled diagram is drawn on each member axis.".to_string()
    }
    else {
        format!("truss members carry no shear or moment and are skipped: {}.", skipped.join(", "))
    };
    let status_size = fit_font_size(&status, PLOT_RIGHT - PLOT_LEFT, 9.5, 7.5);
    let _ = writeln!(
        text,
        "<text x=\"{PLOT_LEFT}\" y=\"57\" font-size=\"{status_size:.2}\" fill=\"#6b7280\">{}</text>",
        xml_escape(&status)
    );

    // Geometry first: every member axis, every truss bar and every filled
    // excursion is registered with the placer before any label is written.
    let mut blocked: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let mut symbol_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut node_pairs = Vec::with_capacity(model.elements.len());
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
        node_pairs.push((from, to));
    }

    text.push_str("<g stroke-linecap=\"round\">\n");
    // Truss bars only carry the axial force, so they stay as a thin skeleton.
    for (&(from, to), element) in node_pairs.iter().zip(&model.elements) {
        if beam_like(element.kind) {
            continue;
        }
        let (x1, y1) = project(geometry[from].0, geometry[from].1);
        let (x2, y2) = project(geometry[to].0, geometry[to].1);
        blocked.push(((x1, y1), (x2, y2)));
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#9ca3af\" stroke-width=\"1.4\" \
             stroke-dasharray=\"5 4\"/>"
        );
    }
    // Supports keep the diagram readable as a structure, not a floating axis.
    for support in &model.supports {
        if !support_is_constrained(support) {
            continue;
        }
        let Some(index) = model.nodes.iter().position(|node| node.id == support.node)
        else {
            continue;
        };
        let (cx, cy) = project(geometry[index].0, geometry[index].1);
        symbol_boxes.push((cx - 11.0, cy + 4.0, cx + 11.0, cy + 16.0));
        let _ = writeln!(
            text,
            "<polygon points=\"{cx:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"#16a34a\"/>",
            cy + 4.0,
            cx - 8.0,
            cy + 15.0,
            cx + 8.0,
            cy + 15.0,
        );
        let _ = writeln!(
            text,
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#16a34a\" stroke-width=\"1.5\"/>",
            cx - 11.0,
            cy + 15.0,
            cx + 11.0,
            cy + 15.0,
        );
    }

    // Each committed polygon edge is a drawing line no label may cross.
    struct DiagramPolygon {
        points: [(f64, f64); 4],
        fill: &'static str,
        stroke: &'static str,
    }
    let mut polygons: Vec<DiagramPolygon> = Vec::new();
    let mut diagram_points: Vec<Vec<(f64, f64)>> = Vec::new();
    for ((&(from, to), element), diagram) in node_pairs.iter().zip(&model.elements).zip(&diagrams.elements) {
        if !beam_like(element.kind) || !diagram.length.is_finite() || diagram.length <= 0.0 {
            continue;
        }
        let (x1, y1) = project(geometry[from].0, geometry[from].1);
        let (x2, y2) = project(geometry[to].0, geometry[to].1);
        let (dx, dy) = (x2 - x1, y2 - y1);
        let length = (dx * dx + dy * dy).sqrt();
        if !length.is_finite() || length <= 0.0 {
            continue;
        }
        // The local +y axis of the member, projected. A horizontal member from
        // left to right plots positive values above its axis.
        let (nx, ny) = (dy / length, -dx / length);
        let map = |position: f64, value: f64| {
            let fraction = (position / diagram.length).clamp(0.0, 1.0);
            let offset = if limit > 0.0 { snap(value) / limit * MAX_OFFSET } else { 0.0 };
            (x1 + dx * fraction + nx * offset, y1 + dy * fraction + ny * offset)
        };
        let bases: Vec<(f64, f64)> = diagram.stations.iter().map(|station| map(station.position, 0.0)).collect();
        let points: Vec<(f64, f64)> =
            diagram.stations.iter().map(|station| map(station.position, quantity.value(station))).collect();
        for window in 0..points.len().saturating_sub(1) {
            let v1 = snap(quantity.value(&diagram.stations[window]));
            let v2 = snap(quantity.value(&diagram.stations[window + 1]));
            if (v1 - 0.0).abs() <= 1.0e-12 * limit.max(1.0) && (v2 - 0.0).abs() <= 1.0e-12 * limit.max(1.0) {
                continue;
            }
            let positive = v1 + v2 >= 0.0;
            let (fill, stroke) = if positive { ("#dbeafe", "#2563eb") } else { ("#fee2e2", "#b91c1c") };
            if v1 * v2 >= 0.0 {
                polygons.push(DiagramPolygon {
                    points: [bases[window], points[window], points[window + 1], bases[window + 1]],
                    fill,
                    stroke,
                });
            }
            else {
                let fraction = v1 / (v1 - v2);
                let (bx, by) = (
                    bases[window].0 + (bases[window + 1].0 - bases[window].0) * fraction,
                    bases[window].1 + (bases[window + 1].1 - bases[window].1) * fraction,
                );
                let crossing = (bx, by);
                let first_positive = v1 > 0.0;
                let first = if first_positive { ("#dbeafe", "#2563eb") } else { ("#fee2e2", "#b91c1c") };
                let second = if first_positive { ("#fee2e2", "#b91c1c") } else { ("#dbeafe", "#2563eb") };
                polygons.push(DiagramPolygon {
                    points: [bases[window], points[window], crossing, crossing],
                    fill: first.0,
                    stroke: first.1,
                });
                polygons.push(DiagramPolygon {
                    points: [crossing, crossing, points[window + 1], bases[window + 1]],
                    fill: second.0,
                    stroke: second.1,
                });
            }
        }
        diagram_points.push(points);
    }
    for polygon in &polygons {
        let [a, b, c, d] = polygon.points;
        let fill = polygon.fill;
        let stroke = polygon.stroke;
        let points = format!("{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}", a.0, a.1, b.0, b.1, c.0, c.1, d.0, d.1);
        let _ = writeln!(text, "<polygon points=\"{points}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1\"/>");
        blocked.push((a, b));
        blocked.push((b, c));
        blocked.push((c, d));
        blocked.push((d, a));
    }
    // The member axis is the zero line, drawn on top of the fill so the sign
    // change stays readable.
    for (&(from, to), element) in node_pairs.iter().zip(&model.elements) {
        if !beam_like(element.kind) {
            continue;
        }
        let (x1, y1) = project(geometry[from].0, geometry[from].1);
        let (x2, y2) = project(geometry[to].0, geometry[to].1);
        blocked.push(((x1, y1), (x2, y2)));
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#111827\" stroke-width=\"2.4\"/>"
        );
    }
    for curve in &diagram_points {
        let mut points = String::new();
        for &(x, y) in curve {
            let _ = write!(points, "{x:.2},{y:.2} ");
        }
        let _ = writeln!(
            text,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#374151\" stroke-width=\"1.2\"/>",
            points.trim_end()
        );
    }
    for &(x, y) in &geometry {
        let (cx, cy) = project(x, y);
        symbol_boxes.push((cx - 4.0, cy - 4.0, cx + 4.0, cy + 4.0));
        let _ = writeln!(text, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"2.8\" fill=\"#111827\"/>");
    }
    text.push_str("</g>\n");

    // Labels last, after every fill and axis, so nothing can be overprinted.
    text.push_str("<g font-size=\"11\">\n");
    let mut placer = LabelPlacer::within(PLOT_LEFT + 4.0, PLOT_RIGHT - 4.0, PLOT_TOP + 4.0, PLOT_BOTTOM - 4.0);
    for (from, to) in &blocked {
        placer.block_line(*from, *to);
    }
    for rect in symbol_boxes {
        placer.block_box(rect);
    }
    let center = centroid(&geometry.iter().map(|&(x, y)| project(x, y)).collect::<Vec<_>>());
    for (index, node) in model.nodes.iter().enumerate() {
        let point = project(geometry[index].0, geometry[index].1);
        let placed = place_node_label(&mut placer, &node.id, point, center, false, false);
        write_leader(&mut text, placed.leader);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"#374151\">{}</text>",
            placed.x,
            placed.y,
            xml_escape(&node.id)
        );
    }

    // A shared node between two members would otherwise print the same end
    // value twice at the same point.
    let mut seen_annotations: Vec<(String, (f64, f64))> = Vec::new();
    for ((&(from, to), element), diagram) in node_pairs.iter().zip(&model.elements).zip(&diagrams.elements) {
        if !beam_like(element.kind) || !diagram.length.is_finite() || diagram.length <= 0.0 {
            continue;
        }
        let (x1, y1) = project(geometry[from].0, geometry[from].1);
        let (x2, y2) = project(geometry[to].0, geometry[to].1);
        let (dx, dy) = (x2 - x1, y2 - y1);
        let length = (dx * dx + dy * dy).sqrt();
        if !length.is_finite() || length <= 0.0 {
            continue;
        }
        let (nx, ny) = (dy / length, -dx / length);
        let tip = |position: f64, value: f64| {
            let fraction = (position / diagram.length).clamp(0.0, 1.0);
            let offset = if limit > 0.0 { snap(value) / limit * MAX_OFFSET } else { 0.0 };
            (x1 + dx * fraction + nx * offset, y1 + dy * fraction + ny * offset)
        };
        let mut annotations: Vec<(String, (f64, f64), bool)> = Vec::new();
        if let (Some(first), Some(last)) = (diagram.stations.first(), diagram.stations.last()) {
            let first_value = snap(quantity.value(first));
            let last_value = snap(quantity.value(last));
            annotations.push((
                format!("{} = {} {value_unit}", quantity.symbol(), format_contour_value(first_value)),
                tip(first.position, first_value),
                first_value >= 0.0,
            ));
            annotations.push((
                format!("{} = {} {value_unit}", quantity.symbol(), format_contour_value(last_value)),
                tip(last.position, last_value),
                last_value >= 0.0,
            ));
        }
        if let Some((station, value)) = diagram
            .stations
            .iter()
            .map(|station| (station, snap(quantity.value(station))))
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap_or(std::cmp::Ordering::Equal))
        {
            // An extreme that sits on a member end is already covered by the
            // two end labels, so a third label would only duplicate it and
            // crowd the same corner.
            let at_end = diagram.stations.first().zip(diagram.stations.last()).is_some_and(|(first, last)| {
                (station.position - first.position).abs() < 1.0e-9 || (station.position - last.position).abs() < 1.0e-9
            });
            if value.abs() > 0.0 && !at_end {
                annotations.push((
                    format!(
                        "max |{}| = {} {value_unit} @ x={}",
                        quantity.symbol(),
                        format_contour_value(value.abs()),
                        format_contour_value(station.position)
                    ),
                    tip(station.position, value),
                    value >= 0.0,
                ));
            }
        }
        for (label, anchor, positive) in annotations {
            if seen_annotations
                .iter()
                .any(|(seen, point)| *seen == label && (point.0 - anchor.0).abs() < 3.0 && (point.1 - anchor.1).abs() < 3.0)
            {
                continue;
            }
            seen_annotations.push((label.clone(), anchor));
            let outward = if positive { (nx, ny) } else { (-nx, -ny) };
            let placed = placer.place_near(anchor, label_width(&label), &directions_towards(outward));
            write_leader(&mut text, placed.leader);
            let colour = if positive { "#1d4ed8" } else { "#b91c1c" };
            let _ = writeln!(
                text,
                "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"{colour}\">{}</text>",
                placed.x,
                placed.y,
                xml_escape(&label)
            );
        }
    }
    text.push_str("</g>\n");

    // Legend on the reserved band above the title block, outside the model.
    let legend_y = LEGEND_BASELINE;
    text.push_str("<g font-size=\"12\" fill=\"#111827\">\n");
    let _ = writeln!(
        text,
        "<rect data-legend=\"1\" x=\"{PLOT_LEFT}\" y=\"{}\" width=\"22\" height=\"8\" fill=\"#dbeafe\" stroke=\"#2563eb\"/>\
         <text x=\"{}\" y=\"{}\">positive {} on local +y</text>",
        legend_y - 8.0,
        PLOT_LEFT + 30.0,
        legend_y,
        quantity.symbol()
    );
    let _ = writeln!(
        text,
        "<rect data-legend=\"1\" x=\"292\" y=\"{}\" width=\"22\" height=\"8\" fill=\"#fee2e2\" stroke=\"#b91c1c\"/>\
         <text x=\"322\" y=\"{legend_y}\">negative {}</text>",
        legend_y - 8.0,
        quantity.symbol()
    );
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"412\" y1=\"{legend_y}\" x2=\"438\" y2=\"{legend_y}\" stroke=\"#111827\" \
         stroke-width=\"2.4\"/><text x=\"446\" y=\"{legend_y}\">member axis (zero)</text>"
    );
    if !skipped.is_empty() {
        let _ = writeln!(
            text,
            "<line data-legend=\"1\" x1=\"604\" y1=\"{legend_y}\" x2=\"630\" y2=\"{legend_y}\" stroke=\"#9ca3af\" \
             stroke-width=\"1.4\" stroke-dasharray=\"5 4\"/><text x=\"638\" y=\"{legend_y}\">truss skipped</text>"
        );
    }
    text.push_str("</g>\n");

    let sign_note = match quantity {
        BeamDiagramQuantity::Shear => {
            "sign convention: V is positive when it acts upward on the left face of the cut (dV/dx = q, local +y), plotted on the local +y side."
        }
        BeamDiagramQuantity::Moment => {
            "sign convention: sagging moment is positive and plotted on the local +y side (dM/dx = V); beam3d plots the local z moment."
        }
    };
    let detail_note = format!(
        "lengths in {length_unit}; each member end value and the extreme station are labelled; truss members carry no shear or moment and are skipped."
    );
    write_sheet_close(&mut text, &meta, &[sign_note, &detail_note]);

    Ok(text)
}

/// Plot geometry of one quantity curve inside a force diagram panel.
struct CurveFrame {
    plot_left: f64,
    plot_right: f64,
    half_height: f64,
}

/// Label and colour of one quantity curve inside a force diagram panel.
struct CurveStyle<'a> {
    name: &'a str,
    axis_label: &'a str,
    color: &'a str,
    /// Fixed baseline of the quantity label, used by a one-curve truss panel
    /// whose zero line may sit far from the panel title.
    label_y: Option<f64>,
}

/// One text annotation requested by [`write_force_curve`]. The anchor is the
/// point on the curve the label belongs to; the shared placer moves the text
/// away from every line before it is written.
struct CurveLabel {
    text: String,
    anchor: (f64, f64),
    colour: &'static str,
}

/// Draws one sampled quantity of a panel as a curve over its zero line.
///
/// The curve is scaled so the largest absolute sampled value fills
/// `half_height` above and below the baseline; a uniform zero field does not
/// degenerate into an unmarked flat line but is labelled `{name} ≡ 0` on the
/// baseline. The extremum station gets a filled dot and a `max |{name}| =
/// {value} @ x={position}` label; every zero crossing of the quantity gets a
/// green dot and a `{name}=0 @ x={position}` label. Exact zeros are detected
/// after snapping round-off below `1e-12` of the largest magnitude, sign
/// changes between neighbouring stations are located by linear interpolation.
///
/// The curve, the baseline, the ticks and the axis label are registered with
/// `placer`, so the returned labels are written after every curve and cannot
/// cross one.
fn write_force_curve(
    text: &mut String,
    placer: &mut LabelPlacer,
    frame: &CurveFrame,
    baseline: f64,
    panel: &ElementDiagram,
    style: &CurveStyle<'_>,
    value: impl Fn(&crate::solver::SectionForces) -> f64,
) -> Vec<CurveLabel> {
    let CurveFrame { plot_left, plot_right, half_height } = *frame;
    let CurveStyle { name, axis_label, color, label_y } = *style;
    let limit = panel.stations.iter().map(|station| value(station).abs()).fold(0.0, f64::max);
    let snap = |raw: f64| if limit > 0.0 && raw.abs() <= 1.0e-12 * limit { 0.0 } else { raw };
    let scale = if limit > 0.0 { half_height / limit } else { 0.0 };
    let map_x = |position: f64| plot_left + (position / panel.length).clamp(0.0, 1.0) * (plot_right - plot_left);
    let map_y = |raw: f64| baseline - snap(raw) * scale;
    let mut labels = Vec::new();

    placer.block_line((plot_left, baseline), (plot_right, baseline));
    let _ = writeln!(
        text,
        "<line x1=\"{plot_left:.2}\" y1=\"{baseline:.2}\" x2=\"{plot_right:.2}\" y2=\"{baseline:.2}\" stroke=\"#d1d5db\" \
         stroke-width=\"1\"/>"
    );
    let axis_y = label_y.unwrap_or(baseline - half_height - 8.0);
    placer.reserve((plot_left, axis_y - 12.0, plot_left + text_width(axis_label, 12.0) + 4.0, axis_y + 3.0));
    let _ =
        writeln!(text, "<text x=\"{plot_left:.2}\" y=\"{axis_y:.2}\" font-size=\"12\" fill=\"#374151\">{axis_label}</text>");

    if limit == 0.0 {
        // A uniformly zero quantity would otherwise draw an unmarked flat
        // line that is indistinguishable from a missing diagram.
        labels.push(CurveLabel {
            text: format!("{name} \u{2261} 0"),
            anchor: ((plot_left + plot_right) / 2.0, baseline),
            colour: "#374151",
        });
        return labels;
    }

    let projected: Vec<(f64, f64)> =
        panel.stations.iter().map(|station| (map_x(station.position), map_y(value(station)))).collect();
    let mut points = String::new();
    for &(x, y) in &projected {
        let _ = write!(points, "{x:.2},{y:.2} ");
    }
    // A one-curve truss panel is shaded between the zero line and the curve.
    // The pale band makes a constant axial force read as a diagram instead of a
    // line floating in a large empty panel.
    if label_y.is_some() {
        for index in 0..projected.len().saturating_sub(1) {
            let v1 = snap(value(&panel.stations[index]));
            let v2 = snap(value(&panel.stations[index + 1]));
            if v1 == 0.0 && v2 == 0.0 {
                continue;
            }
            let first = projected[index];
            let second = projected[index + 1];
            let mut quads: Vec<Vec<(f64, f64)>> = Vec::new();
            if v1 * v2 >= 0.0 {
                quads.push(vec![(first.0, baseline), (second.0, baseline), second, first]);
            }
            else {
                let fraction = v1 / (v1 - v2);
                let x = first.0 + (second.0 - first.0) * fraction;
                quads.push(vec![(first.0, baseline), (x, baseline), first]);
                quads.push(vec![(x, baseline), (second.0, baseline), second]);
            }
            for quad in quads {
                let positive = v1 + v2 >= 0.0;
                let fill = if positive { "#fef2f2" } else { "#eff6ff" };
                let rendered = quad.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ");
                let _ = writeln!(text, "<polygon points=\"{rendered}\" fill=\"{fill}\" stroke=\"none\"/>");
            }
        }
    }
    for window in projected.windows(2) {
        placer.block_line(window[0], window[1]);
    }
    let _ = writeln!(text, "<polyline points=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\"/>", points.trim_end(),);

    let mut extreme: Option<(f64, f64)> = None;
    for station in &panel.stations {
        let raw = snap(value(station));
        if extreme.is_none_or(|(best, _)| raw.abs() > best.abs()) {
            extreme = Some((raw, station.position));
        }
    }
    if let Some((best, position)) = extreme {
        let x = map_x(position);
        let y = map_y(best);
        let _ = writeln!(text, "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"3\" fill=\"{color}\"/>");
        placer.block_box((x - 3.0, y - 3.0, x + 3.0, y + 3.0));
        labels.push(CurveLabel {
            text: format!("max |{name}| = {} @ x={}", format_contour_value(best.abs()), format_contour_value(position)),
            anchor: (x, y),
            colour: "#111827",
        });
    }

    let mut previous: Option<(f64, f64)> = None;
    for station in &panel.stations {
        let raw = snap(value(station));
        let crossing = match previous {
            None => None,
            Some((before, before_position)) => {
                if raw == 0.0 && before != 0.0 {
                    Some(station.position)
                }
                else if before * raw < 0.0 {
                    Some(before_position + (station.position - before_position) * before / (before - raw))
                }
                else {
                    None
                }
            }
        };
        if let Some(position) = crossing {
            let x = map_x(position);
            let _ = writeln!(text, "<circle cx=\"{x:.2}\" cy=\"{baseline:.2}\" r=\"3.5\" fill=\"#16a34a\"/>");
            placer.block_box((x - 3.5, baseline - 3.5, x + 3.5, baseline + 3.5));
            labels.push(CurveLabel {
                text: format!("{name}=0 @ x={}", format_contour_value(position)),
                anchor: (x, baseline),
                colour: "#166534",
            });
        }
        previous = Some((raw, station.position));
    }
    labels
}
/// Picks the deformation scale that makes the largest nodal displacement span
/// about 18 % of the drawing canvas.
///
/// The screen factor is measured on the undeformed projection of the model, so
/// the result depends only on the model geometry and the solved displacement
/// magnitudes and is deterministic. A model without any displacement, or with
/// non-finite geometry, keeps the neutral `1.0`.
pub fn auto_deformation_scale(model: &Model, solution: &Solution) -> f64 {
    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for node in &model.nodes {
        let x = node.x + oblique_x * node.z;
        let y = node.y + oblique_y * node.z;
        if !x.is_finite() || !y.is_finite() {
            return 1.0;
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
    }
    if height <= 0.0 {
        height = 1.0;
    }
    let available_width = VIEW_RIGHT - VIEW_LEFT;
    let available_height = VIEW_BOTTOM - VIEW_TOP;
    let factor = (available_width / width).min(available_height / height);
    let max_magnitude = solution
        .nodes
        .iter()
        .map(|result| (result.ux * result.ux + result.uy * result.uy + result.uz * result.uz).sqrt())
        .fold(0.0, f64::max);
    if max_magnitude > 0.0 && max_magnitude.is_finite() && factor > 0.0 && factor.is_finite() {
        (0.18 * available_height) / (max_magnitude * factor)
    }
    else {
        1.0
    }
}

/// Approximate width of a label drawn at the annotation size in the sheet
/// font.
///
/// The label placement only needs a stable estimate, not exact metrics, so a
/// fixed average advance keeps labels apart without measuring text.
fn label_width(text: &str) -> f64 {
    text_width(text, FONT_LABEL)
}

fn rectangles_overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// Glyph box of a label whose left end and baseline are (`x`, `y`).
fn label_box(x: f64, y: f64, width: f64) -> (f64, f64, f64, f64) {
    (x - 1.0, y - 11.0, x + width + 1.0, y + 3.0)
}

fn point_in_rect(point: (f64, f64), rect: (f64, f64, f64, f64)) -> bool {
    point.0 >= rect.0 && point.0 <= rect.2 && point.1 >= rect.1 && point.1 <= rect.3
}

fn orientation(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Proper-intersection test of two open segments; touching endpoints count,
/// because a label that touches a member line still reads as a crossing.
fn segments_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let o1 = orientation(a, b, c);
    let o2 = orientation(a, b, d);
    let o3 = orientation(c, d, a);
    let o4 = orientation(c, d, b);
    if ((o1 > 0.0 && o2 < 0.0) || (o1 < 0.0 && o2 > 0.0)) && ((o3 > 0.0 && o4 < 0.0) || (o3 < 0.0 && o4 > 0.0)) {
        return true;
    }
    // Collinear segments that touch: check the bounding boxes.
    let on = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        o1.abs() <= 1.0e-9
            && (p.0.min(q.0) - 1.0e-9..=p.0.max(q.0) + 1.0e-9).contains(&r.0)
            && (p.1.min(q.1) - 1.0e-9..=p.1.max(q.1) + 1.0e-9).contains(&r.1)
    };
    (o1.abs() <= 1.0e-9 && on(a, b, c))
        || (o2.abs() <= 1.0e-9 && on(a, b, d))
        || (o3.abs() <= 1.0e-9 && on(c, d, a))
        || (o4.abs() <= 1.0e-9 && on(c, d, b))
}

fn segment_hits_rect(segment: ((f64, f64), (f64, f64)), rect: (f64, f64, f64, f64)) -> bool {
    if point_in_rect(segment.0, rect) || point_in_rect(segment.1, rect) {
        return true;
    }
    let corners = [(rect.0, rect.1), (rect.2, rect.1), (rect.2, rect.3), (rect.0, rect.3)];
    (0..4).any(|index| {
        let from = corners[index];
        let to = corners[(index + 1) % 4];
        segments_cross(segment.0, segment.1, from, to)
    })
}

/// The eight compass directions, used to search room around a label anchor.
const DIAGONAL: f64 = std::f64::consts::FRAC_1_SQRT_2;
const LABEL_DIRECTIONS: [(f64, f64); 8] = [
    (0.0, -1.0),
    (DIAGONAL, -DIAGONAL),
    (1.0, 0.0),
    (DIAGONAL, DIAGONAL),
    (0.0, 1.0),
    (-DIAGONAL, DIAGONAL),
    (-1.0, 0.0),
    (-DIAGONAL, -DIAGONAL),
];

/// A label that has been placed, with an optional leader line from its anchor
/// to the glyph box when the free spot was only found further out.
#[derive(Debug, Clone, Copy, Default)]
struct PlacedLabel {
    x: f64,
    y: f64,
    leader: Option<((f64, f64), (f64, f64))>,
}

/// Remembers the glyph boxes and the drawn geometry of the sheet so the next
/// label can pick a spot that overprints neither an earlier label nor a member
/// line, an axis or an arrow.
struct LabelPlacer {
    taken: Vec<(f64, f64, f64, f64)>,
    blocked: Vec<((f64, f64), (f64, f64))>,
    min_x: f64,
    max_x: f64,
    min_baseline: f64,
    max_baseline: f64,
}

impl LabelPlacer {
    /// Restricts every label to the given box, so labels of a geometry sheet
    /// stay inside the plot area instead of drifting into the frame.
    fn within(min_x: f64, max_x: f64, min_baseline: f64, max_baseline: f64) -> Self {
        Self { taken: Vec::new(), blocked: Vec::new(), min_x, max_x, min_baseline, max_baseline }
    }

    /// Registers a drawn line that labels must not cross.
    fn block_line(&mut self, from: (f64, f64), to: (f64, f64)) {
        self.blocked.push((from, to));
    }

    /// Registers a filled symbol such as a node dot or support triangle.
    fn block_box(&mut self, rect: (f64, f64, f64, f64)) {
        let (min_x, min_y, max_x, max_y) = rect;
        let corners = [(min_x, min_y), (max_x, min_y), (max_x, max_y), (min_x, max_y)];
        for index in 0..4 {
            self.blocked.push((corners[index], corners[(index + 1) % 4]));
        }
    }

    /// Reserves a box drawn outside the placer flow, such as a panel title, so
    /// labels avoid it.
    fn reserve(&mut self, rect: (f64, f64, f64, f64)) {
        self.taken.push(rect);
    }

    fn free(&self, candidate: (f64, f64, f64, f64)) -> bool {
        if self.taken.iter().any(|taken| rectangles_overlap(candidate, *taken)) {
            return false;
        }
        let padded = (candidate.0 - 1.0, candidate.1 - 1.0, candidate.2 + 1.0, candidate.3 + 1.0);
        !self.blocked.iter().any(|segment| segment_hits_rect(*segment, padded))
    }

    /// Places a label of `width` whose preferred baseline anchor is (`x`, `y`)
    /// and returns the final baseline anchor.
    fn place(&mut self, x: f64, y: f64, width: f64) -> (f64, f64) {
        let placed = self.place_label(x, y, width);
        (placed.x, placed.y)
    }

    /// Like [`LabelPlacer::place`] but keeps the leader line of a label that
    /// had to move away from its anchor.
    fn place_label(&mut self, x: f64, y: f64, width: f64) -> PlacedLabel {
        let anchor = (x + width / 2.0, y - 4.0);
        self.place_near(anchor, width, &LABEL_DIRECTIONS)
    }

    /// Places a label around `anchor`, probing the eight directions outwards at
    /// increasing distances. A candidate is accepted only when its glyph box
    /// stays in bounds and crosses no registered line and no earlier label.
    /// When every near spot collides, the label moves further out and is
    /// connected to its anchor with a leader line instead of overprinting.
    fn place_near(&mut self, anchor: (f64, f64), width: f64, directions: &[(f64, f64)]) -> PlacedLabel {
        const DISTANCES: [f64; 4] = [12.0, 21.0, 31.0, 43.0];
        for distance in DISTANCES {
            for &(dx, dy) in directions {
                let cx = anchor.0 + dx * distance;
                let cy = anchor.1 + dy * distance;
                let ax = (cx - width / 2.0).clamp(self.min_x, self.max_x - width);
                let ay = (cy + 4.0).clamp(self.min_baseline, self.max_baseline);
                let candidate = label_box(ax, ay, width);
                if self.free(candidate) {
                    self.taken.push(candidate);
                    let leader = if distance > 18.0 { Some(leader_line(anchor, candidate)) } else { None };
                    return PlacedLabel { x: ax, y: ay, leader };
                }
            }
        }
        // Every near slot is taken: move the label out along its preferred
        // direction and connect it with a leader instead of overprinting a
        // member or another label.
        let (dx, dy) = directions.first().copied().unwrap_or((0.0, -1.0));
        let ax = (anchor.0 + dx * 56.0 - width / 2.0).clamp(self.min_x, self.max_x - width);
        let ay = (anchor.1 + dy * 56.0 + 4.0).clamp(self.min_baseline, self.max_baseline);
        let candidate = label_box(ax, ay, width);
        self.taken.push(candidate);
        PlacedLabel { x: ax, y: ay, leader: Some(leader_line(anchor, candidate)) }
    }
}

/// Leader from a geometry anchor to the nearest edge of a label box.
fn leader_line(anchor: (f64, f64), rect: (f64, f64, f64, f64)) -> ((f64, f64), (f64, f64)) {
    let edge_x = if anchor.0 < rect.0 {
        rect.0
    }
    else if anchor.0 > rect.2 {
        rect.2
    }
    else {
        anchor.0
    };
    let edge_y = if anchor.1 < rect.1 {
        rect.1
    }
    else if anchor.1 > rect.3 {
        rect.3
    }
    else {
        anchor.1
    };
    (anchor, (edge_x, edge_y))
}

/// Writes the leader line of a placed label, if the placement had to move the
/// text away from its anchor.
fn write_leader(text: &mut String, leader: Option<((f64, f64), (f64, f64))>) {
    let Some((from, to)) = leader
    else {
        return;
    };
    let _ = writeln!(
        text,
        "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#9ca3af\" stroke-width=\"0.9\"/>",
        from.0, from.1, to.0, to.1
    );
}

/// Orders the eight directions so the one closest to `preferred` is probed
/// first, keeping a label on the side of the member or node it belongs to.
fn directions_towards(preferred: (f64, f64)) -> [(f64, f64); 8] {
    let length = (preferred.0 * preferred.0 + preferred.1 * preferred.1).sqrt();
    let preferred = if length > 1.0e-9 { (preferred.0 / length, preferred.1 / length) } else { (0.0, -1.0) };
    let mut directions = LABEL_DIRECTIONS;
    directions.sort_by(|a, b| {
        let score = |value: &(f64, f64)| value.0 * preferred.0 + value.1 * preferred.1;
        score(b).partial_cmp(&score(a)).unwrap_or(std::cmp::Ordering::Equal)
    });
    directions
}

/// Screen-space centroid of a projected point set.
fn centroid(points: &[(f64, f64)]) -> (f64, f64) {
    let count = points.len().max(1) as f64;
    let sum = points.iter().fold((0.0, 0.0), |acc, point| (acc.0 + point.0, acc.1 + point.1));
    (sum.0 / count, sum.1 / count)
}

/// Places a node id on the side facing away from the model centre. A support
/// node keeps its id on the outside of the triangle, and a node under a load
/// arrow avoids the space above it, so neither symbol is overprinted.
fn place_node_label(
    placer: &mut LabelPlacer,
    id: &str,
    point: (f64, f64),
    center: (f64, f64),
    supported: bool,
    avoid_above: bool,
) -> PlacedLabel {
    let width = label_width(id);
    let away = (point.0 - center.0, point.1 - center.1);
    let preferred = if supported {
        (0.0, -1.0)
    }
    else if avoid_above {
        if away.0 >= 0.0 { (1.0, -0.2) } else { (-1.0, -0.2) }
    }
    else if away.0.abs() + away.1.abs() > 1.0e-6 {
        away
    }
    else {
        (0.0, -1.0)
    };
    placer.place_near(point, width, &directions_towards(preferred))
}

/// Writes the member id of every element beside its undeformed midpoint and,
/// when a solution is supplied, the signed axial force with its unit on the
/// opposite side of the member. Both labels are pushed along the member normal
/// until they clear every line and every earlier label, and fall back to a
/// leader line rather than crossing the member.
fn write_member_annotations(
    text: &mut String,
    placer: &mut LabelPlacer,
    model: &Model,
    solution: Option<&Solution>,
    endpoints: &[(usize, usize)],
    undeformed: &[(f64, f64)],
    projection: &Projection,
) {
    for (index, element) in model.elements.iter().enumerate() {
        let (from, to) = endpoints[index];
        let (x1, y1) = projection.project(undeformed[from].0, undeformed[from].1);
        let (x2, y2) = projection.project(undeformed[to].0, undeformed[to].1);
        let (dx, dy) = (x2 - x1, y2 - y1);
        let length = (dx * dx + dy * dy).sqrt();
        if !length.is_finite() || length <= 0.0 {
            continue;
        }
        let (nx, ny) = (-dy / length, dx / length);
        let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        let id_width = label_width(&element.id);
        let placed = placer.place_near((mx, my), id_width, &directions_towards((nx, ny)));
        write_leader(text, placed.leader);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{FONT_LABEL}\" font-weight=\"600\" fill=\"#111827\">{}</text>",
            placed.x,
            placed.y,
            xml_escape(&element.id)
        );
        if let Some(solution) = solution {
            let force = solution.elements[index].axial_force;
            if force.is_finite() && force.abs() > 0.0 {
                let sign = if force > 0.0 { "+" } else { "" };
                let label = format!("N = {sign}{} N", format_contour_value(force));
                let width = label_width(&label);
                let placed = placer.place_near((mx, my), width, &directions_towards((-nx, -ny)));
                write_leader(text, placed.leader);
                let colour = if force > 0.0 { "#b91c1c" } else { "#1d4ed8" };
                let _ = writeln!(
                    text,
                    "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{FONT_LABEL}\" fill=\"{colour}\">{}</text>",
                    placed.x,
                    placed.y,
                    xml_escape(&label)
                );
            }
        }
    }
}

/// Builds the deformed centreline of one member in model coordinates.
///
/// Two-dimensional `beam` members bend along the cubic Hermite interpolation
/// of the endpoint transverse displacements and end rotations, so the
/// drawing shows the curvature the Euler-Bernoulli element actually carries
/// instead of a straight chord between the displaced end nodes. The axial
/// extension is applied linearly along the member. Truss bars and `beam3d`
/// members keep the straight endpoint-to-endpoint line.
fn deformed_polyline_points(
    from: (f64, f64, f64),
    to: (f64, f64, f64),
    from_disp: (f64, f64, f64, f64),
    to_disp: (f64, f64, f64, f64),
    kind: ElementKind,
    scale: f64,
) -> Vec<(f64, f64, f64)> {
    let axis = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let length = (axis.0 * axis.0 + axis.1 * axis.1 + axis.2 * axis.2).sqrt();
    let straight = |index: usize| {
        let (base, disp) = if index == 0 { (from, from_disp) } else { (to, to_disp) };
        (base.0 + scale * disp.0, base.1 + scale * disp.1, base.2 + scale * disp.2)
    };
    if kind != ElementKind::Beam || !length.is_finite() || length <= 0.0 {
        return vec![straight(0), straight(1)];
    }
    let unit = (axis.0 / length, axis.1 / length, axis.2 / length);
    // The bending plane of a planar beam is spanned by the member axis and
    // the global Z axis; the transverse unit vector is `z × axis`.
    let normal = (-unit.1, unit.0, 0.0);
    let normal_length = (normal.0 * normal.0 + normal.1 * normal.1).sqrt();
    if normal_length <= 0.0 {
        return vec![straight(0), straight(1)];
    }
    let normal = (normal.0 / normal_length, normal.1 / normal_length, 0.0);
    let axial = |disp: (f64, f64, f64, f64)| disp.0 * unit.0 + disp.1 * unit.1 + disp.2 * unit.2;
    let transverse = |disp: (f64, f64, f64, f64)| disp.0 * normal.0 + disp.1 * normal.1 + disp.2 * normal.2;
    let axial1 = axial(from_disp) * scale;
    let axial2 = axial(to_disp) * scale;
    let v1 = transverse(from_disp) * scale;
    let v2 = transverse(to_disp) * scale;
    let slope1 = from_disp.3 * scale;
    let slope2 = to_disp.3 * scale;
    const SAMPLES: usize = 12;
    let mut points = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let s = index as f64 / (SAMPLES - 1) as f64;
        let s2 = s * s;
        let s3 = s2 * s;
        let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
        let h10 = s3 - 2.0 * s2 + s;
        let h01 = -2.0 * s3 + 3.0 * s2;
        let h11 = s3 - s2;
        let axial = axial1 + (axial2 - axial1) * s;
        let v = h00 * v1 + h10 * length * slope1 + h01 * v2 + h11 * length * slope2;
        points.push((
            from.0 + axis.0 * s + unit.0 * axial + normal.0 * v,
            from.1 + axis.1 * s + unit.1 * axial + normal.1 * v,
            from.2 + axis.2 * s + unit.2 * axial + normal.2 * v,
        ));
    }
    points
}

/// Deformed centrelines of every member, projected onto the drawing plane.
fn deformed_member_curves(
    model: &Model,
    solution: &Solution,
    scale: f64,
    project_point: impl Fn(f64, f64, f64) -> (f64, f64),
) -> Result<Vec<Vec<(f64, f64)>>, CaeError> {
    let mut curves = Vec::with_capacity(model.elements.len());
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
        let points = deformed_polyline_points(
            (model.nodes[from].x, model.nodes[from].y, model.nodes[from].z),
            (model.nodes[to].x, model.nodes[to].y, model.nodes[to].z),
            (solution.nodes[from].ux, solution.nodes[from].uy, solution.nodes[from].uz, solution.nodes[from].rz),
            (solution.nodes[to].ux, solution.nodes[to].uy, solution.nodes[to].uz, solution.nodes[to].rz),
            element.kind,
            scale,
        );
        curves.push(points.iter().map(|&(x, y, z)| project_point(x, y, z)).collect());
    }
    Ok(curves)
}

/// Writes a deterministic SVG sheet of nodal displacement vectors.
///
/// The undeformed outline is drawn first as dashed grey lines, then the
/// members of the deformed shape as blue polylines scaled by `scale`; beam
/// members follow their cubic Hermite deflection curve instead of a straight
/// chord. All geometry is drawn before all text, and every label picks a
/// position that does not overprint a label placed before it. On
/// top of that,
/// every node with a non-zero displacement gets a green arrow starting at the
/// deformed node position and pointing along the projected displacement vector
/// `(ux, uy, uz)`. The arrow length is proportional to the displacement
/// magnitude, normalised so the largest magnitude spans 34 px, and the tip is
/// labelled with the magnitude and the displacement unit to four significant
/// digits. The legend names the deformation scale, the proportionality of the
/// arrows and the maximum magnitude, so the drawing and the `nodes.csv`
/// results agree. `scale` must be finite and positive, otherwise the export
/// fails with `export/invalid-scale`; an empty model fails with
/// `export/no-solution`.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise `path`
/// is treated as a directory and `displacements.svg` is created inside it.
pub fn export_displacement_svg(model: &Model, solution: &Solution, scale: f64, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Displacement.default_file_name())
    };
    write_atomic(&target, &displacement_svg_text(model, solution, scale)?)
}

/// Builds the displacement-vector sheet as SVG text without touching the
/// file system, so the same bytes back the SVG and the PDF export.
pub fn displacement_svg_text(model: &Model, solution: &Solution, scale: f64) -> Result<String, CaeError> {
    if model.nodes.is_empty() {
        return Err(CaeError::new("export/no-solution", "the model has no nodes, so there is no displacement to draw"));
    }
    check_alignment(model, solution)?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(CaeError::new(
            "export/invalid-scale",
            format!("the deformed-shape scale must be a finite positive number, but is {scale}"),
        ));
    }

    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let project_point = |x: f64, y: f64, z: f64| (x + oblique_x * z, y + oblique_y * z);
    let undeformed: Vec<(f64, f64)> = model.nodes.iter().map(|node| project_point(node.x, node.y, node.z)).collect();
    let deformed: Vec<(f64, f64)> = model
        .nodes
        .iter()
        .zip(&solution.nodes)
        .map(|(node, result)| project_point(node.x + scale * result.ux, node.y + scale * result.uy, node.z + scale * result.uz))
        .collect();
    let curves = deformed_member_curves(model, solution, scale, project_point)?;
    let magnitudes: Vec<f64> = solution
        .nodes
        .iter()
        .map(|result| (result.ux * result.ux + result.uy * result.uy + result.uz * result.uz).sqrt())
        .collect();
    let max_magnitude = magnitudes.iter().copied().fold(0.0, f64::max);
    let unit = ContourQuantity::Displacement.unit(model.units.unwrap_or(Units::Millimeter));

    let projection = Projection::fit(undeformed.iter().chain(&deformed).chain(curves.iter().flatten()).copied().collect())?;
    let project = |x: f64, y: f64| projection.project(x, y);
    let meta =
        SheetMeta::new(model, DrawingKind::Displacement.title(), DrawingKind::Displacement.number(), format!("×{scale:.2}"));

    let mut text = String::new();
    write_sheet_open(&mut text, &meta);
    text.push_str(
        "<defs>\n<marker id=\"ocae-displacement-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" \
         markerHeight=\"6\" orient=\"auto\" markerUnits=\"strokeWidth\">\n<path d=\"M0,0 L10,5 L0,10 z\" \
         fill=\"#16a34a\"/>\n</marker>\n</defs>\n",
    );
    text.push_str("<g stroke-linecap=\"round\">\n");
    let mut blocked: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let mut symbol_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();

    let mut endpoints = Vec::with_capacity(model.elements.len());
    // Every geometry is drawn before every label, so no member, arrow or
    // marker can overprint text that was placed earlier. The undeformed
    // dashed outline comes first so the blue deformed members stay visible
    // on top of it wherever the two shapes coincide.
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
        let (x1, y1) = project(undeformed[from].0, undeformed[from].1);
        let (x2, y2) = project(undeformed[to].0, undeformed[to].1);
        blocked.push(((x1, y1), (x2, y2)));
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#9ca3af\" stroke-width=\"1.2\" \
             stroke-dasharray=\"4 3\"/>"
        );
    }
    // Beam members follow their Hermite deflection curve; bars stay straight.
    for curve in &curves {
        let mut points = String::new();
        let mut projected = Vec::with_capacity(curve.len());
        for &(x, y) in curve {
            let (sx, sy) = project(x, y);
            projected.push((sx, sy));
            let _ = write!(points, "{sx:.2},{sy:.2} ");
        }
        for window in projected.windows(2) {
            blocked.push((window[0], window[1]));
        }
        let _ =
            writeln!(text, "<polyline points=\"{}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\"/>", points.trim_end(),);
    }
    for point in &deformed {
        let (cx, cy) = project(point.0, point.1);
        symbol_boxes.push((cx - 4.0, cy - 4.0, cx + 4.0, cy + 4.0));
        let _ = writeln!(text, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"3\" fill=\"#111827\"/>");
    }
    let mut arrows = Vec::with_capacity(solution.nodes.len());
    for (index, result) in solution.nodes.iter().enumerate() {
        let magnitude = magnitudes[index];
        if magnitude <= 0.0 || !magnitude.is_finite() {
            continue;
        }
        let (cx, cy) = project(deformed[index].0, deformed[index].1);
        // The displacement is a vector, so the same linear map projects its
        // direction; a pure Z displacement becomes an inclined arrow in 3D.
        let (vector_x, vector_y) = project_point(result.ux, result.uy, result.uz);
        let vector_length = (vector_x * vector_x + vector_y * vector_y).sqrt();
        if vector_length <= 0.0 || !vector_length.is_finite() {
            continue;
        }
        let direction_x = vector_x / vector_length;
        let direction_y = -vector_y / vector_length;
        let arrow_length = 34.0 * (magnitude / max_magnitude);
        let tx = cx + direction_x * arrow_length;
        let ty = cy + direction_y * arrow_length;
        blocked.push(((cx, cy), (tx, ty)));
        let _ = writeln!(
            text,
            "<line x1=\"{cx:.2}\" y1=\"{cy:.2}\" x2=\"{tx:.2}\" y2=\"{ty:.2}\" stroke=\"#16a34a\" stroke-width=\"2\" \
             marker-end=\"url(#ocae-displacement-arrow)\"/>"
        );
        arrows.push((tx, ty, direction_x, direction_y, magnitude));
    }
    text.push_str("</g>\n");

    // Labels are drawn last and avoid the geometry as well as the labels
    // placed before them, so no value lands on a member, an arrow or another
    // label.
    text.push_str("<g font-size=\"11\">\n");
    let mut placer = LabelPlacer::within(PLOT_LEFT + 2.0, PLOT_RIGHT - 2.0, PLOT_TOP + 2.0, PLOT_BOTTOM - 2.0);
    for (from, to) in &blocked {
        placer.block_line(*from, *to);
    }
    for rect in symbol_boxes {
        placer.block_box(rect);
    }
    let center = centroid(&deformed.iter().map(|&(x, y)| project(x, y)).collect::<Vec<_>>());
    for (index, node) in model.nodes.iter().enumerate() {
        let point = project(deformed[index].0, deformed[index].1);
        let placed = place_node_label(&mut placer, &node.id, point, center, false, false);
        write_leader(&mut text, placed.leader);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"#374151\">{}</text>",
            placed.x,
            placed.y,
            xml_escape(&node.id)
        );
    }
    write_member_annotations(&mut text, &mut placer, model, None, &endpoints, &undeformed, &projection);
    for (tx, ty, direction_x, direction_y, magnitude) in arrows {
        let label = format!("{} {unit}", format_contour_value(magnitude));
        let (lx, ly) = placer.place(tx - direction_y * 12.0 + 2.0, ty + direction_x * 12.0 + 2.0, label_width(&label));
        let _ = writeln!(text, "<text x=\"{lx:.2}\" y=\"{ly:.2}\" fill=\"#166534\">{}</text>", xml_escape(&label));
    }
    text.push_str("</g>\n");

    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"{PLOT_LEFT}\" y1=\"{LEGEND_BASELINE}\" x2=\"{}\" y2=\"{LEGEND_BASELINE}\" \
         stroke=\"#16a34a\" stroke-width=\"2\"/>",
        PLOT_LEFT + 26.0
    );
    let _ = writeln!(
        text,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">displacement vector</text>",
        PLOT_LEFT + 34.0,
        LEGEND_BASELINE + 4.0
    );
    let _ = writeln!(
        text,
        "<text x=\"280\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">deformation × {:.2} · arrow length ∝ |u| · max |u| = {} \
         {unit}</text>",
        LEGEND_BASELINE + 4.0,
        scale,
        format_contour_value(max_magnitude),
    );
    write_sheet_close(
        &mut text,
        &meta,
        &[
            "dashed grey: undeformed members; solid blue: deformed centreline at the printed scale.",
            "green arrows: nodal displacement vectors, length proportional to the magnitude; every node and member is labelled.",
        ],
    );

    Ok(text)
}

/// Writes a deterministic SVG sheet of support reaction vectors.
///
/// The members are drawn as blue lines at their undeformed positions, the
/// nodes as black dots with their ids and every constrained support as a
/// green triangle, exactly like the plain SVG drawing. On top of that, every
/// support of the solution's reaction summary — the same per
/// support rows `reactions.csv` writes — gets a purple arrow starting at the
/// node and pointing along the projected reaction vector `(fx, fy, fz)`. The
/// arrow length is proportional to the translational magnitude, normalised so
/// the largest magnitude spans 34 px, and the tip is labelled with the
/// magnitude in newtons; a non-zero reaction moment is labelled next to the
/// node with its component and the moment unit. The legend names the
/// proportionality of the arrows and the maximum magnitude, so the drawing
/// and the reaction tables agree. An empty model fails with
/// `export/no-solution` and a model without supports fails with
/// `export/no-reactions` instead of drawing a meaningless sheet.
///
/// All geometry is drawn before all text, and every magnitude or moment
/// label picks a position that does not overprint a label placed before it.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise `path`
/// is treated as a directory and `reactions.svg` is created inside it.
pub fn export_reactions_svg(model: &Model, solution: &Solution, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Reactions.default_file_name())
    };
    write_atomic(&target, &reactions_svg_text(model, solution)?)
}

/// Builds the reaction-vector sheet as SVG text without touching the file
/// system, so the same bytes back the SVG and the PDF export.
pub fn reactions_svg_text(model: &Model, solution: &Solution) -> Result<String, CaeError> {
    if model.nodes.is_empty() {
        return Err(CaeError::new("export/no-solution", "the model has no nodes, so there is no reaction to draw"));
    }
    check_alignment(model, solution)?;
    if model.supports.is_empty() {
        return Err(CaeError::new("export/no-reactions", "the model has no supports, so there is no reaction to draw"));
    }

    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let project_point = |x: f64, y: f64, z: f64| (x + oblique_x * z, y + oblique_y * z);
    let geometry: Vec<(f64, f64)> = model.nodes.iter().map(|node| project_point(node.x, node.y, node.z)).collect();

    let projection = Projection::fit(geometry.clone())?;
    let project = |x: f64, y: f64| projection.project(x, y);

    let units = model.units.unwrap_or(Units::Millimeter);
    let moment_unit = ContourQuantity::Moment.unit(units);
    let magnitudes: Vec<f64> = solution
        .reaction_summary
        .supports
        .iter()
        .map(|support| (support.fx * support.fx + support.fy * support.fy + support.fz * support.fz).sqrt())
        .collect();
    let max_magnitude = magnitudes.iter().copied().fold(0.0, f64::max);

    let meta = SheetMeta::new(model, DrawingKind::Reactions.title(), DrawingKind::Reactions.number(), "1:1 geometry");
    let mut text = String::new();
    write_sheet_open(&mut text, &meta);
    text.push_str(
        "<defs>\n<marker id=\"ocae-reaction-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" \
         markerHeight=\"6\" orient=\"auto\" markerUnits=\"strokeWidth\">\n<path d=\"M0,0 L10,5 L0,10 z\" \
         fill=\"#7c3aed\"/>\n</marker>\n</defs>\n",
    );
    text.push_str("<g stroke-linecap=\"round\">\n");
    let mut blocked: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let mut symbol_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();

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
        let (x1, y1) = project(geometry[from].0, geometry[from].1);
        let (x2, y2) = project(geometry[to].0, geometry[to].1);
        blocked.push(((x1, y1), (x2, y2)));
        let _ = writeln!(
            text,
            "<polyline points=\"{x1:.2},{y1:.2} {x2:.2},{y2:.2}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\"/>"
        );
    }
    let mut supported_nodes = vec![false; model.nodes.len()];
    for support in &model.supports {
        let Some(index) = model.nodes.iter().position(|node| node.id == support.node)
        else {
            continue;
        };
        let constrained = support_is_constrained(support);
        supported_nodes[index] = constrained;
        if !constrained {
            continue;
        }
        let (cx, cy) = project(geometry[index].0, geometry[index].1);
        symbol_boxes.push((cx - 11.0, cy + 4.0, cx + 11.0, cy + 16.0));
        // The triangle apex sits just below the node dot so the symbol never
        // covers the joint itself, and a short ground line closes the base.
        let _ = writeln!(
            text,
            "<polygon points=\"{cx:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"#16a34a\"/>",
            cy + 4.0,
            cx - 8.0,
            cy + 15.0,
            cx + 8.0,
            cy + 15.0,
        );
        let _ = writeln!(
            text,
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#16a34a\" stroke-width=\"1.5\"/>",
            cx - 11.0,
            cy + 15.0,
            cx + 11.0,
            cy + 15.0,
        );
    }
    for point in &geometry {
        let (cx, cy) = project(point.0, point.1);
        symbol_boxes.push((cx - 4.0, cy - 4.0, cx + 4.0, cy + 4.0));
        let _ = writeln!(text, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"3\" fill=\"#111827\"/>");
    }
    // One entry per support: node anchor, optional arrow tip and magnitude,
    // and the non-zero reaction moments; the text is emitted after the
    // geometry so no arrow can overprint a label.
    type ReactionLabel = (f64, f64, Option<(f64, f64, f64, f64, f64)>, Vec<(f64, &'static str)>);
    let mut labels: Vec<ReactionLabel> = Vec::new();
    for (position, support) in solution.reaction_summary.supports.iter().enumerate() {
        let Some(index) = model.nodes.iter().position(|node| node.id == support.node)
        else {
            continue;
        };
        let (cx, cy) = project(geometry[index].0, geometry[index].1);
        let magnitude = magnitudes[position];
        let mut arrow = None;
        if magnitude > 0.0 && magnitude.is_finite() {
            // The reaction is a vector, so the same linear map projects its
            // direction; a pure Z reaction becomes an inclined arrow in 3D.
            let (vector_x, vector_y) = project_point(support.fx, support.fy, support.fz);
            let vector_length = (vector_x * vector_x + vector_y * vector_y).sqrt();
            if vector_length > 0.0 && vector_length.is_finite() {
                let direction_x = vector_x / vector_length;
                let direction_y = -vector_y / vector_length;
                let arrow_length = 34.0 * (magnitude / max_magnitude);
                let tx = cx + direction_x * arrow_length;
                let ty = cy + direction_y * arrow_length;
                blocked.push(((cx, cy), (tx, ty)));
                let _ = writeln!(
                    text,
                    "<line x1=\"{cx:.2}\" y1=\"{cy:.2}\" x2=\"{tx:.2}\" y2=\"{ty:.2}\" stroke=\"#7c3aed\" \
                     stroke-width=\"2\" marker-end=\"url(#ocae-reaction-arrow)\"/>"
                );
                arrow = Some((tx, ty, direction_x, direction_y, magnitude));
            }
        }
        let moments: Vec<(f64, &'static str)> = [("Mx", support.mx), ("My", support.my), ("Mz", Some(support.mz))]
            .into_iter()
            .filter_map(|(component, value)| {
                value.filter(|value| *value != 0.0 && value.is_finite()).map(|value| (value, component))
            })
            .collect();
        labels.push((cx, cy, arrow, moments));
    }
    text.push_str("</g>\n");

    // Labels are drawn after every geometry and avoid the drawn lines as well
    // as the labels placed before them.
    text.push_str("<g font-size=\"11\">\n");
    let mut placer = LabelPlacer::within(PLOT_LEFT + 2.0, PLOT_RIGHT - 2.0, PLOT_TOP + 2.0, PLOT_BOTTOM - 2.0);
    for (from, to) in &blocked {
        placer.block_line(*from, *to);
    }
    for rect in symbol_boxes {
        placer.block_box(rect);
    }
    let center = centroid(&geometry.iter().map(|&(x, y)| project(x, y)).collect::<Vec<_>>());
    for (index, node) in model.nodes.iter().enumerate() {
        let point = project(geometry[index].0, geometry[index].1);
        let placed = place_node_label(&mut placer, &node.id, point, center, supported_nodes[index], false);
        write_leader(&mut text, placed.leader);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"#374151\">{}</text>",
            placed.x,
            placed.y,
            xml_escape(&node.id)
        );
    }
    write_member_annotations(&mut text, &mut placer, model, None, &endpoints, &geometry, &projection);
    for (cx, cy, arrow, moments) in &labels {
        if let Some((tx, ty, direction_x, direction_y, magnitude)) = arrow {
            // Offset the value to the side of the arrow instead of printing it
            // on top of the shaft or the node id.
            let label = format!("{} N", format_contour_value(*magnitude));
            let (lx, ly) = placer.place(tx - direction_y * 12.0 + 2.0, ty + direction_x * 12.0 + 2.0, label_width(&label));
            let _ = writeln!(text, "<text x=\"{lx:.2}\" y=\"{ly:.2}\" fill=\"#5b21b6\">{}</text>", xml_escape(&label));
        }
        let mut moment_y = cy + 28.0;
        for (value, component) in moments {
            let label = format!("{component} {} {moment_unit}", format_contour_value(*value));
            let (lx, ly) = placer.place(cx + 6.0, moment_y, label_width(&label));
            let _ = writeln!(text, "<text x=\"{lx:.2}\" y=\"{ly:.2}\" fill=\"#5b21b6\">{}</text>", xml_escape(&label));
            moment_y += 14.0;
        }
    }
    text.push_str("</g>\n");

    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"{PLOT_LEFT}\" y1=\"{LEGEND_BASELINE}\" x2=\"{}\" y2=\"{LEGEND_BASELINE}\" \
         stroke=\"#7c3aed\" stroke-width=\"2\"/>",
        PLOT_LEFT + 26.0
    );
    let _ = writeln!(
        text,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">reaction vector</text>",
        PLOT_LEFT + 34.0,
        LEGEND_BASELINE + 4.0
    );
    let _ = writeln!(
        text,
        "<text x=\"260\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">arrow length ∝ |R| · max |R| = {} N · moments labelled in \
         {moment_unit}</text>",
        LEGEND_BASELINE + 4.0,
        format_contour_value(max_magnitude),
    );
    write_sheet_close(
        &mut text,
        &meta,
        &[
            "green triangles: constrained supports; purple arrows: reaction vectors with their magnitude.",
            "member ids mark every element; reaction moments are labelled next to their node.",
        ],
    );

    Ok(text)
}

/// Writes a deterministic SVG sheet overlaying the deformed shape on the
/// undeformed skeleton.
///
/// The undeformed members are solid black lines and the deformed centreline
/// is a blue curve (beam members follow their Hermite deflection curve),
/// scaled by `scale`. Only the node with the largest displacement is
/// annotated, with its id and magnitude; there are no arrows and no load
/// symbols, so the two shapes can be compared without clutter. `scale` must
/// be finite and positive, otherwise the export fails with
/// `export/invalid-scale`; an empty model fails with `export/no-solution`.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise
/// `path` is treated as a directory and `overlay.svg` is created inside it.
pub fn export_overlay_svg(model: &Model, solution: &Solution, scale: f64, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Overlay.default_file_name())
    };
    write_atomic(&target, &overlay_svg_text(model, solution, scale)?)
}

/// Builds the deformation-overlay sheet as SVG text without touching the
/// file system, so the same bytes back the SVG and the PDF export.
pub fn overlay_svg_text(model: &Model, solution: &Solution, scale: f64) -> Result<String, CaeError> {
    if model.nodes.is_empty() {
        return Err(CaeError::new("export/no-solution", "the model has no nodes, so there is no shape to overlay"));
    }
    check_alignment(model, solution)?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(CaeError::new(
            "export/invalid-scale",
            format!("the deformed-shape scale must be a finite positive number, but is {scale}"),
        ));
    }

    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let project_point = |x: f64, y: f64, z: f64| (x + oblique_x * z, y + oblique_y * z);
    let undeformed: Vec<(f64, f64)> = model.nodes.iter().map(|node| project_point(node.x, node.y, node.z)).collect();
    let curves = deformed_member_curves(model, solution, scale, project_point)?;
    let magnitudes: Vec<f64> = solution.nodes.iter().map(|result| result.magnitude).collect();
    let unit = ContourQuantity::Displacement.unit(model.units.unwrap_or(Units::Millimeter));

    let projection = Projection::fit(undeformed.iter().chain(curves.iter().flatten()).copied().collect())?;
    let project = |x: f64, y: f64| projection.project(x, y);
    let meta = SheetMeta::new(model, DrawingKind::Overlay.title(), DrawingKind::Overlay.number(), format!("×{scale:.2}"));

    let mut text = String::new();
    write_sheet_open(&mut text, &meta);
    text.push_str("<g stroke-linecap=\"round\">\n");
    let mut blocked: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let mut symbol_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();
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
        let (x1, y1) = project(undeformed[from].0, undeformed[from].1);
        let (x2, y2) = project(undeformed[to].0, undeformed[to].1);
        blocked.push(((x1, y1), (x2, y2)));
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#9ca3af\" stroke-width=\"1.2\" \
             stroke-dasharray=\"4 3\"/>"
        );
    }
    for curve in &curves {
        let mut points = String::new();
        let mut projected = Vec::with_capacity(curve.len());
        for &(x, y) in curve {
            let (sx, sy) = project(x, y);
            projected.push((sx, sy));
            let _ = write!(points, "{sx:.2},{sy:.2} ");
        }
        for window in projected.windows(2) {
            blocked.push((window[0], window[1]));
        }
        let _ =
            writeln!(text, "<polyline points=\"{}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\"/>", points.trim_end(),);
    }
    for &(x, y) in &undeformed {
        let (cx, cy) = project(x, y);
        symbol_boxes.push((cx - 3.5, cy - 3.5, cx + 3.5, cy + 3.5));
        let _ = writeln!(text, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"2.5\" fill=\"#111827\"/>");
    }
    text.push_str("</g>\n");

    text.push_str("<g font-size=\"11\">\n");
    let mut placer = LabelPlacer::within(PLOT_LEFT + 2.0, PLOT_RIGHT - 2.0, PLOT_TOP + 2.0, PLOT_BOTTOM - 2.0);
    for (from, to) in &blocked {
        placer.block_line(*from, *to);
    }
    for rect in symbol_boxes {
        placer.block_box(rect);
    }
    let center = centroid(&undeformed.iter().map(|&(x, y)| project(x, y)).collect::<Vec<_>>());
    for (index, node) in model.nodes.iter().enumerate() {
        let point = project(undeformed[index].0, undeformed[index].1);
        let placed = place_node_label(&mut placer, &node.id, point, center, false, false);
        write_leader(&mut text, placed.leader);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"#374151\">{}</text>",
            placed.x,
            placed.y,
            xml_escape(&node.id)
        );
    }
    write_member_annotations(&mut text, &mut placer, model, None, &endpoints, &undeformed, &projection);
    if let Some((index, magnitude)) = magnitudes
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, magnitude)| *magnitude > 0.0 && magnitude.is_finite())
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    {
        let node = &model.nodes[index];
        let (cx, cy) = project(undeformed[index].0, undeformed[index].1);
        let label = format!("{} · {} {unit}", node.id, format_contour_value(magnitude));
        let (lx, ly) = placer.place(cx + 16.0, cy + 4.0, label_width(&label));
        let _ = writeln!(text, "<text x=\"{lx:.2}\" y=\"{ly:.2}\" fill=\"#1d4ed8\">{}</text>", xml_escape(&label));
    }
    text.push_str("</g>\n");

    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"{PLOT_LEFT}\" y1=\"{LEGEND_BASELINE}\" x2=\"{}\" y2=\"{LEGEND_BASELINE}\" \
         stroke=\"#9ca3af\" stroke-width=\"1.2\" stroke-dasharray=\"4 3\"/>",
        PLOT_LEFT + 26.0
    );
    let _ = writeln!(
        text,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">undeformed</text>",
        PLOT_LEFT + 34.0,
        LEGEND_BASELINE + 4.0
    );
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"250\" y1=\"{LEGEND_BASELINE}\" x2=\"276\" y2=\"{LEGEND_BASELINE}\" stroke=\"#2563eb\" \
         stroke-width=\"2\"/>"
    );
    let _ = writeln!(
        text,
        "<text x=\"284\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">deformed × {scale:.2}</text>",
        LEGEND_BASELINE + 4.0,
    );
    let _ = writeln!(
        text,
        "<text x=\"{PLOT_RIGHT}\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\" text-anchor=\"end\">max displacement node \
         annotated</text>",
        LEGEND_BASELINE + 4.0,
    );
    write_sheet_close(
        &mut text,
        &meta,
        &[
            "grey dashed: undeformed members; blue: deformed centreline at the printed scale.",
            "every node and member is labelled; the largest displacement node also carries its magnitude.",
        ],
    );

    Ok(text)
}

/// Writes a deterministic SVG sheet of the mode shapes of a modal solution.
///
/// One panel per mode (up to five, in ascending frequency order) overlays
/// the scaled mode shape as a blue curve on the grey undeformed skeleton;
/// the panel title names the mode number and the natural frequency in
/// hertz, so the sheet and the modal JSON report agree. A modal solution
/// without usable modes fails with `export/no-modes`; an empty model fails
/// with `export/no-solution`.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise
/// `path` is treated as a directory and `modes.svg` is created inside it.
pub fn export_modal_svg(model: &Model, modal: &ModalSolution, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Modal.default_file_name())
    };
    write_atomic(&target, &modal_svg_text(model, modal)?)
}

/// Builds the mode-shape sheet as SVG text without touching the file
/// system, so the same bytes back the SVG and the PDF export.
pub fn modal_svg_text(model: &Model, modal: &ModalSolution) -> Result<String, CaeError> {
    if model.nodes.is_empty() {
        return Err(CaeError::new("export/no-solution", "the model has no nodes, so there is no mode shape to draw"));
    }
    if modal.modes.is_empty() {
        return Err(CaeError::new("export/no-modes", "the modal analysis produced no usable mode shape to draw"));
    }
    if modal.modes.iter().any(|mode| mode.shape.len() != model.nodes.len()) {
        return Err(CaeError::new(
            "export/invalid-model",
            "mode shapes must carry one entry per model node before an SVG can be drawn",
        ));
    }

    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let project_point = |x: f64, y: f64, z: f64| (x + oblique_x * z, y + oblique_y * z);
    let geometry: Vec<(f64, f64)> = model.nodes.iter().map(|node| project_point(node.x, node.y, node.z)).collect();

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &(x, y) in &geometry {
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

    // A 2×2 grid shows the four lowest modes; more panels would shrink the
    // mode shapes below a readable size.
    let modes: Vec<&crate::solver::Mode> = modal.modes.iter().take(4).collect();
    let columns = 2;
    let rows = modes.len().div_ceil(columns);
    let panel_width = (PLOT_RIGHT - PLOT_LEFT) / columns as f64;
    let slot_height = PLOT_BOTTOM - PLOT_TOP;
    // Panels are capped so a two-mode sheet does not stretch each panel over
    // the whole page; a short grid is centred vertically.
    let panel_height = (slot_height / rows as f64).min(330.0);
    let grid_top = PLOT_TOP + (slot_height - rows as f64 * panel_height) / 2.0;
    let available_width = panel_width - 64.0;
    // The header occupies the top 56 px and the mode shape can swing one
    // amplitude above its undeformed position; reserve both so a peak can
    // never reach the mode label.
    let amplitude = (panel_height * 0.14).min(46.0);
    let available_height = (panel_height - 56.0 - 2.0 * amplitude - 12.0).max(20.0);
    let factor = (available_width / width).min(available_height / height);

    // One amplitude scale shared by every panel, so relative amplitudes are
    // comparable instead of each panel being blown up on its own.
    let max_translation = modes
        .iter()
        .flat_map(|mode| mode.shape.iter())
        .map(|shape| (shape.ux * shape.ux + shape.uy * shape.uy + shape.uz * shape.uz).sqrt())
        .fold(0.0, f64::max);
    let shape_scale = if max_translation > 0.0 && max_translation.is_finite() && factor > 0.0 && factor.is_finite() {
        amplitude / (max_translation * factor)
    }
    else {
        0.0
    };

    let meta = SheetMeta::new(model, DrawingKind::Modal.title(), DrawingKind::Modal.number(), format!("×{shape_scale:.2}"));
    let mut text = String::new();
    write_sheet_open(&mut text, &meta);
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

    for (index, mode) in modes.iter().enumerate() {
        let column = index % columns;
        let row = index / columns;
        let px = PLOT_LEFT + column as f64 * panel_width;
        let py = grid_top + row as f64 * panel_height;
        let origin_x = px + 32.0 + (available_width - width * factor) / 2.0;
        let origin_y = py + 56.0 + amplitude + (available_height - height * factor) / 2.0;
        let project = |x: f64, y: f64| (origin_x + (x - min_x) * factor, origin_y + (max_y - y) * factor);

        let _ = writeln!(
            text,
            "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#ffffff\" stroke=\"#d1d5db\" \
             stroke-width=\"1\"/>",
            px + 4.0,
            py + 4.0,
            panel_width - 8.0,
            panel_height - 8.0,
        );
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"14\" font-weight=\"600\" fill=\"#111827\">mode {} · f = {} Hz</text>",
            px + 12.0,
            py + 24.0,
            index + 1,
            format_contour_value(mode.frequency_hz),
        );
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"11\" fill=\"#6b7280\">eigenvalue λ = {} · max |φ| = {}</text>",
            px + 12.0,
            py + 40.0,
            format_contour_value(mode.eigenvalue),
            format_contour_value(mode.max_amplitude),
        );

        let displaced = |node: usize| -> (f64, f64) {
            let shape = &mode.shape[node];
            project_point(
                model.nodes[node].x + shape_scale * shape.ux,
                model.nodes[node].y + shape_scale * shape.uy,
                model.nodes[node].z + shape_scale * shape.uz,
            )
        };
        for &(from, to) in &endpoints {
            let (x1, y1) = project(geometry[from].0, geometry[from].1);
            let (x2, y2) = project(geometry[to].0, geometry[to].1);
            let _ = writeln!(
                text,
                "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#9ca3af\" stroke-width=\"1.2\" \
                 stroke-dasharray=\"4 3\"/>"
            );
            let (dx1, dy1) = displaced(from);
            let (dx2, dy2) = displaced(to);
            let (dx1, dy1) = project(dx1, dy1);
            let (dx2, dy2) = project(dx2, dy2);
            let _ = writeln!(
                text,
                "<polyline points=\"{dx1:.2},{dy1:.2} {dx2:.2},{dy2:.2}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\"/>"
            );
        }
    }
    text.push_str("</g>\n");

    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"{PLOT_LEFT}\" y1=\"{LEGEND_BASELINE}\" x2=\"{}\" y2=\"{LEGEND_BASELINE}\" \
         stroke=\"#9ca3af\" stroke-width=\"1.2\" stroke-dasharray=\"4 3\"/>",
        PLOT_LEFT + 26.0
    );
    let _ = writeln!(
        text,
        "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">undeformed</text>",
        PLOT_LEFT + 34.0,
        LEGEND_BASELINE + 4.0
    );
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"250\" y1=\"{LEGEND_BASELINE}\" x2=\"276\" y2=\"{LEGEND_BASELINE}\" stroke=\"#2563eb\" \
         stroke-width=\"2\"/>"
    );
    let _ = writeln!(
        text,
        "<text x=\"284\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\">mode shape × {shape_scale:.3}</text>",
        LEGEND_BASELINE + 4.0,
    );
    let _ = writeln!(
        text,
        "<text x=\"{PLOT_RIGHT}\" y=\"{:.2}\" font-size=\"12\" fill=\"#111827\" text-anchor=\"end\">first {} modes · common \
         amplitude scale</text>",
        LEGEND_BASELINE + 4.0,
        modes.len(),
    );
    write_sheet_close(
        &mut text,
        &meta,
        &[
            "mode shapes are mass-normalised; all panels share one amplitude scale, so relative amplitudes are comparable.",
            "grey dashed: undeformed model; blue: mode shape; frequencies and eigenvalues come from the modal report.",
        ],
    );

    Ok(text)
}

/// Writes a deterministic SVG sheet of the force envelope of the solved
/// load combinations.
///
/// One row per member shows three horizontal bars — axial force `N` in
/// cyan, shear `V` in blue and bending moment `M` in red — spanning the
/// `min … max` range of that quantity across every combination, measured on
/// a symmetric axis normalised by the largest absolute value of the same
/// quantity over all members. Each bar is labelled with its signed extremes
/// and the combinations that produced them, so the sheet and the envelope
/// JSON report agree. An envelope without member results fails with
/// `export/no-envelope`.
///
/// When `path` ends in `.svg` it is used as the output file, otherwise
/// `path` is treated as a directory and `envelope.svg` is created inside it.
pub fn export_envelope_svg(model: &Model, report: &EnvelopeReport, path: &Path) -> Result<(), CaeError> {
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join(DrawingKind::Envelope.default_file_name())
    };
    write_atomic(&target, &envelope_svg_text(model, report)?)
}

/// Builds the envelope sheet as SVG text without touching the file system,
/// so the same bytes back the SVG and the PDF export.
pub fn envelope_svg_text(model: &Model, report: &EnvelopeReport) -> Result<String, CaeError> {
    if report.elements.is_empty() {
        return Err(CaeError::new("export/no-envelope", "the envelope report has no member results to draw"));
    }
    let moment_unit = ContourQuantity::Moment.unit(model.units.unwrap_or(Units::Millimeter));

    let limit = |pick: &dyn Fn(&EnvelopeElement) -> &EnvelopeExtreme| {
        report
            .elements
            .iter()
            .map(|element| {
                let extreme = pick(element);
                extreme.min.abs().max(extreme.max.abs())
            })
            .fold(0.0, f64::max)
    };
    let axial_limit = limit(&|element| &element.axial).max(1.0e-12);
    let shear_limit = limit(&|element| &element.shear).max(1.0e-12);
    let moment_limit = limit(&|element| &element.moment).max(1.0e-12);

    let rows = report.elements.len();
    let row_height = ((PLOT_BOTTOM - PLOT_TOP) / rows as f64).min(64.0);
    let track_left = PLOT_LEFT + 110.0;
    let track_right = PLOT_RIGHT - 250.0;
    let track_width = track_right - track_left;

    let meta = SheetMeta::new(
        model,
        DrawingKind::Envelope.title(),
        DrawingKind::Envelope.number(),
        format!("{} combinations", report.combination_count),
    );
    let mut text = String::new();
    write_sheet_open(&mut text, &meta);

    let quantities: [(&str, &str, &str); 3] = [("N", "N", "#0891b2"), ("V", "N", "#2563eb"), ("M", moment_unit, "#b91c1c")];
    for (index, element) in report.elements.iter().enumerate() {
        let py = PLOT_TOP + index as f64 * row_height;
        let extremes = [&element.axial, &element.shear, &element.moment];
        let limits = [axial_limit, shear_limit, moment_limit];
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"12\" font-weight=\"600\" fill=\"#111827\">{}</text>",
            PLOT_LEFT + 4.0,
            py + row_height / 2.0 + 4.0,
            xml_escape(&element.id),
        );
        for (slot, (name, unit, color)) in quantities.iter().enumerate() {
            let baseline = py + 8.0 + slot as f64 * (row_height - 16.0) / 2.0;
            let extreme = extremes[slot];
            let limit = limits[slot];
            let map = |value: f64| track_left + ((value / limit).clamp(-1.0, 1.0) + 1.0) * track_width / 2.0;
            let zero = map(0.0);
            let x_min = map(extreme.min);
            let x_max = map(extreme.max);
            let left = x_min.min(x_max);
            let bar_width = (x_max - x_min).abs().max(1.5);
            let _ = writeln!(
                text,
                "<line x1=\"{track_left:.2}\" y1=\"{baseline:.2}\" x2=\"{track_right:.2}\" y2=\"{baseline:.2}\" \
                 stroke=\"#d1d5db\" stroke-width=\"1\"/>"
            );
            let _ = writeln!(
                text,
                "<line x1=\"{zero:.2}\" y1=\"{:.2}\" x2=\"{zero:.2}\" y2=\"{:.2}\" stroke=\"#9ca3af\" stroke-width=\"1\"/>",
                baseline - 6.0,
                baseline + 10.0,
            );
            let _ = writeln!(
                text,
                "<rect x=\"{left:.2}\" y=\"{:.2}\" width=\"{bar_width:.2}\" height=\"7\" fill=\"{color}\"/>",
                baseline - 3.5,
            );
            let _ = writeln!(
                text,
                "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"10\" fill=\"#374151\">{name} {} … {} {unit} · {} / {}</text>",
                track_right + 10.0,
                baseline + 4.0,
                format_contour_value(extreme.min),
                format_contour_value(extreme.max),
                xml_escape(&extreme.min_combination),
                xml_escape(&extreme.max_combination),
            );
        }
    }

    let combinations_note = format!(
        "bars span the min … max range of every solved case ({} combinations); the grey tick marks zero.",
        report.combination_count
    );
    write_sheet_close(
        &mut text,
        &meta,
        &[combinations_note.as_str(), "N cyan · V blue · M red · controlling combinations labelled per bar."],
    );

    Ok(text)
}

/// Two-dimensional nodes are projected onto the global X–Y plane. In a
/// three-dimensional document the Z axis is drawn as an oblique axis
/// (`x + 0.5·cos30°·z`, `y + 0.5·sin30°·z`) so pure Z members stay visible
/// instead of collapsing to a point. The undeformed members
/// are thin grey lines, the deformed shape after a static solve is a blue
/// polyline scaled by `scale`, nodes are black dots annotated with their node
/// id at the upper right, supported nodes are green triangles and nodal loads
/// are red arrows. The `viewBox` is fixed at
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
    let target = if path.extension().and_then(|extension| extension.to_str()) == Some("svg") {
        path.to_path_buf()
    }
    else {
        path.join("model.svg")
    };
    write_atomic(&target, &svg_with_contour_text(model, solution, scale, contour, stations)?)
}

/// Builds the one-page result summary: model size, load cases, extreme
/// results, a thumbnail sketch and the shared title block.
pub fn summary_svg_text(model: &Model, solution: &Solution) -> Result<String, CaeError> {
    if model.nodes.is_empty() {
        return Err(CaeError::new("export/no-solution", "the model has no nodes, so there is no summary to draw"));
    }
    check_alignment(model, solution)?;
    let units = model.units.unwrap_or(Units::Millimeter);
    let unit = match units {
        Units::Millimeter => "mm",
        Units::Meter => "m",
    };
    let moment_unit = ContourQuantity::Moment.unit(units);
    let stress_unit = match units {
        Units::Millimeter => "N/mm²",
        Units::Meter => "N/m²",
    };

    // Cabinet-oblique projection for 3D documents, the plain XY view for 2D.
    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let project_point = |x: f64, y: f64, z: f64| (x + oblique_x * z, y + oblique_y * z);
    let geometry: Vec<(f64, f64)> = model.nodes.iter().map(|node| project_point(node.x, node.y, node.z)).collect();
    let projection = Projection::fit_in(geometry.clone(), 96.0, 120.0, 436.0, 552.0)?;
    let project = |x: f64, y: f64| projection.project(x, y);

    let meta = SheetMeta::new(model, DrawingKind::Summary.title(), DrawingKind::Summary.number(), "1:1 geometry");
    let mut text = String::new();
    write_sheet_open(&mut text, &meta);
    text.push_str(
        "<defs>\n<marker id=\"ocae-summary-load\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" \
         markerHeight=\"6\" orient=\"auto\" markerUnits=\"strokeWidth\">\n<path d=\"M0,0 L10,5 L0,10 z\" \
         fill=\"#dc2626\"/>\n</marker>\n</defs>\n",
    );

    let _ = writeln!(
        text,
        "<rect x=\"84\" y=\"84\" width=\"364\" height=\"512\" fill=\"#ffffff\" stroke=\"#d1d5db\" stroke-width=\"1\"/>"
    );
    let _ = writeln!(
        text,
        "<text x=\"96\" y=\"102\" font-size=\"{FONT_LABEL}\" letter-spacing=\"0.7\" fill=\"#6b7280\">MODEL SKETCH</text>"
    );

    let mut supported_nodes = vec![false; model.nodes.len()];
    for support in &model.supports {
        if let Some(index) = model.nodes.iter().position(|node| node.id == support.node) {
            supported_nodes[index] = support_is_constrained(support);
        }
    }
    text.push_str("<g stroke-linecap=\"round\">\n");
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
        let (x1, y1) = project(geometry[from].0, geometry[from].1);
        let (x2, y2) = project(geometry[to].0, geometry[to].1);
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#2563eb\" stroke-width=\"1.8\"/>"
        );
    }
    for (index, point) in geometry.iter().enumerate() {
        let (cx, cy) = project(point.0, point.1);
        let _ = writeln!(text, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"2.4\" fill=\"#111827\"/>");
        if supported_nodes[index] {
            let _ = writeln!(
                text,
                "<polygon points=\"{cx:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"#16a34a\"/>",
                cy + 3.0,
                cx - 7.0,
                cy + 12.0,
                cx + 7.0,
                cy + 12.0,
            );
        }
    }
    for load in &model.loads {
        let magnitude = (load.fx * load.fx + load.fy * load.fy + load.fz * load.fz).sqrt();
        if !magnitude.is_finite() || magnitude <= 0.0 {
            continue;
        }
        let Some(index) = model.nodes.iter().position(|node| node.id == load.node)
        else {
            continue;
        };
        let (cx, cy) = project(geometry[index].0, geometry[index].1);
        let (vector_x, vector_y) = project_point(load.fx, load.fy, load.fz);
        let length = (vector_x * vector_x + vector_y * vector_y).sqrt();
        if length <= 0.0 || !length.is_finite() {
            continue;
        }
        let x1 = cx - vector_x / length * 26.0;
        let y1 = cy + vector_y / length * 26.0;
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{cx:.2}\" y2=\"{cy:.2}\" stroke=\"#dc2626\" stroke-width=\"1.8\" \
             marker-end=\"url(#ocae-summary-load)\"/>"
        );
    }
    text.push_str("</g>\n");
    let _ = writeln!(
        text,
        "<text x=\"96\" y=\"582\" font-size=\"{FONT_LABEL}\" fill=\"#6b7280\">blue: members · \
         green: supports · red: loads</text>"
    );

    let constrained = supported_nodes.iter().filter(|supported| **supported).count();
    let truss = model.elements.iter().filter(|element| element.kind == ElementKind::Truss).count();
    let beam = model.elements.iter().filter(|element| element.kind == ElementKind::Beam).count();
    let beam3d = model.elements.iter().filter(|element| element.kind == ElementKind::Beam3D).count();
    let applied_force: f64 =
        model.loads.iter().map(|load| (load.fx * load.fx + load.fy * load.fy + load.fz * load.fz).sqrt()).sum();
    let load_case =
        if model.combinations.is_empty() { "static".to_string() } else { format!("{} combinations", model.combinations.len()) };
    let displacement = (solution.max_displacement.ux * solution.max_displacement.ux
        + solution.max_displacement.uy * solution.max_displacement.uy
        + solution.max_displacement.uz * solution.max_displacement.uz)
        .sqrt();
    let max_stress = solution
        .elements
        .iter()
        .max_by(|a, b| a.stress.abs().partial_cmp(&b.stress.abs()).unwrap_or(std::cmp::Ordering::Equal));
    let max_reaction = solution.reaction_summary.supports.iter().max_by(|a, b| {
        let magnitude = |support: &crate::solver::SupportReaction| {
            (support.fx * support.fx + support.fy * support.fy + support.fz * support.fz).sqrt()
        };
        magnitude(a).partial_cmp(&magnitude(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    let reaction_magnitude = max_reaction
        .map_or(0.0, |support| (support.fx * support.fx + support.fy * support.fy + support.fz * support.fz).sqrt());

    let model_rows: Vec<(&str, String)> = vec![
        ("Nodes", format!("{}", model.nodes.len())),
        ("Members", format!("{} (truss {truss}, beam {beam}, beam3d {beam3d})", model.elements.len())),
        ("Supports", format!("{constrained} constrained / {}", model.supports.len())),
        ("Nodal loads", format!("{} · Σ|F| = {} N", model.loads.len(), format_contour_value(applied_force))),
        ("Dimension", if model.dimension == 3 { "3D spatial".to_string() } else { "2D planar".to_string() }),
        ("Units", format!("{} (force · length)", meta.units)),
        ("Load case", load_case.clone()),
    ];
    let extreme_rows: Vec<(&str, String)> = vec![
        ("max |u|", format!("{} {unit} @ {}", format_contour_value(displacement), solution.max_displacement.id)),
        (
            "max |N|",
            format!("{} N @ {}", format_contour_value(solution.max_axial_force.axial_force), solution.max_axial_force.element),
        ),
        (
            "max |M|",
            format!("{} {moment_unit} @ {}", format_contour_value(solution.max_moment.moment), solution.max_moment.element),
        ),
        (
            "max stress",
            max_stress.map_or_else(
                || format!("0 {stress_unit}"),
                |element| format!("{} {stress_unit} @ {}", format_contour_value(element.stress), element.id),
            ),
        ),
        (
            "max |R|",
            max_reaction.map_or_else(
                || "0 N".to_string(),
                |support| format!("{} N @ {}", format_contour_value(reaction_magnitude), support.node),
            ),
        ),
        ("Strain energy", format!("{} N·{unit}", format_contour_value(solution.strain_energy))),
        (
            "Equilibrium",
            if solution.reaction_summary.balanced {
                format!("balanced (tol {})", format_contour_value(solution.reaction_summary.tolerance))
            }
            else {
                "unbalanced — check supports and loads".to_string()
            },
        ),
    ];

    write_summary_table(&mut text, 470.0, 84.0, 410.0, "MODEL", &model_rows);
    write_summary_table(&mut text, 470.0, 310.0, 410.0, "EXTREMES", &extreme_rows);

    let note = "extreme values are signed; positive axial force is tension, positive moment follows the force-diagram sign convention.";
    let note2 = "per-kind sheets (forces, displacements, reactions, overlay, modal, envelope) carry the full detail.";
    write_sheet_close(&mut text, &meta, &[note, note2]);

    Ok(text)
}

/// Draws one two-column table on the summary sheet. Rows alternate a very
/// light separator so a printed copy stays readable without a heavy grid.
fn write_summary_table(text: &mut String, x: f64, top: f64, width: f64, title: &str, rows: &[(&str, String)]) {
    let _ = writeln!(
        text,
        "<text x=\"{x:.2}\" y=\"{:.2}\" font-size=\"12.5\" font-weight=\"600\" \
         letter-spacing=\"0.5\" fill=\"#111827\">{}</text>",
        top + 4.0,
        title
    );
    let _ = writeln!(
        text,
        "<line x1=\"{x:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#111827\" stroke-width=\"1\"/>",
        top + 10.0,
        x + width,
        top + 10.0,
    );
    const ROW_HEIGHT: f64 = 28.0;
    for (index, (label, value)) in rows.iter().enumerate() {
        let baseline = top + 34.0 + index as f64 * ROW_HEIGHT;
        if index > 0 {
            let _ = writeln!(
                text,
                "<line x1=\"{x:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#e5e7eb\" stroke-width=\"0.8\"/>",
                baseline - 19.0,
                x + width,
                baseline - 19.0,
            );
        }
        let _ =
            writeln!(text, "<text x=\"{x:.2}\" y=\"{baseline:.2}\" font-size=\"{FONT_BODY}\" fill=\"#4b5563\">{label}</text>");
        // Long values (member lists, unit systems) shrink until they fit the
        // value column instead of running under the label.
        let value_size = fit_font_size(value, width - 130.0, FONT_BODY, 8.5);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{baseline:.2}\" font-size=\"{value_size:.2}\" \
             text-anchor=\"end\" fill=\"#111827\">{}</text>",
            x + width,
            xml_escape(value),
        );
    }
}

/// Builds the plain or contour drawing as SVG text without touching the
/// file system, so the same bytes back the SVG and the PDF export.
pub fn svg_with_contour_text(
    model: &Model,
    solution: &Solution,
    scale: f64,
    contour: Option<ContourQuantity>,
    stations: usize,
) -> Result<String, CaeError> {
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

    // A three-dimensional document draws Z as a cabinet oblique axis at 30°
    // with a 0.5 foreshortening, so a member along Z becomes an inclined line
    // instead of a zero-length point. The map is linear, therefore contour
    // segments interpolated along the projected centreline match the projected
    // 3D centreline. Two-dimensional documents keep z = 0 and the plain XY view.
    let depth = if model.dimension == 3 { 0.5 } else { 0.0 };
    let oblique_x = depth * (std::f64::consts::FRAC_PI_6).cos();
    let oblique_y = depth * (std::f64::consts::FRAC_PI_6).sin();
    let project_point = |x: f64, y: f64, z: f64| (x + oblique_x * z, y + oblique_y * z);
    let undeformed: Vec<(f64, f64)> = model.nodes.iter().map(|node| project_point(node.x, node.y, node.z)).collect();
    let deformed: Vec<(f64, f64)> = model
        .nodes
        .iter()
        .zip(&solution.nodes)
        .map(|(node, result)| project_point(node.x + scale * result.ux, node.y + scale * result.uy, node.z + scale * result.uz))
        .collect();
    let series = contour.map(|quantity| contour_series(model, solution, &deformed, quantity, stations)).transpose()?;
    // Plain drawings bend beam members along their Hermite deflection
    // curve; contour drawings keep the sampled straight colour segments.
    let curves = if contour.is_none() { Some(deformed_member_curves(model, solution, scale, project_point)?) } else { None };

    let mut points = Vec::with_capacity(undeformed.len() * 2);
    points.extend(undeformed.iter().copied());
    points.extend(deformed.iter().copied());
    if let Some(curves) = &curves {
        points.extend(curves.iter().flatten().copied());
    }
    // A contour drawing reserves the bottom of the plot area for the colour
    // bar, so its geometry is fitted above that band.
    let projection = if contour.is_some() {
        Projection::fit_in(points, VIEW_LEFT, VIEW_TOP, VIEW_RIGHT, 528.0)?
    }
    else {
        Projection::fit(points)?
    };
    let project = |x: f64, y: f64| projection.project(x, y);

    let scale_label = format!("{scale:.2}");
    let moment_unit = ContourQuantity::Moment.unit(model.units.unwrap_or(Units::Millimeter));
    let heading = if contour.is_some() { "OpenCAE model and contour" } else { "OpenCAE model and deformed shape" };
    let meta = SheetMeta::new(model, heading, "OCAE-DRW-00", format!("×{scale:.2}"));
    let mut text = String::new();
    write_sheet_open(&mut text, &meta);
    text.push_str(
        "<defs>\n<marker id=\"ocae-load-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" \
         markerHeight=\"6\" orient=\"auto\" markerUnits=\"strokeWidth\">\n<path d=\"M0,0 L10,5 L0,10 z\" \
         fill=\"#dc2626\"/>\n</marker>\n</defs>\n",
    );
    text.push_str("<g stroke-linecap=\"round\">\n");
    // Every drawn line is registered with the label placer, so no label can be
    // placed across a member, an outline or an arrow.
    let mut blocked: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let mut symbol_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();

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
    if let Some(series) = &series {
        let limit = series.min.abs().max(series.max.abs());
        for segment in &series.segments {
            let (x1, y1) = project(segment.x1, segment.y1);
            let (x2, y2) = project(segment.x2, segment.y2);
            blocked.push(((x1, y1), (x2, y2)));
            let color = contour_color(segment.value, limit);
            let _ = writeln!(
                text,
                "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"{color}\" stroke-width=\"4\"/>"
            );
        }
    }
    else {
        for curve in curves.as_ref().expect("a plain drawing carries deformed curves") {
            let mut points = String::new();
            let mut projected = Vec::with_capacity(curve.len());
            for &(x, y) in curve {
                let (sx, sy) = project(x, y);
                projected.push((sx, sy));
                let _ = write!(points, "{sx:.2},{sy:.2} ");
            }
            for window in projected.windows(2) {
                blocked.push((window[0], window[1]));
            }
            let _ = writeln!(
                text,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"#2563eb\" stroke-width=\"2\"/>",
                points.trim_end(),
            );
        }
    }
    // The undeformed outline is drawn after the deformed/coloured members as a
    // dashed grey line so it stays visible where the two shapes coincide.
    for &(from, to) in &endpoints {
        let (x1, y1) = project(undeformed[from].0, undeformed[from].1);
        let (x2, y2) = project(undeformed[to].0, undeformed[to].1);
        blocked.push(((x1, y1), (x2, y2)));
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{x2:.2}\" y2=\"{y2:.2}\" stroke=\"#9ca3af\" \
             stroke-width=\"1.2\" stroke-dasharray=\"4 3\"/>"
        );
    }
    for &(x, y) in &undeformed {
        let (cx, cy) = project(x, y);
        symbol_boxes.push((cx - 4.0, cy - 4.0, cx + 4.0, cy + 4.0));
        let _ = writeln!(text, "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"3\" fill=\"#111827\"/>");
    }
    let mut supported_nodes = vec![false; model.nodes.len()];
    for support in &model.supports {
        let constrained = support_is_constrained(support);
        let Some(index) = model.nodes.iter().position(|node| node.id == support.node)
        else {
            continue;
        };
        supported_nodes[index] = constrained;
        if !constrained {
            continue;
        }
        let (cx, cy) = project(undeformed[index].0, undeformed[index].1);
        symbol_boxes.push((cx - 11.0, cy + 4.0, cx + 11.0, cy + 16.0));
        // The triangle apex sits just below the node dot so the symbol never
        // covers the joint itself, and a short ground line closes the base.
        let _ = writeln!(
            text,
            "<polygon points=\"{cx:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"#16a34a\"/>",
            cy + 4.0,
            cx - 8.0,
            cy + 15.0,
            cx + 8.0,
            cy + 15.0,
        );
        let _ = writeln!(
            text,
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"#16a34a\" stroke-width=\"1.5\"/>",
            cx - 11.0,
            cy + 15.0,
            cx + 11.0,
            cy + 15.0,
        );
    }
    let mut load_labels: Vec<(f64, f64, String)> = Vec::new();
    let mut load_above = vec![false; model.nodes.len()];
    for load in &model.loads {
        let magnitude = (load.fx * load.fx + load.fy * load.fy + load.fz * load.fz).sqrt();
        if !magnitude.is_finite() {
            continue;
        }
        let Some(index) = model.nodes.iter().position(|node| node.id == load.node)
        else {
            continue;
        };
        let (cx, cy) = project(undeformed[index].0, undeformed[index].1);
        // The force is a vector, so the same linear map projects its
        // direction; a pure Z load becomes an inclined arrow in 3D.
        let (vector_x, vector_y) = project_point(load.fx, load.fy, load.fz);
        let vector_length = (vector_x * vector_x + vector_y * vector_y).sqrt();
        if vector_length <= 0.0 || !vector_length.is_finite() {
            continue;
        }
        let direction_x = vector_x / vector_length;
        let direction_y = -vector_y / vector_length;
        let arrow_length = 34.0;
        let x1 = cx - direction_x * arrow_length;
        let y1 = cy - direction_y * arrow_length;
        blocked.push(((x1, y1), (cx, cy)));
        let _ = writeln!(
            text,
            "<line x1=\"{x1:.2}\" y1=\"{y1:.2}\" x2=\"{cx:.2}\" y2=\"{cy:.2}\" stroke=\"#dc2626\" \
             stroke-width=\"2\" marker-end=\"url(#ocae-load-arrow)\"/>"
        );
        load_labels.push((x1, y1, format!("{} N", format_contour_value(magnitude))));
        if y1 < cy - 8.0 {
            load_above[index] = true;
        }
        if load.mz.abs() > 0.0 && load.mz.is_finite() {
            let label = format!("M = {} {moment_unit}", format_contour_value(load.mz));
            load_labels.push((cx + 8.0, cy + 20.0, label));
        }
    }
    text.push_str("</g>\n");

    // Every label is drawn after every line, arrow and marker, and avoids the
    // geometry as well as the labels placed before it, so text never overprints
    // a member line or a symbol.
    text.push_str("<g font-size=\"11\">\n");
    let mut placer = LabelPlacer::within(PLOT_LEFT + 2.0, PLOT_RIGHT - 2.0, PLOT_TOP + 2.0, PLOT_BOTTOM - 2.0);
    for (from, to) in &blocked {
        placer.block_line(*from, *to);
    }
    for rect in symbol_boxes {
        placer.block_box(rect);
    }
    let center = centroid(&undeformed.iter().map(|&(x, y)| project(x, y)).collect::<Vec<_>>());
    for (index, node) in model.nodes.iter().enumerate() {
        let point = project(undeformed[index].0, undeformed[index].1);
        let placed = place_node_label(&mut placer, &node.id, point, center, supported_nodes[index], load_above[index]);
        write_leader(&mut text, placed.leader);
        let _ = writeln!(
            text,
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"#374151\">{}</text>",
            placed.x,
            placed.y,
            xml_escape(&node.id)
        );
    }
    let show_axial = contour
        .is_none_or(|quantity| matches!(quantity, ContourQuantity::Axial | ContourQuantity::Shear | ContourQuantity::Moment));
    write_member_annotations(
        &mut text,
        &mut placer,
        model,
        show_axial.then_some(solution),
        &endpoints,
        &undeformed,
        &projection,
    );
    for (x, y, label) in &load_labels {
        let (lx, ly) = placer.place(x + 4.0, y - 6.0, label_width(label));
        let _ = writeln!(text, "<text x=\"{lx:.2}\" y=\"{ly:.2}\" fill=\"#b91c1c\" {LABEL_HALO}>{}</text>", xml_escape(label));
    }
    text.push_str("</g>\n");

    if let (Some(quantity), Some(series)) = (contour, &series) {
        write_contour_legend(&mut text, quantity, series, model.units.unwrap_or(Units::Millimeter));
    }

    // Row legend lists only the items the drawing actually contains.
    let legend_y = LEGEND_BASELINE;
    let legend_text_y = legend_y + 4.0;
    let mut cursor = PLOT_LEFT;
    text.push_str("<g font-size=\"12\" fill=\"#111827\">\n");
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"{cursor}\" y1=\"{legend_y}\" x2=\"{}\" y2=\"{legend_y}\" stroke=\"#9ca3af\" \
         stroke-width=\"1.2\" stroke-dasharray=\"4 3\"/>\n<text x=\"{}\" y=\"{legend_text_y}\">undeformed</text>",
        cursor + 26.0,
        cursor + 34.0,
    );
    cursor += 132.0;
    let deformed_label = if contour.is_some() { "contour" } else { "deformed" };
    let deformed_color = match &series {
        Some(series) => {
            let limit = series.min.abs().max(series.max.abs());
            let extreme = if series.min.abs() >= series.max.abs() { series.min } else { series.max };
            contour_color(extreme, limit)
        }
        None => "#2563eb".to_string(),
    };
    let _ = writeln!(
        text,
        "<line data-legend=\"1\" x1=\"{cursor}\" y1=\"{legend_y}\" x2=\"{}\" y2=\"{legend_y}\" stroke=\"{deformed_color}\" \
         stroke-width=\"2\"/>\n<text x=\"{}\" y=\"{legend_text_y}\">{deformed_label} (scale {scale_label})</text>",
        cursor + 26.0,
        cursor + 34.0,
    );
    cursor += 238.0;
    if supported_nodes.iter().any(|supported| *supported) {
        let _ = writeln!(
            text,
            "<polygon points=\"{},{:.2} {},{:.2} {},{:.2}\" fill=\"#16a34a\"/>\n<line x1=\"{}\" y1=\"{:.2}\" \
             x2=\"{}\" y2=\"{:.2}\" stroke=\"#16a34a\" stroke-width=\"1.5\"/>\n<text x=\"{}\" y=\"{legend_text_y}\">support</text>",
            cursor + 13.0,
            legend_y - 6.0,
            cursor + 5.0,
            legend_y + 8.0,
            cursor + 21.0,
            legend_y + 8.0,
            cursor + 2.0,
            legend_y + 8.0,
            cursor + 24.0,
            legend_y + 8.0,
            cursor + 30.0,
        );
        cursor += 106.0;
    }
    if !load_labels.is_empty() {
        let _ = writeln!(
            text,
            "<line data-legend=\"1\" x1=\"{cursor}\" y1=\"{legend_y}\" x2=\"{}\" y2=\"{legend_y}\" stroke=\"#dc2626\" \
             stroke-width=\"2\" marker-end=\"url(#ocae-load-arrow)\"/>\n<text x=\"{}\" y=\"{legend_text_y}\">load</text>",
            cursor + 26.0,
            cursor + 34.0,
        );
    }
    let projection_label = if model.dimension == 3 { "projection: XY, oblique Z (30°, 0.5)" } else { "projection: XY" };
    let _ =
        writeln!(text, "<text x=\"{PLOT_RIGHT}\" y=\"{legend_text_y}\" text-anchor=\"end\">{projection_label}</text>\n</g>\n");

    let note = if contour.is_some() {
        "coloured members carry the sampled field; values at member midpoints are signed and labelled with their unit."
    }
    else {
        "grey dashed: undeformed members; blue: deformed centreline at the printed scale; loads carry their magnitude."
    };
    let note2 = "node ids, member ids and axial forces are offset clear of every member line; a leader line connects a value that had to move further out.";
    write_sheet_close(&mut text, &meta, &[note, note2]);

    Ok(text)
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

/// Formats a contour extreme with four significant digits.
///
/// The legend labels must stay compact and meaningful: fixed decimal places
/// printed values such as `-1000000.000` or `-19954.390`, where the trailing
/// digits are noise. Very large or very small magnitudes use scientific
/// notation, everything else rounds to four significant digits and trims the
/// trailing zeros.
fn format_contour_value(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let magnitude = value.abs();
    if magnitude >= 1.0e6 || magnitude < 1.0e-3 {
        return format!("{value:.3e}");
    }
    let decimals = (3.0 - magnitude.log10().floor()).clamp(0.0, 6.0) as usize;
    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.truncate(text.trim_end_matches('0').trim_end_matches('.').len());
    }
    text
}

fn write_contour_legend(text: &mut String, quantity: ContourQuantity, series: &ContourSeries, units: Units) {
    // The bar sits in the band reserved below the contour geometry, left of
    // the row legend, so it never collides with the title block.
    const BAR_LEFT: f64 = 560.0;
    const BAR_RIGHT: f64 = 862.0;
    const BAR_TOP: f64 = 546.0;
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
        "<g font-size=\"12\" fill=\"#111827\">\n\
         <text x=\"{BAR_LEFT}\" y=\"538\">contour: {} ({})</text>",
        quantity.label(),
        quantity.unit(units)
    );
    let span = maximum - minimum;
    // A uniform field has no gradient to show. Drawing twelve identical swatches
    // would print the same endpoint twice, so use one solid swatch and a single
    // `uniform <value>` label instead. The tolerance is relative to the largest
    // sampled magnitude, so solver round-off does not fake a gradient.
    if span.abs() <= limit.abs() * 1.0e-9 {
        let color = contour_color(minimum, limit);
        let _ = writeln!(
            text,
            "<rect data-swatch=\"1\" x=\"{BAR_LEFT}\" y=\"{BAR_TOP}\" width=\"120\" height=\"{BAR_HEIGHT}\" \
             fill=\"{color}\"/>\n\
             <text x=\"{}\" y=\"{}\" font-size=\"12\">uniform {}</text>\n</g>",
            BAR_LEFT + 128.0,
            BAR_TOP + BAR_HEIGHT - 2.0,
            format_contour_value(minimum)
        );
        return;
    }
    let width = (BAR_RIGHT - BAR_LEFT) / STEPS as f64;
    // The bar must show the colours the members actually carry across the
    // sampled value range. A fixed `-limit..+limit` bar would label, for
    // example, a red `+707` swatch as `-707` whenever the data is uniform
    // compression, so the gradient is mapped over `minimum..maximum` and
    // coloured with the same normalisation used for the members.
    for step in 0..STEPS {
        let value = minimum + span * step as f64 / (STEPS - 1) as f64;
        let color = contour_color(value, limit);
        let x = BAR_LEFT + step as f64 * width;
        let _ = writeln!(
            text,
            "<rect data-swatch=\"1\" x=\"{x:.2}\" y=\"{BAR_TOP}\" width=\"{width:.2}\" height=\"{BAR_HEIGHT}\" \
             fill=\"{color}\"/>"
        );
    }
    let _ = writeln!(text, "<text x=\"{BAR_LEFT}\" y=\"574\" font-size=\"11\">{}</text>", format_contour_value(minimum));
    if minimum < 0.0 && maximum > 0.0 {
        let zero_x = BAR_LEFT + (0.0 - minimum) / span * (BAR_RIGHT - BAR_LEFT);
        let _ = writeln!(text, "<text x=\"{zero_x:.2}\" y=\"574\" font-size=\"11\" text-anchor=\"middle\">0</text>");
    }
    let _ = writeln!(
        text,
        "<text x=\"{BAR_RIGHT}\" y=\"574\" font-size=\"11\" text-anchor=\"end\">{}</text>\n</g>",
        format_contour_value(maximum)
    );
}

fn missing_svg_node(element: &str, node: &str) -> CaeError {
    CaeError::new("export/invalid-model", format!("SVG export cannot find node `{node}` for element `{element}`"))
}

/// Escapes the five predefined XML entities in text drawn into the SVG.
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
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
        "<tr><td>Element types</td><td>truss {} / beam {} / beam3d {}</td></tr>",
        model.elements.iter().filter(|element| element.kind == ElementKind::Truss).count(),
        model.elements.iter().filter(|element| element.kind == ElementKind::Beam).count(),
        model.elements.iter().filter(|element| element.kind == ElementKind::Beam3D).count()
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

    let spatial_nodes = solution.nodes.iter().any(|node| node.rx.is_some());
    text.push_str("<section>\n<h2>Nodal Displacements</h2>\n<table>\n");
    text.push_str(if spatial_nodes {
        "<tr><th>Node</th><th>ux</th><th>uy</th><th>uz</th><th>rx</th><th>ry</th><th>rz</th><th>Magnitude</th></tr>\n"
    }
    else {
        "<tr><th>Node</th><th>ux</th><th>uy</th><th>uz</th><th>rz</th><th>Magnitude</th></tr>\n"
    });
    for node in &solution.nodes {
        let _ = writeln!(
            text,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td>{}<td>{}</td><td>{}</td></tr>",
            escape_html(&node.id),
            format_float(node.ux),
            format_float(node.uy),
            format_float(node.uz),
            if spatial_nodes {
                format!(
                    "<td>{}</td><td>{}</td>",
                    format_float(node.rx.unwrap_or_default()),
                    format_float(node.ry.unwrap_or_default())
                )
            }
            else {
                String::new()
            },
            format_float(node.rz),
            format_float(node.magnitude)
        );
    }
    text.push_str("</table>\n</section>\n");

    let spatial_reactions = solution.reactions.iter().any(|reaction| reaction.mx.is_some());
    text.push_str("<section>\n<h2>Nodal Reactions</h2>\n<table>\n");
    text.push_str(if spatial_reactions {
        "<tr><th>Node</th><th>fx</th><th>fy</th><th>fz</th><th>mx</th><th>my</th><th>mz</th></tr>\n"
    }
    else {
        "<tr><th>Node</th><th>fx</th><th>fy</th><th>fz</th><th>mz</th></tr>\n"
    });
    for reaction in &solution.reactions {
        let _ = writeln!(
            text,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td>{}<td>{}</td></tr>",
            escape_html(&reaction.node),
            format_float(reaction.fx),
            format_float(reaction.fy),
            format_float(reaction.fz),
            if spatial_reactions {
                format!(
                    "<td>{}</td><td>{}</td>",
                    format_float(reaction.mx.unwrap_or_default()),
                    format_float(reaction.my.unwrap_or_default())
                )
            }
            else {
                String::new()
            },
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
