use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    CaeError, Severity,
    model::{Element, ElementKind, LoadCombination, Model, Units, beam_nodes},
};

/// Version of the JSON result report produced by [`solve`].
pub const REPORT_VERSION: u32 = 1;

/// Format tag of the JSON result report.
pub const REPORT_FORMAT: &str = "open-cae-truss-report";

/// Version of the JSON report produced by [`solve_modal`].
pub const MODAL_REPORT_VERSION: u32 = 1;

/// Format tag of the modal JSON report.
pub const MODAL_REPORT_FORMAT: &str = "open-cae-modal-report";

/// Version of the JSON report produced by [`solve_buckling`].
pub const BUCKLING_REPORT_VERSION: u32 = 1;

/// Format tag of the linear buckling JSON report.
pub const BUCKLING_REPORT_FORMAT: &str = "open-cae-buckling-report";

/// Version of the JSON report produced by [`solve_internal_forces`].
pub const INTERNAL_FORCE_REPORT_VERSION: u32 = 1;

/// Format tag of the internal force diagram JSON report.
pub const INTERNAL_FORCE_REPORT_FORMAT: &str = "open-cae-internal-force-report";

/// Version of the JSON report produced by [`solve_combinations`].
pub const COMBINATION_REPORT_VERSION: u32 = 1;

/// Format tag of the load combination JSON report.
pub const COMBINATION_REPORT_FORMAT: &str = "open-cae-combination-report";

/// Version of the JSON report produced by [`solve_stress`].
pub const STRESS_REPORT_VERSION: u32 = 1;

/// Format tag of the beam stress-recovery JSON report.
pub const STRESS_REPORT_FORMAT: &str = "open-cae-stress-report";

/// Version of the JSON report produced by [`solve_buckling_combinations`].
pub const BUCKLING_COMBINATION_REPORT_VERSION: u32 = 1;

/// Format tag of the buckling-combination JSON report.
pub const BUCKLING_COMBINATION_REPORT_FORMAT: &str = "open-cae-buckling-combination-report";

/// Nodal displacement result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDisplacement {
    /// Node identifier.
    pub id: String,
    /// Translation along X.
    pub ux: f64,
    /// Translation along Y.
    pub uy: f64,
    /// Translation along Z (always zero for two-dimensional models).
    pub uz: f64,
    /// Rotation about Z (only non-zero at beam nodes in two dimensions).
    pub rz: f64,
    /// Translation magnitude.
    pub magnitude: f64,
}

/// Nodal reaction residual `K·u − F`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeReaction {
    /// Node identifier.
    pub node: String,
    /// Reaction along X.
    pub fx: f64,
    /// Reaction along Y.
    pub fy: f64,
    /// Reaction along Z (always zero for two-dimensional models).
    pub fz: f64,
    /// Reaction moment about Z (only non-zero at beam nodes in two dimensions).
    pub mz: f64,
}

/// Local end forces of an element, expressed in the element's local frame.
///
/// The values are the generalized forces applied to the element ends by the
/// nodes, `f = K·d − f_eq`, where `f_eq` is the consistent nodal load vector.
/// The separate `axialForce` field remains positive in tension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndForces {
    /// Local axial end force at the start node.
    pub n1: f64,
    /// Local shear end force at the start node.
    pub v1: f64,
    /// Local bending moment at the start node.
    pub m1: f64,
    /// Local axial end force at the end node.
    pub n2: f64,
    /// Local shear end force at the end node.
    pub v2: f64,
    /// Local bending moment at the end node.
    pub m2: f64,
}

/// Member result. Axial force is positive in tension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementResult {
    /// Element identifier.
    pub id: String,
    /// Start node identifier.
    pub from: String,
    /// End node identifier.
    pub to: String,
    /// Element formulation (`truss` or `beam`).
    pub kind: ElementKind,
    /// Element length.
    pub length: f64,
    /// Axial force (positive in tension).
    pub axial_force: f64,
    /// Axial stress `N/A`.
    pub stress: f64,
    /// Axial strain `σ/E`.
    pub strain: f64,
    /// Local end forces at both ends.
    pub end_forces: EndForces,
    /// Largest absolute bending moment at either end.
    pub max_abs_moment: f64,
}

/// Element carrying the largest absolute axial force.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaxAxialForce {
    /// Element identifier.
    pub element: String,
    /// Signed axial force of that element.
    pub axial_force: f64,
}

/// Element carrying the largest absolute bending moment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaxMoment {
    /// Element identifier.
    pub element: String,
    /// Signed bending moment of that element (the end with the larger magnitude).
    pub moment: f64,
}

/// Reaction forces summarised at one support node.
///
/// Rigidly constrained degrees of freedom report the residual `K·u − F`;
/// elastic supports report the spring force `−k·u`. Components that are
/// neither constrained nor spring-supported are `0`. Positive values act in
/// the positive global direction, so they are the forces the support applies
/// to the structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SupportReaction {
    /// Node identifier.
    pub node: String,
    /// Reaction force along X.
    pub fx: f64,
    /// Reaction force along Y.
    pub fy: f64,
    /// Reaction force along Z (always zero in two dimensions).
    pub fz: f64,
    /// Reaction moment about Z (only populated at beam supports in two dimensions).
    pub mz: f64,
}

/// Translational resultant `(fx, fy, fz)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ForceResultant {
    /// Resultant along X.
    pub fx: f64,
    /// Resultant along Y.
    pub fy: f64,
    /// Resultant along Z (always zero in two dimensions).
    pub fz: f64,
}

/// Support reaction summary and global force equilibrium check.
///
/// `applied` is the sum of every external translational load assembled by the
/// solver, including nodal forces, consistent distributed loads and self-weight.
/// `unbalanced` equals `reaction + applied`; `balanced` is true when every
/// component is within `tolerance`, which scales with the largest resultant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReactionSummary {
    /// One entry per support, in model support order.
    pub supports: Vec<SupportReaction>,
    /// Sum of the support reactions.
    pub reaction: ForceResultant,
    /// Sum of the applied external loads.
    pub applied: ForceResultant,
    /// Residual `reaction + applied`.
    pub unbalanced: ForceResultant,
    /// Absolute tolerance used by the equilibrium check.
    pub tolerance: f64,
    /// `true` when every component of `unbalanced` is within `tolerance`.
    pub balanced: bool,
}

/// Complete linear-static solution and result report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Report format tag, always `open-cae-truss-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Nodal displacements in model node order.
    pub nodes: Vec<NodeDisplacement>,
    /// Reaction residual at every node.
    pub reactions: Vec<NodeReaction>,
    /// Reactions grouped by support and the global force equilibrium check.
    ///
    /// Documents produced by older builds omit this field; the default is an
    /// empty summary with a balanced check.
    #[serde(default)]
    pub reaction_summary: ReactionSummary,
    /// Member results in model element order.
    pub elements: Vec<ElementResult>,
    /// Total strain energy.
    pub strain_energy: f64,
    /// Node with the largest displacement magnitude.
    pub max_displacement: NodeDisplacement,
    /// Element with the largest absolute axial force.
    pub max_axial_force: MaxAxialForce,
    /// Element with the largest absolute bending moment.
    pub max_moment: MaxMoment,
}

/// Nodal value of one mass-normalized mode shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeShape {
    /// Node identifier.
    pub node: String,
    /// Shape value along X.
    pub ux: f64,
    /// Shape value along Y.
    pub uy: f64,
    /// Shape value along Z (zero in two dimensions).
    pub uz: f64,
    /// Shape value of the rotation about Z (zero outside beam nodes).
    pub rz: f64,
}

/// One free vibration mode.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    /// Natural frequency in hertz.
    pub frequency_hz: f64,
    /// Generalized eigenvalue `ω²`.
    pub eigenvalue: f64,
    /// Largest absolute nodal shape value.
    pub max_amplitude: f64,
    /// Effective modal mass `(φᵀ·M·r)²` in the global X, Y and Z directions.
    ///
    /// The shape is mass-normalized, so the participation factor is
    /// `φᵀ·M·r` and its square is the mass excited by this mode. The ratio to
    /// `ModalSolution::total_mass` is the effective mass fraction.
    pub effective_mass: [f64; 3],
    /// Mass-normalized mode shape in model node order.
    pub shape: Vec<ModeShape>,
}

/// Modal solution with the lowest requested modes in ascending frequency order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModalSolution {
    /// Report format tag, always `open-cae-modal-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Requested number of modes.
    pub requested: usize,
    /// Number of eigenvalues skipped because they were zero, negative or
    /// non-finite; skipped values are never emitted.
    pub skipped: usize,
    /// Total translational mass in the global X, Y and Z directions.
    ///
    /// Two-dimensional models report `z = 0`. The sum of
    /// `Mode::effective_mass` divided by these values is the cumulative mass
    /// participation of the returned modes.
    pub total_mass: [f64; 3],
    /// Modes in ascending frequency order.
    pub modes: Vec<Mode>,
}

/// One linear buckling mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucklingMode {
    /// Load factor `λ` that multiplies the static reference axial forces at
    /// the onset of instability. The critical load is `λ` times the static
    /// load case.
    pub factor: f64,
    /// Largest absolute nodal shape value after normalization to one.
    pub max_amplitude: f64,
    /// Mode shape in model node order, normalized so the largest absolute
    /// nodal value is exactly one.
    pub shape: Vec<ModeShape>,
}

/// Linear buckling solution with the lowest requested factors in ascending
/// order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucklingSolution {
    /// Report format tag, always `open-cae-buckling-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Requested number of modes.
    pub requested: usize,
    /// Eigenpairs skipped because they did not yield a finite positive load
    /// factor; skipped values are never emitted.
    pub skipped: usize,
    /// Modes in ascending load-factor order.
    pub modes: Vec<BucklingMode>,
}

/// Internal force result at one station along an element.
///
/// `position` is measured from the `from` node along the element. The axial
/// force is positive in tension, the shear follows `dV/dx = q` with `q` the
/// local `+y` load intensity, and the bending moment satisfies `dM/dx = V`
/// (sagging positive for a horizontally oriented beam).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionForces {
    /// Distance from the `from` node along the element.
    pub position: f64,
    /// Axial force, positive in tension.
    pub axial_force: f64,
    /// Shear force.
    pub shear: f64,
    /// Bending moment.
    pub moment: f64,
}

/// Sampled internal forces of one element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementDiagram {
    /// Element identifier.
    pub id: String,
    /// Start node identifier.
    pub from: String,
    /// End node identifier.
    pub to: String,
    /// Element formulation (`truss` or `beam`).
    pub kind: ElementKind,
    /// Element length.
    pub length: f64,
    /// Largest absolute shear among the sampled stations.
    pub max_abs_shear: f64,
    /// Largest absolute bending moment among the sampled stations.
    pub max_abs_moment: f64,
    /// Sampled stations from the `from` node to the `to` node.
    pub stations: Vec<SectionForces>,
}

/// Internal force diagrams of every element of a solved model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalForceSolution {
    /// Report format tag, always `open-cae-internal-force-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of stations sampled per element, including both ends.
    pub stations: usize,
    /// Diagrams in model element order.
    pub elements: Vec<ElementDiagram>,
}

/// Section stress recovered at one station of an element.
///
/// `minStress` and `maxStress` are the two extreme fibre stresses
/// `N/A ± |M|/S`, where `S = I/c` is the elastic section modulus. Both equal
/// the axial stress for truss elements, which carry no bending.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationStress {
    /// Distance from the `from` node along the element.
    pub position: f64,
    /// Axial force, positive in tension.
    pub axial_force: f64,
    /// Bending moment.
    pub moment: f64,
    /// Axial stress `N/A`.
    pub axial_stress: f64,
    /// Signed bending stress `M/S` at the extreme fibre.
    pub bending_stress: f64,
    /// Smaller extreme-fibre stress.
    pub min_stress: f64,
    /// Larger extreme-fibre stress.
    pub max_stress: f64,
}

/// Recovered stresses of one element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementStress {
    /// Element identifier.
    pub id: String,
    /// Start node identifier.
    pub from: String,
    /// End node identifier.
    pub to: String,
    /// Element formulation (`truss` or `beam`).
    pub kind: ElementKind,
    /// Element length.
    pub length: f64,
    /// Cross-sectional area used by `N/A`.
    pub area: f64,
    /// Elastic section modulus `I/c`; zero for truss elements.
    pub section_modulus: f64,
    /// Largest absolute extreme-fibre stress among the sampled stations.
    pub max_abs_stress: f64,
    /// Index of the governing station in `stations`.
    pub governing_station: usize,
    /// Position of the governing station.
    pub governing_position: f64,
    /// `maxAbsStress / allowableStress` when the material defines one.
    pub utilization: Option<f64>,
    /// Sampled stations from the `from` node to the `to` node.
    pub stations: Vec<StationStress>,
}

/// Governing element and station of a stress-recovery report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StressGoverning {
    /// Element that carries the largest absolute extreme-fibre stress.
    pub element: String,
    /// Position along that element.
    pub position: f64,
    /// Largest absolute extreme-fibre stress.
    pub max_abs_stress: f64,
    /// `maxAbsStress / allowableStress` when the material defines one.
    pub utilization: Option<f64>,
}

/// Result of [`solve_stress`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StressSolution {
    /// Report format tag, always `open-cae-stress-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of stations sampled per element, including both ends.
    pub stations: usize,
    /// Allowable stress used for utilisation checks, when declared.
    pub allowable_stress: Option<f64>,
    /// Governing element and station.
    pub governing: StressGoverning,
    /// Per-element stresses in model element order.
    pub elements: Vec<ElementStress>,
}

/// One solved load combination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinationCase {
    /// Combination name.
    pub name: String,
    /// Factor applied to nodal and element applied loads.
    pub loads: f64,
    /// Factor applied to the gravity vector.
    pub gravity: f64,
    /// Static solution of the scaled model.
    pub solution: Solution,
}

/// Governing value of one envelope entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeValue {
    /// Combination that produced the governing value.
    pub combination: String,
    /// Node or element identifier the value belongs to.
    pub entity: String,
    /// Signed value; the comparison used its absolute magnitude.
    pub value: f64,
}

/// Displacement envelope of one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeEnvelope {
    /// Node identifier.
    pub node: String,
    /// Largest displacement magnitude across all combinations.
    pub max_abs_displacement: f64,
    /// Combination that produced it.
    pub combination: String,
}

/// Force envelope of one element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementEnvelope {
    /// Element identifier.
    pub element: String,
    /// Most compressive signed axial force across all combinations.
    pub min_axial_force: f64,
    /// Most tensile signed axial force across all combinations.
    pub max_axial_force: f64,
    /// Largest absolute bending moment across all combinations.
    pub max_abs_moment: f64,
    /// Combination that produced `max_abs_moment`.
    pub moment_combination: String,
}

/// Envelope of several load combinations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinationEnvelope {
    /// Governing nodal displacement magnitude.
    pub max_displacement: EnvelopeValue,
    /// Governing signed axial force, compared by absolute value.
    pub max_abs_axial_force: EnvelopeValue,
    /// Governing signed bending moment, compared by absolute value.
    pub max_abs_moment: EnvelopeValue,
    /// Per-node displacement envelope.
    pub nodes: Vec<NodeEnvelope>,
    /// Per-element force envelope.
    pub elements: Vec<ElementEnvelope>,
}

/// Result of [`solve_combinations`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinationSolution {
    /// Report format tag, always `open-cae-combination-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Solved combinations in the requested order.
    pub combinations: Vec<CombinationCase>,
    /// Envelope of all solved combinations.
    pub envelope: CombinationEnvelope,
}

/// One solved buckling load combination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucklingCase {
    /// Combination name.
    pub name: String,
    /// Factor applied to nodal and element applied loads.
    pub loads: f64,
    /// Factor applied to the gravity vector.
    pub gravity: f64,
    /// Buckling solution of the scaled model.
    pub solution: BucklingSolution,
}

/// Governing combination of a buckling envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucklingEnvelope {
    /// Combination with the lowest first buckling factor.
    pub combination: String,
    /// First (lowest) buckling load factor of that combination.
    pub factor: f64,
}

/// Result of [`solve_buckling_combinations`].
///
/// Each combination is scaled independently and solved as its own linear
/// buckling case, so factors always multiply that combination's load set rather
/// than the unscaled document loads. `envelope` is the combination whose first
/// factor is lowest; it is `None` when no combination produces a positive
/// factor at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucklingCombinationSolution {
    /// Report format tag, always `open-cae-buckling-combination-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Requested number of modes per combination.
    pub requested: usize,
    /// Solved combinations in the requested order.
    pub combinations: Vec<BucklingCase>,
    /// Governing combination and its first buckling factor.
    pub envelope: Option<BucklingEnvelope>,
}

/// Two-dimensional field layout with explicit degree-of-freedom activation.
///
/// Every node owns `slots` full slots. In two dimensions without beams those
/// are `ux, uy`; in two dimensions with beams they are `ux, uy, rz`, but the
/// rotation slot is active only at nodes touched by a beam. The `index` maps a
/// full slot to its position in the assembled system.
struct Layout {
    slots: usize,
    uses_rotation: bool,
    index: Vec<Option<usize>>,
    size: usize,
}

impl Layout {
    fn dof(&self, slot: usize) -> Result<usize, CaeError> {
        self.index.get(slot).copied().flatten().ok_or_else(|| {
            CaeError::new(
                "element/inactive-dof",
                format!("degree-of-freedom slot {slot} does not exist in the active global system"),
            )
        })
    }
}

struct AssemblyElement {
    id: String,
    from: usize,
    to: usize,
    kind: ElementKind,
    length: f64,
    youngs_modulus: f64,
    area: f64,
    displacement_dofs: Vec<usize>,
    model: ElementModel,
}

enum ElementModel {
    Truss {
        direction: [f64; 3],
        axial_stiffness: f64,
        components: usize,
        /// Equivalent thermal axial force `E·A·alpha·ΔT`.
        thermal: f64,
    },
    Beam(Box<BeamLocal>),
}

struct BeamLocal {
    transform: [[f64; 6]; 6],
    stiffness: [[f64; 6]; 6],
    load: [f64; 6],
    /// Combined user and gravity transverse intensity at the start node.
    transverse_start: f64,
    /// Combined user and gravity transverse intensity at the end node.
    transverse_end: f64,
    /// Uniform distributed axial load per unit length (gravity component).
    axial_uniform: f64,
}

struct System {
    layout: Layout,
    stiffness: Vec<f64>,
    forces: Vec<f64>,
    constrained: Vec<bool>,
    /// Enforced displacement of every constrained degree of freedom; `0` for
    /// ordinary homogeneous supports.
    prescribed: Vec<f64>,
    elements: Vec<AssemblyElement>,
}

/// Solves a two- or three-dimensional model for a single linear-static load
/// case, including beam bending, user distributed loads and self-weight.
///
/// The function first runs [`Model::validate`] and returns its first error
/// unchanged, so callers receive codes such as `model/mechanism` or
/// `material/invalid` instead of a numerical failure.
pub fn solve(model: &Model) -> Result<Solution, CaeError> {
    if let Some(issue) = model.validate().into_iter().find(|issue| issue.severity == Severity::Error) {
        return Err(CaeError::new(issue.code, issue.message));
    }
    let system = assemble(model)?;
    let size = system.layout.size;
    let free: Vec<usize> = (0..size).filter(|&dof| !constrained(&system, dof)).collect();
    let free_count = free.len();
    let (reduced, mut reduced_forces) = reduce(&system, &free);
    // Non-zero prescribed displacements contribute `−K_fc · u_c` to the free
    // right-hand side. With all prescribed values zero this term vanishes and
    // the homogeneous behaviour is unchanged.
    for (row, &row_dof) in free.iter().enumerate() {
        for (column, &value) in system.prescribed.iter().enumerate() {
            if value != 0.0 {
                reduced_forces[row] -= system.stiffness[row_dof * size + column] * value;
            }
        }
    }
    let free_displacements = solve_dense(reduced, reduced_forces, free_count).map_err(|()| {
        CaeError::new(
            "model/mechanism",
            "free stiffness matrix is singular (pivot near zero); the structure is under-constrained or contains a mechanism",
        )
    })?;

    let mut displacement = system.prescribed.clone();
    for (index, &dof) in free.iter().enumerate() {
        displacement[dof] = free_displacements[index];
    }
    if displacement.iter().any(|value| !value.is_finite()) {
        return Err(CaeError::new(
            "model/mechanism",
            "solution contains non-finite values; the structure is numerically singular",
        ));
    }

    let residual = residual(&system, &displacement);
    let layout = &system.layout;
    let nodes: Vec<NodeDisplacement> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let ux = node_value(layout, &displacement, index, 0);
            let uy = node_value(layout, &displacement, index, 1);
            let uz = if model.dimension == 3 { node_value(layout, &displacement, index, 2) } else { 0.0 };
            let rz = if layout.uses_rotation { node_value(layout, &displacement, index, 2) } else { 0.0 };
            NodeDisplacement { id: node.id.clone(), ux, uy, uz, rz, magnitude: (ux * ux + uy * uy + uz * uz).sqrt() }
        })
        .collect();
    let reactions = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let fx = node_value(layout, &residual, index, 0);
            let fy = node_value(layout, &residual, index, 1);
            let fz = if model.dimension == 3 { node_value(layout, &residual, index, 2) } else { 0.0 };
            let mz = if layout.uses_rotation { node_value(layout, &residual, index, 2) } else { 0.0 };
            NodeReaction { node: node.id.clone(), fx, fy, fz, mz }
        })
        .collect();
    let reaction_summary = summarize_reactions(model, &system, &residual, &displacement);

    let mut elements = Vec::with_capacity(system.elements.len());
    let mut strain_energy = 0.0;
    for element in &system.elements {
        let local_displacement: Vec<f64> = element.displacement_dofs.iter().map(|&dof| displacement[dof]).collect();
        let (axial_force, end_forces, energy) = match &element.model {
            ElementModel::Truss { direction, axial_stiffness, components, thermal } => {
                let mut axial = 0.0;
                for component in 0..*components {
                    axial +=
                        (local_displacement[*components + component] - local_displacement[component]) * direction[component];
                }
                // The equivalent nodal thermal load is `E·A·alpha·ΔT·[-1, +1]`,
                // so the physical axial force is the stiffness elongation minus
                // that thermal force and a free member expands without stress.
                let mechanical = axial - thermal / axial_stiffness;
                let force = axial_stiffness * mechanical;
                (
                    force,
                    EndForces { n1: -force, v1: 0.0, m1: 0.0, n2: force, v2: 0.0, m2: 0.0 },
                    0.5 * axial_stiffness * mechanical * mechanical,
                )
            }
            ElementModel::Beam(beam) => {
                let mut local = [0.0; 6];
                for (row, value) in local.iter_mut().enumerate() {
                    *value = (0..6).map(|column| beam.transform[row][column] * local_displacement[column]).sum();
                }
                let mut forces = [0.0; 6];
                for (row, value) in forces.iter_mut().enumerate() {
                    *value = (0..6).map(|column| beam.stiffness[row][column] * local[column]).sum::<f64>() - beam.load[row];
                }
                let axial = 0.5 * (forces[3] - forces[0]);
                let energy = 0.5
                    * (0..6)
                        .map(|row| local[row] * (0..6).map(|column| beam.stiffness[row][column] * local[column]).sum::<f64>())
                        .sum::<f64>();
                (
                    axial,
                    EndForces { n1: forces[0], v1: forces[1], m1: forces[2], n2: forces[3], v2: forces[4], m2: forces[5] },
                    energy,
                )
            }
        };
        let stress = axial_force / element.area;
        let strain = stress / element.youngs_modulus;
        strain_energy += energy;
        let max_abs_moment = end_forces.m1.abs().max(end_forces.m2.abs());
        elements.push(ElementResult {
            id: element.id.clone(),
            from: model.nodes[element.from].id.clone(),
            to: model.nodes[element.to].id.clone(),
            kind: element.kind,
            length: element.length,
            axial_force,
            stress,
            strain,
            end_forces,
            max_abs_moment,
        });
    }

    let max_displacement = nodes
        .iter()
        .max_by(|left, right| left.magnitude.partial_cmp(&right.magnitude).unwrap_or(std::cmp::Ordering::Equal))
        .cloned()
        .ok_or_else(|| CaeError::new("model/no-nodes", "the model requires at least 1 node"))?;
    let max_axial_force = elements
        .iter()
        .max_by(|left, right| left.axial_force.abs().partial_cmp(&right.axial_force.abs()).unwrap_or(std::cmp::Ordering::Equal))
        .map(|element| MaxAxialForce { element: element.id.clone(), axial_force: element.axial_force })
        .ok_or_else(|| CaeError::new("model/no-elements", "the model requires at least 1 element"))?;
    let max_moment = elements
        .iter()
        .max_by(|left, right| left.max_abs_moment.partial_cmp(&right.max_abs_moment).unwrap_or(std::cmp::Ordering::Equal))
        .map(|element| MaxMoment {
            element: element.id.clone(),
            moment: if element.end_forces.m1.abs() >= element.end_forces.m2.abs() {
                element.end_forces.m1
            }
            else {
                element.end_forces.m2
            },
        })
        .ok_or_else(|| CaeError::new("model/no-elements", "the model requires at least 1 element"))?;

    Ok(Solution {
        format: REPORT_FORMAT.to_string(),
        version: REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        nodes,
        reactions,
        reaction_summary,
        elements,
        strain_energy,
        max_displacement,
        max_axial_force,
        max_moment,
    })
}

/// Solves the generalized eigenproblem `K·φ = ω²·M·φ` with a diagonal mass
/// matrix and returns the lowest `modes` frequencies in hertz.
///
/// The material density must be strictly positive. A zero or negative density
/// is reported as `material/invalid-density` instead of being approximated.
pub fn solve_modal(model: &Model, modes: usize) -> Result<ModalSolution, CaeError> {
    if modes == 0 {
        return Err(CaeError::new("analysis/invalid-modes", "the number of modes must be at least 1"));
    }
    if let Some(issue) = model.validate().into_iter().find(|issue| issue.severity == Severity::Error) {
        return Err(CaeError::new(issue.code, issue.message));
    }
    if !positive_finite(model.material.density) {
        return Err(CaeError::new(
            "material/invalid-density",
            format!("modal analysis requires a positive mass density (currently {})", model.material.density),
        ));
    }

    let system = assemble(model)?;
    let size = system.layout.size;
    let free: Vec<usize> = (0..size).filter(|&dof| !constrained(&system, dof)).collect();
    if free.is_empty() {
        return Err(CaeError::new("model/mechanism", "no free degrees of freedom available for modal analysis"));
    }
    let mass = mass_matrix(model, &system.layout)?;
    let translations = if model.dimension == 3 { 3 } else { 2 };
    let mut total_mass = [0.0_f64; 3];
    for node in 0..model.nodes.len() {
        for (component, value) in total_mass.iter_mut().enumerate().take(translations) {
            if let Some(dof) = system.layout.index.get(node * system.layout.slots + component).copied().flatten() {
                *value += mass[dof];
            }
        }
    }
    let count = free.len();
    let mut matrix = vec![0.0; count * count];
    for (row, &row_dof) in free.iter().enumerate() {
        let row_mass = mass[row_dof];
        if !positive_finite(row_mass) {
            return Err(CaeError::new(
                "modal/zero-mass",
                format!("degree of freedom {row_dof} has zero mass; modal analysis is not possible"),
            ));
        }
        for (column, &column_dof) in free.iter().enumerate() {
            let column_mass = mass[column_dof];
            if !positive_finite(column_mass) {
                return Err(CaeError::new(
                    "modal/zero-mass",
                    format!("degree of freedom {column_dof} has zero mass; modal analysis is not possible"),
                ));
            }
            matrix[row * count + column] = system.stiffness[row_dof * size + column_dof] / (row_mass * column_mass).sqrt();
        }
    }

    let eigenvectors = jacobi_eigen(&mut matrix, count);
    let mut eigenvalues: Vec<(usize, f64)> = (0..count).map(|index| (index, matrix[index * count + index])).collect();
    eigenvalues.sort_by(|left, right| left.1.partial_cmp(&right.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut skipped = 0;
    let mut result_modes = Vec::new();
    for (index, eigenvalue) in eigenvalues {
        if !eigenvalue.is_finite() || eigenvalue <= 0.0 {
            skipped += 1;
            continue;
        }
        if result_modes.len() >= modes {
            continue;
        }
        let mut shape_values = vec![0.0; size];
        for (row, &dof) in free.iter().enumerate() {
            shape_values[dof] = eigenvectors[row * count + index] / mass[dof].sqrt();
        }
        let mut effective_mass = [0.0_f64; 3];
        for node in 0..model.nodes.len() {
            for (component, value) in effective_mass.iter_mut().enumerate().take(translations) {
                if let Some(dof) = system.layout.index.get(node * system.layout.slots + component).copied().flatten() {
                    *value += mass[dof] * shape_values[dof];
                }
            }
        }
        for value in &mut effective_mass {
            *value *= *value;
        }
        let mut max_amplitude = 0.0_f64;
        let shape: Vec<ModeShape> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(node, entry)| {
                let ux = node_value(&system.layout, &shape_values, node, 0);
                let uy = node_value(&system.layout, &shape_values, node, 1);
                let uz = if model.dimension == 3 { node_value(&system.layout, &shape_values, node, 2) } else { 0.0 };
                let rz = if system.layout.uses_rotation { node_value(&system.layout, &shape_values, node, 2) } else { 0.0 };
                max_amplitude = max_amplitude.max(ux.abs()).max(uy.abs()).max(uz.abs()).max(rz.abs());
                ModeShape { node: entry.id.clone(), ux, uy, uz, rz }
            })
            .collect();
        result_modes.push(Mode {
            frequency_hz: eigenvalue.sqrt() / (2.0 * std::f64::consts::PI),
            eigenvalue,
            max_amplitude,
            effective_mass,
            shape,
        });
    }

    Ok(ModalSolution {
        format: MODAL_REPORT_FORMAT.to_string(),
        version: MODAL_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        requested: modes,
        skipped,
        total_mass,
        modes: result_modes,
    })
}

/// Solves the linear buckling eigenproblem `(K + λ·Kg)·φ = 0` for the lowest
/// `modes` load factors.
///
/// The geometric stiffness `Kg` is assembled from the axial forces of the
/// static solution of the same model, so each returned factor multiplies that
/// static load case. The eigenproblem is reduced to a symmetric standard form
/// with a Cholesky factor of `K` and solved by the crate's Jacobi iteration.
///
/// Two-dimensional truss and beam models and three-dimensional truss models
/// are supported. Three-dimensional beams are rejected earlier by the static
/// reference solve. Modes are normalized so that the largest absolute nodal
/// value is one.
pub fn solve_buckling(model: &Model, modes: usize) -> Result<BucklingSolution, CaeError> {
    if modes == 0 {
        return Err(CaeError::new("analysis/invalid-modes", "buckling mode count must be at least 1"));
    }
    let reference = solve(model)?;
    let system = assemble(model)?;
    let layout = &system.layout;
    let size = layout.size;
    let free: Vec<usize> = (0..size).filter(|&dof| !constrained(&system, dof)).collect();
    if free.is_empty() {
        return Err(CaeError::new(
            "model/mechanism",
            "no unconstrained degrees of freedom are available for buckling analysis",
        ));
    }

    let mut geometric = vec![0.0; size * size];
    for (element, result) in system.elements.iter().zip(&reference.elements) {
        let axial_force = result.axial_force;
        if !axial_force.is_finite() {
            return Err(CaeError::new(
                "buckling/non-finite",
                format!("the axial force of element `{}` is not finite", element.id),
            ));
        }
        let length = element.length;
        match &element.model {
            ElementModel::Beam(beam) => {
                let mut local = [[0.0_f64; 6]; 6];
                let scale = axial_force / length;
                local[1][1] = 6.0 / 5.0;
                local[1][2] = length / 10.0;
                local[1][4] = -6.0 / 5.0;
                local[1][5] = length / 10.0;
                local[2][1] = length / 10.0;
                local[2][2] = 2.0 * length * length / 15.0;
                local[2][4] = -length / 10.0;
                local[2][5] = -length * length / 30.0;
                local[4][1] = -6.0 / 5.0;
                local[4][2] = -length / 10.0;
                local[4][4] = 6.0 / 5.0;
                local[4][5] = -length / 10.0;
                local[5][1] = length / 10.0;
                local[5][2] = -length * length / 30.0;
                local[5][4] = -length / 10.0;
                local[5][5] = 2.0 * length * length / 15.0;
                let transform = &beam.transform;
                for (row, &row_dof) in element.displacement_dofs.iter().enumerate() {
                    for (column, &column_dof) in element.displacement_dofs.iter().enumerate() {
                        let value: f64 = (0..6)
                            .map(|a| (0..6).map(|b| transform[a][row] * local[a][b] * transform[b][column]).sum::<f64>())
                            .sum();
                        geometric[row_dof * size + column_dof] += scale * value;
                    }
                }
            }
            ElementModel::Truss { direction, components, .. } => {
                // Geometric stiffness of a two-force member is
                // (N/L)·[[P, −P], [−P, P]] with the transverse projector
                // P = I − n·nᵀ. It works unchanged in two and three dimensions.
                let scale = axial_force / length;
                let ends = 2 * components;
                for row in 0..ends {
                    let row_end = row / components;
                    for column in 0..ends {
                        let column_end = column / components;
                        let sign = if row_end == column_end { 1.0 } else { -1.0 };
                        let i = row % components;
                        let j = column % components;
                        let projector = if i == j { 1.0 } else { 0.0 } - direction[i] * direction[j];
                        geometric[element.displacement_dofs[row] * size + element.displacement_dofs[column]] +=
                            scale * sign * projector;
                    }
                }
            }
        }
    }

    let (stiffness_reduced, _) = reduce(&system, &free);
    let count = free.len();
    let mut geometric_reduced = vec![0.0; count * count];
    for (row, &row_dof) in free.iter().enumerate() {
        for (column, &column_dof) in free.iter().enumerate() {
            geometric_reduced[row * count + column] = geometric[row_dof * size + column_dof];
        }
    }

    let factor = cholesky(&stiffness_reduced, count).ok_or_else(|| {
        CaeError::new(
            "model/mechanism",
            "the free stiffness matrix is not positive definite: the structure is under-constrained or contains a mechanism, so buckling analysis cannot proceed",
        )
    })?;

    // A = L⁻¹·Kg·L⁻ᵀ is symmetric; its eigenvalues μ give λ = −1/μ.
    let mut left = vec![0.0; count * count];
    for column in 0..count {
        let right_hand_side: Vec<f64> = (0..count).map(|row| geometric_reduced[row * count + column]).collect();
        let solved = forward_substitution(&factor, &right_hand_side, count);
        for (row, value) in solved.into_iter().enumerate() {
            left[row * count + column] = value;
        }
    }
    let mut standard = vec![0.0; count * count];
    for row in 0..count {
        let right_hand_side: Vec<f64> = (0..count).map(|column| left[row * count + column]).collect();
        let solved = forward_substitution(&factor, &right_hand_side, count);
        for (column, value) in solved.into_iter().enumerate() {
            standard[row * count + column] = value;
        }
    }
    for row in 0..count {
        for column in 0..row {
            let symmetric = 0.5 * (standard[row * count + column] + standard[column * count + row]);
            standard[row * count + column] = symmetric;
            standard[column * count + row] = symmetric;
        }
    }

    let eigenvectors = jacobi_eigen(&mut standard, count);
    let scale = standard.iter().fold(0.0_f64, |largest, value| largest.max(value.abs()));
    let threshold = 1e-12 * scale.max(f64::MIN_POSITIVE);
    let mut candidates: Vec<(f64, usize)> = Vec::new();
    let mut skipped = 0;
    for index in 0..count {
        let eigenvalue = standard[index * count + index];
        if !eigenvalue.is_finite() || eigenvalue >= -threshold {
            skipped += 1;
            continue;
        }
        let load_factor = -1.0 / eigenvalue;
        if !load_factor.is_finite() || load_factor <= 0.0 {
            skipped += 1;
            continue;
        }
        candidates.push((load_factor, index));
    }
    candidates.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut result_modes = Vec::new();
    for (load_factor, index) in candidates.into_iter().take(modes) {
        let shape_right: Vec<f64> = (0..count).map(|row| eigenvectors[row * count + index]).collect();
        let mut shape_values = vec![0.0; size];
        let free_values = back_substitution(&factor, &shape_right, count);
        for (row, &dof) in free.iter().enumerate() {
            shape_values[dof] = free_values[row];
        }
        let mut max_amplitude = 0.0_f64;
        for &dof in &free {
            max_amplitude = max_amplitude.max(shape_values[dof].abs());
        }
        if !positive_finite(max_amplitude) {
            skipped += 1;
            continue;
        }
        let mut sign = 1.0;
        let mut largest = 0.0_f64;
        for &dof in &free {
            if shape_values[dof].abs() > largest {
                largest = shape_values[dof].abs();
                sign = shape_values[dof].signum();
            }
        }
        for &dof in &free {
            shape_values[dof] = shape_values[dof] / max_amplitude * sign;
        }
        let mut shape_max = 0.0_f64;
        let shape: Vec<ModeShape> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(node, entry)| {
                let ux = node_value(layout, &shape_values, node, 0);
                let uy = node_value(layout, &shape_values, node, 1);
                let uz = if model.dimension == 3 { node_value(layout, &shape_values, node, 2) } else { 0.0 };
                let rz = if layout.uses_rotation { node_value(layout, &shape_values, node, 2) } else { 0.0 };
                shape_max = shape_max.max(ux.abs()).max(uy.abs()).max(uz.abs()).max(rz.abs());
                ModeShape { node: entry.id.clone(), ux, uy, uz, rz }
            })
            .collect();
        result_modes.push(BucklingMode { factor: load_factor, max_amplitude: shape_max, shape });
    }

    Ok(BucklingSolution {
        format: BUCKLING_REPORT_FORMAT.to_string(),
        version: BUCKLING_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        requested: modes,
        skipped,
        modes: result_modes,
    })
}

/// Solves the linear buckling eigenproblem for every load combination.
///
/// Each combination scales the applied nodal and element loads by `loads` and
/// the gravity vector by `gravity`, then runs [`solve_buckling`] on the scaled
/// model, so every factor multiplies that combination's load set. Prescribed
/// displacements are boundary conditions and stay unchanged.
///
/// An empty list is rejected with `combination/none`, `modes` must be at least
/// one, and every combination is validated by the static reference solve.
/// `envelope` reports the combination with the lowest first factor; it is
/// `None` when no combination yields a positive factor.
pub fn solve_buckling_combinations(
    model: &Model,
    combinations: &[LoadCombination],
    modes: usize,
) -> Result<BucklingCombinationSolution, CaeError> {
    if modes == 0 {
        return Err(CaeError::new("analysis/invalid-modes", "buckling mode count must be at least 1"));
    }
    if combinations.is_empty() {
        return Err(CaeError::new(
            "combination/none",
            "no load combinations are defined; add at least one combination before solving buckling",
        ));
    }
    let mut cases = Vec::with_capacity(combinations.len());
    for combination in combinations {
        if combination.name.trim().is_empty() {
            return Err(CaeError::new("combination/invalid-name", "a load combination requires a non-empty name"));
        }
        if !combination.loads.is_finite() || !combination.gravity.is_finite() {
            return Err(CaeError::new(
                "combination/invalid-factor",
                format!("load combination `{}` factors must be finite", combination.name),
            ));
        }
        let scaled = scaled_model(model, combination.loads, combination.gravity);
        let solution = solve_buckling(&scaled, modes)?;
        cases.push(BucklingCase {
            name: combination.name.clone(),
            loads: combination.loads,
            gravity: combination.gravity,
            solution,
        });
    }

    let mut envelope: Option<BucklingEnvelope> = None;
    for case in &cases {
        let Some(first) = case.solution.modes.first()
        else {
            continue;
        };
        let lower = envelope.as_ref().is_none_or(|current| first.factor < current.factor);
        if lower {
            envelope = Some(BucklingEnvelope { combination: case.name.clone(), factor: first.factor });
        }
    }

    Ok(BucklingCombinationSolution {
        format: BUCKLING_COMBINATION_REPORT_FORMAT.to_string(),
        version: BUCKLING_COMBINATION_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        requested: modes,
        combinations: cases,
        envelope,
    })
}

/// Solves the static load case and samples the internal axial force, shear and
/// bending moment at `stations` equally spaced points per element.
///
/// The sampling follows section equilibrium using the element end forces and
/// the applied transverse and axial distributed loads, so the first and last
/// stations reproduce the element end forces exactly. `stations` counts both
/// ends and must be at least `2`; a larger value only refines the interior
/// points. Truss elements report a constant axial force and zero shear and
/// moment.
pub fn solve_internal_forces(model: &Model, stations: usize) -> Result<InternalForceSolution, CaeError> {
    if stations < 2 {
        return Err(CaeError::new(
            "analysis/invalid-stations",
            format!("internal force sampling requires at least 2 stations, but {stations} were requested"),
        ));
    }
    let solution = solve(model)?;
    let system = assemble(model)?;
    let mut elements = Vec::with_capacity(system.elements.len());
    for (element, result) in system.elements.iter().zip(&solution.elements) {
        let length = element.length;
        let mut samples = Vec::with_capacity(stations);
        match &element.model {
            ElementModel::Truss { .. } => {
                for index in 0..stations {
                    let position = length * index as f64 / (stations - 1) as f64;
                    samples.push(SectionForces { position, axial_force: result.axial_force, shear: 0.0, moment: 0.0 });
                }
            }
            ElementModel::Beam(beam) => {
                let q1 = beam.transverse_start;
                let slope = (beam.transverse_end - q1) / length;
                let axial_uniform = beam.axial_uniform;
                for index in 0..stations {
                    let position = length * index as f64 / (stations - 1) as f64;
                    let shear = result.end_forces.v1 + q1 * position + 0.5 * slope * position * position;
                    let moment = -result.end_forces.m1
                        + result.end_forces.v1 * position
                        + 0.5 * q1 * position * position
                        + slope * position * position * position / 6.0;
                    samples.push(SectionForces {
                        position,
                        axial_force: -result.end_forces.n1 - axial_uniform * position,
                        shear,
                        moment,
                    });
                }
            }
        }
        let max_abs_shear = samples.iter().fold(0.0_f64, |largest, sample| largest.max(sample.shear.abs()));
        let max_abs_moment = samples.iter().fold(0.0_f64, |largest, sample| largest.max(sample.moment.abs()));
        elements.push(ElementDiagram {
            id: element.id.clone(),
            from: model.nodes[element.from].id.clone(),
            to: model.nodes[element.to].id.clone(),
            kind: element.kind,
            length,
            max_abs_shear,
            max_abs_moment,
            stations: samples,
        });
    }

    let mut output = InternalForceSolution::empty(model, stations);
    output.elements = elements;
    Ok(output)
}

impl InternalForceSolution {
    fn empty(model: &Model, stations: usize) -> Self {
        Self {
            format: INTERNAL_FORCE_REPORT_FORMAT.to_string(),
            version: INTERNAL_FORCE_REPORT_VERSION,
            units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
            stations,
            elements: Vec::new(),
        }
    }
}

/// Recovers extreme-fibre stresses from the sampled internal forces.
///
/// Each station reports `σ = N/A ± |M|/S` with the elastic section modulus
/// `S = I/c`. Beams take `I` from their own `inertia` or from the material
/// section, and `c` always comes from the material section. A model that
/// contains beams without a material section is rejected with
/// `material/missing-section`, because the extreme-fibre distance is unknown.
/// Truss elements report the axial stress only.
///
/// `stations` counts both ends and must be at least `2`. When the material
/// declares `allowableStress`, each element and the governing result also
/// report `maxAbsStress / allowableStress`.
pub fn solve_stress(model: &Model, stations: usize) -> Result<StressSolution, CaeError> {
    if stations < 2 {
        return Err(CaeError::new(
            "analysis/invalid-stations",
            format!("stress recovery requires at least 2 stations, but {stations} were requested"),
        ));
    }
    let solution = solve(model)?;
    let diagram = solve_internal_forces(model, stations)?;
    let properties = match model.material.section {
        Some(section) => Some(section.properties()?),
        None => None,
    };
    let has_beam = model.elements.iter().any(|element| element.kind == ElementKind::Beam);
    if has_beam && properties.is_none() {
        return Err(CaeError::new(
            "material/missing-section",
            "stress recovery for beam elements requires a parametric material section (`material.section`) to provide the extreme-fibre distance",
        ));
    }
    let allowable = model.material.allowable_stress;

    let mut elements = Vec::with_capacity(diagram.elements.len());
    for ((element, result), element_diagram) in model.elements.iter().zip(&solution.elements).zip(&diagram.elements) {
        let area = element.material.as_ref().and_then(|material| material.area).unwrap_or(model.material.area);
        let section_modulus = match element.kind {
            ElementKind::Truss => 0.0,
            ElementKind::Beam => {
                let properties = properties.as_ref().ok_or_else(|| {
                    CaeError::new(
                        "material/missing-section",
                        format!("beam element `{}` needs a parametric material section for stress recovery", element.id),
                    )
                })?;
                let inertia = resolve_inertia(model, element).ok_or_else(|| {
                    CaeError::new(
                        "element/invalid-inertia",
                        format!("beam element `{}` requires a positive finite inertia", element.id),
                    )
                })?;
                inertia / properties.extreme_fiber
            }
        };

        let mut samples = Vec::with_capacity(element_diagram.stations.len());
        let mut governing_station = 0;
        let mut max_abs_stress = 0.0_f64;
        for (index, station) in element_diagram.stations.iter().enumerate() {
            let axial_stress = station.axial_force / area;
            let bending_stress = if section_modulus > 0.0 { station.moment / section_modulus } else { 0.0 };
            let half = bending_stress.abs();
            let min_stress = axial_stress - half;
            let max_stress = axial_stress + half;
            let absolute = min_stress.abs().max(max_stress.abs());
            if absolute > max_abs_stress {
                max_abs_stress = absolute;
                governing_station = index;
            }
            samples.push(StationStress {
                position: station.position,
                axial_force: station.axial_force,
                moment: station.moment,
                axial_stress,
                bending_stress,
                min_stress,
                max_stress,
            });
        }
        let governing_position = samples.get(governing_station).map(|station| station.position).unwrap_or(0.0);
        elements.push(ElementStress {
            id: result.id.clone(),
            from: result.from.clone(),
            to: result.to.clone(),
            kind: element.kind,
            length: element_diagram.length,
            area,
            section_modulus,
            max_abs_stress,
            governing_station,
            governing_position,
            utilization: allowable.map(|allowable| max_abs_stress / allowable),
            stations: samples,
        });
    }

    let governing_element = elements
        .iter()
        .max_by(|left, right| left.max_abs_stress.partial_cmp(&right.max_abs_stress).unwrap_or(std::cmp::Ordering::Equal))
        .ok_or_else(|| CaeError::new("model/no-elements", "the model requires at least 1 element"))?;
    let governing = StressGoverning {
        element: governing_element.id.clone(),
        position: governing_element.governing_position,
        max_abs_stress: governing_element.max_abs_stress,
        utilization: governing_element.utilization,
    };

    Ok(StressSolution {
        format: STRESS_REPORT_FORMAT.to_string(),
        version: STRESS_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        stations,
        allowable_stress: allowable,
        governing,
        elements,
    })
}

fn resolve_inertia(model: &Model, element: &Element) -> Option<f64> {
    element
        .inertia
        .or_else(|| {
            model.material.section.and_then(|section| section.properties().ok()).map(|properties| properties.inertia_strong)
        })
        .filter(|value| positive_finite(*value))
}

/// Solves every provided load combination and returns each solution plus an
/// envelope of the governing values.
///
/// Each combination scales the applied nodal and element loads by `loads` and
/// the gravity vector by `gravity`; prescribed displacements are boundary
/// conditions and stay unchanged. An empty list is rejected with
/// `combination/none`. Every combination is validated by the static solver, so
/// invalid factors, mechanisms or a missing density surface with their usual
/// codes.
pub fn solve_combinations(model: &Model, combinations: &[LoadCombination]) -> Result<CombinationSolution, CaeError> {
    if combinations.is_empty() {
        return Err(CaeError::new(
            "combination/none",
            "no load combinations are defined; add at least one combination before solving",
        ));
    }
    let mut cases = Vec::with_capacity(combinations.len());
    for combination in combinations {
        if combination.name.trim().is_empty() {
            return Err(CaeError::new("combination/invalid-name", "a load combination requires a non-empty name"));
        }
        if !combination.loads.is_finite() || !combination.gravity.is_finite() {
            return Err(CaeError::new(
                "combination/invalid-factor",
                format!("load combination `{}` factors must be finite", combination.name),
            ));
        }
        let scaled = scaled_model(model, combination.loads, combination.gravity);
        let solution = solve(&scaled)?;
        cases.push(CombinationCase {
            name: combination.name.clone(),
            loads: combination.loads,
            gravity: combination.gravity,
            solution,
        });
    }

    let envelope = build_envelope(&cases);
    Ok(CombinationSolution {
        format: COMBINATION_REPORT_FORMAT.to_string(),
        version: COMBINATION_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        combinations: cases,
        envelope,
    })
}

fn scaled_model(model: &Model, loads: f64, gravity: f64) -> Model {
    let mut scaled = model.clone();
    for load in &mut scaled.loads {
        load.fx *= loads;
        load.fy *= loads;
        load.fz *= loads;
        load.mz *= loads;
    }
    for element in &mut scaled.elements {
        if let Some(value) = &mut element.distributed_load {
            *value *= loads;
        }
        if let Some(value) = &mut element.distributed_load_end {
            *value *= loads;
        }
    }
    if let Some(vector) = &mut scaled.gravity {
        for component in vector.iter_mut() {
            *component *= gravity;
        }
        if vector.iter().all(|component| *component == 0.0) {
            scaled.gravity = None;
        }
    }
    scaled
}

fn build_envelope(cases: &[CombinationCase]) -> CombinationEnvelope {
    let first = &cases[0].solution;
    let mut node_max = vec![0.0_f64; first.nodes.len()];
    let mut node_combination = vec![String::new(); first.nodes.len()];
    let mut maximum_displacement = EnvelopeValue { combination: String::new(), entity: String::new(), value: 0.0 };
    let mut maximum_axial = EnvelopeValue { combination: String::new(), entity: String::new(), value: 0.0 };
    let mut maximum_moment = EnvelopeValue { combination: String::new(), entity: String::new(), value: 0.0 };

    let mut min_axial = vec![f64::INFINITY; first.elements.len()];
    let mut max_axial = vec![f64::NEG_INFINITY; first.elements.len()];
    let mut max_abs_moment = vec![0.0_f64; first.elements.len()];
    let mut moment_combination = vec![String::new(); first.elements.len()];

    for case in cases {
        for (index, node) in case.solution.nodes.iter().enumerate() {
            if node.magnitude > node_max[index] {
                node_max[index] = node.magnitude;
                node_combination[index] = case.name.clone();
            }
        }
        let displacement = &case.solution.max_displacement;
        if displacement.magnitude > maximum_displacement.value {
            maximum_displacement = EnvelopeValue {
                combination: case.name.clone(),
                entity: displacement.id.clone(),
                value: displacement.magnitude,
            };
        }
        let axial = &case.solution.max_axial_force;
        if axial.axial_force.abs() > maximum_axial.value.abs() {
            maximum_axial =
                EnvelopeValue { combination: case.name.clone(), entity: axial.element.clone(), value: axial.axial_force };
        }
        let moment = &case.solution.max_moment;
        if moment.moment.abs() > maximum_moment.value.abs() {
            maximum_moment =
                EnvelopeValue { combination: case.name.clone(), entity: moment.element.clone(), value: moment.moment };
        }
        for (index, element) in case.solution.elements.iter().enumerate() {
            min_axial[index] = min_axial[index].min(element.axial_force);
            max_axial[index] = max_axial[index].max(element.axial_force);
            if element.max_abs_moment > max_abs_moment[index] {
                max_abs_moment[index] = element.max_abs_moment;
                moment_combination[index] = case.name.clone();
            }
        }
    }

    let nodes = first
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| NodeEnvelope {
            node: node.id.clone(),
            max_abs_displacement: node_max[index],
            combination: node_combination[index].clone(),
        })
        .collect();
    let elements = first
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| ElementEnvelope {
            element: element.id.clone(),
            min_axial_force: min_axial[index],
            max_axial_force: max_axial[index],
            max_abs_moment: max_abs_moment[index],
            moment_combination: moment_combination[index].clone(),
        })
        .collect();

    CombinationEnvelope {
        max_displacement: maximum_displacement,
        max_abs_axial_force: maximum_axial,
        max_abs_moment: maximum_moment,
        nodes,
        elements,
    }
}

/// Returns ids of nodes that have no support constraining any active degree of
/// freedom.
pub(crate) fn unconstrained_nodes(model: &Model) -> Vec<String> {
    model
        .nodes
        .iter()
        .filter(|node| {
            !model
                .supports
                .iter()
                .any(|support| support.node == node.id && (support.dx || support.dy || support.dz || support.rz))
        })
        .map(|node| node.id.clone())
        .collect()
}

/// Checks that the stiffness matrix on the free degrees of freedom has full
/// numerical rank. Returns the unconstrained nodes when it does not.
pub(crate) fn check_stability(model: &Model) -> Result<(), Vec<String>> {
    let system = assemble(model).map_err(|_| unconstrained_nodes(model))?;
    let free: Vec<usize> = (0..system.layout.size).filter(|&dof| !constrained(&system, dof)).collect();
    if free.is_empty() {
        return Ok(());
    }
    let (reduced, forces) = reduce(&system, &free);
    solve_dense(reduced, forces, free.len()).map(|_| ()).map_err(|()| unconstrained_nodes(model))
}

fn build_layout(model: &Model) -> Layout {
    let three_dimensional = model.dimension == 3;
    let beams = beam_nodes(model);
    let uses_rotation = !three_dimensional && !beams.is_empty();
    let slots = if three_dimensional || uses_rotation { 3 } else { 2 };
    let full_size = model.nodes.len() * slots;
    let mut active = vec![false; full_size];
    for (position, node) in model.nodes.iter().enumerate() {
        let base = position * slots;
        active[base] = true;
        active[base + 1] = true;
        if three_dimensional || (uses_rotation && beams.contains(node.id.as_str())) {
            active[base + 2] = true;
        }
    }
    let mut index = vec![None; full_size];
    let mut size = 0;
    for (slot, &enabled) in active.iter().enumerate() {
        if enabled {
            index[slot] = Some(size);
            size += 1;
        }
    }
    Layout { slots, uses_rotation, index, size }
}

fn constrained(system: &System, dof: usize) -> bool {
    system.constrained[dof]
}

fn assemble(model: &Model) -> Result<System, CaeError> {
    let layout = build_layout(model);
    let size = layout.size;
    let index: HashMap<&str, usize> =
        model.nodes.iter().enumerate().map(|(position, node)| (node.id.as_str(), position)).collect();

    let mut stiffness = vec![0.0; size * size];
    let mut forces = vec![0.0; size];
    let mut elements = Vec::with_capacity(model.elements.len());

    let density = model.material.density;
    let gravity = model.gravity;
    if gravity.is_some() && !positive_finite(density) {
        return Err(CaeError::new(
            "material/invalid-density",
            format!("gravity requires a positive mass density (currently {density})"),
        ));
    }
    let translation_components = if model.dimension == 3 { 3 } else { 2 };

    for element in &model.elements {
        let from = *index.get(element.from.as_str()).ok_or_else(|| {
            CaeError::new(
                "element/unknown-node",
                format!("element `{}` references unknown node `{}`", element.id, element.from),
            )
        })?;
        let to = *index.get(element.to.as_str()).ok_or_else(|| {
            CaeError::new("element/unknown-node", format!("element `{}` references unknown node `{}`", element.id, element.to))
        })?;
        let delta = [
            model.nodes[to].x - model.nodes[from].x,
            model.nodes[to].y - model.nodes[from].y,
            model.nodes[to].z - model.nodes[from].z,
        ];
        let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        if !positive_finite(length) {
            return Err(CaeError::new("element/zero-length", format!("element `{}` has zero length", element.id)));
        }
        let (youngs_modulus, area) = resolve_material(model, element)?;
        let thermal = match element.temperature {
            Some(delta) => {
                let alpha = model.material.alpha.filter(|value| value.is_finite()).ok_or_else(|| {
                    CaeError::new(
                        "material/missing-alpha",
                        format!(
                            "element `{}` declares a temperature change but the material does not define a finite coefficient of thermal expansion (alpha)",
                            element.id
                        ),
                    )
                })?;
                youngs_modulus * area * alpha * delta
            }
            None => 0.0,
        };

        match element.kind {
            ElementKind::Beam => {
                if model.dimension != 2 {
                    return Err(CaeError::new(
                        "capability/not-implemented",
                        format!(
                            "3D beam elements (element `{}`) are not implemented; beam supports dimension=2 only",
                            element.id
                        ),
                    ));
                }
                let inertia = element
                    .inertia
                    .or_else(|| {
                        model
                            .material
                            .section
                            .and_then(|section| section.properties().ok())
                            .map(|properties| properties.inertia_strong)
                    })
                    .filter(|value| positive_finite(*value))
                    .ok_or_else(|| {
                        CaeError::new(
                            "element/invalid-inertia",
                            format!(
                                "beam element `{}` requires a positive finite inertia, or a material section that provides one",
                                element.id
                            ),
                        )
                    })?;
                let c = delta[0] / length;
                let s = delta[1] / length;
                let mut local_load = [0.0; 6];
                if element.distributed_load.is_some_and(|value| !value.is_finite())
                    || element.distributed_load_end.is_some_and(|value| !value.is_finite())
                {
                    return Err(CaeError::new(
                        "element/invalid-distributed-load",
                        format!("distributed load w of element `{}` must be finite", element.id),
                    ));
                }
                let mut load_start = element.distributed_load.unwrap_or(0.0);
                let mut load_end = element.distributed_load_end.unwrap_or(load_start);
                let mut axial_uniform = 0.0;
                if let Some(gravity) = gravity {
                    let mass_per_length = density * area;
                    let transverse = mass_per_length * (-gravity[0] * s + gravity[1] * c);
                    load_start += transverse;
                    load_end += transverse;
                    axial_uniform = mass_per_length * (gravity[0] * c + gravity[1] * s);
                    let axial = axial_uniform * length;
                    local_load[0] += 0.5 * axial;
                    local_load[3] += 0.5 * axial;
                }
                if load_start != 0.0 || load_end != 0.0 {
                    add_linear_transverse(&mut local_load, load_start, load_end, length);
                }
                // Uniform thermal expansion enters as the consistent axial
                // load `E·A·alpha·ΔT·[-1, +1]` in local coordinates.
                if thermal != 0.0 {
                    local_load[0] -= thermal;
                    local_load[3] += thermal;
                }
                let local_stiffness = beam_stiffness(youngs_modulus, area, inertia, length);
                let transform = beam_transform(c, s);
                let displacement_dofs = vec![
                    layout.dof(from * layout.slots)?,
                    layout.dof(from * layout.slots + 1)?,
                    layout.dof(from * layout.slots + 2)?,
                    layout.dof(to * layout.slots)?,
                    layout.dof(to * layout.slots + 1)?,
                    layout.dof(to * layout.slots + 2)?,
                ];
                let mut global_stiffness = [[0.0_f64; 6]; 6];
                for (row, row_values) in global_stiffness.iter_mut().enumerate() {
                    for (column, value) in row_values.iter_mut().enumerate() {
                        *value = (0..6)
                            .map(|a| {
                                (0..6).map(|b| transform[a][row] * local_stiffness[a][b] * transform[b][column]).sum::<f64>()
                            })
                            .sum();
                    }
                }
                for (row, &row_dof) in displacement_dofs.iter().enumerate() {
                    for (column, &column_dof) in displacement_dofs.iter().enumerate() {
                        stiffness[row_dof * size + column_dof] += global_stiffness[row][column];
                    }
                    let load: f64 = (0..6).map(|a| transform[a][row] * local_load[a]).sum();
                    forces[row_dof] += load;
                }
                elements.push(AssemblyElement {
                    id: element.id.clone(),
                    from,
                    to,
                    kind: element.kind,
                    length,
                    youngs_modulus,
                    area,
                    displacement_dofs,
                    model: ElementModel::Beam(Box::new(BeamLocal {
                        transform,
                        stiffness: local_stiffness,
                        load: local_load,
                        transverse_start: load_start,
                        transverse_end: load_end,
                        axial_uniform,
                    })),
                });
            }
            ElementKind::Truss => {
                if element.distributed_load.is_some() || element.distributed_load_end.is_some() {
                    return Err(CaeError::new(
                        "element/distributed-load-requires-beam",
                        format!(
                            "element `{}` is a truss element; distributed load w can only be applied to beam elements",
                            element.id
                        ),
                    ));
                }
                let direction =
                    [delta[0] / length, delta[1] / length, if model.dimension == 3 { delta[2] / length } else { 0.0 }];
                let axial_stiffness = youngs_modulus * area / length;
                let mut displacement_dofs = Vec::with_capacity(2 * translation_components);
                for &node in &[from, to] {
                    for component in 0..translation_components {
                        displacement_dofs.push(layout.dof(node * layout.slots + component)?);
                    }
                }
                for row in 0..2 * translation_components {
                    let row_end = row / translation_components;
                    for column in 0..2 * translation_components {
                        let column_end = column / translation_components;
                        let sign = if row_end == column_end { 1.0 } else { -1.0 };
                        let value = sign
                            * direction[row % translation_components]
                            * direction[column % translation_components]
                            * axial_stiffness;
                        stiffness[displacement_dofs[row] * size + displacement_dofs[column]] += value;
                    }
                }
                if let Some(gravity) = gravity {
                    let total = [
                        density * area * length * gravity[0],
                        density * area * length * gravity[1],
                        density * area * length * gravity[2],
                    ];
                    for &node in &[from, to] {
                        for component in 0..translation_components {
                            forces[layout.dof(node * layout.slots + component)?] += 0.5 * total[component];
                        }
                    }
                }
                if thermal != 0.0 {
                    for component in 0..translation_components {
                        let value = thermal * direction[component];
                        forces[layout.dof(from * layout.slots + component)?] -= value;
                        forces[layout.dof(to * layout.slots + component)?] += value;
                    }
                }
                elements.push(AssemblyElement {
                    id: element.id.clone(),
                    from,
                    to,
                    kind: element.kind,
                    length,
                    youngs_modulus,
                    area,
                    displacement_dofs,
                    model: ElementModel::Truss { direction, axial_stiffness, components: translation_components, thermal },
                });
            }
        }
    }

    for load in &model.loads {
        if let Some(&node) = index.get(load.node.as_str()) {
            let base = node * layout.slots;
            forces[layout.dof(base)?] += load.fx;
            forces[layout.dof(base + 1)?] += load.fy;
            if model.dimension == 3 {
                forces[layout.dof(base + 2)?] += load.fz;
            }
            if layout.uses_rotation {
                forces[layout.dof(base + 2)?] += load.mz;
            }
        }
    }

    let mut constrained = vec![false; size];
    let mut prescribed = vec![0.0; size];
    for support in &model.supports {
        let Some(&node) = index.get(support.node.as_str())
        else {
            continue;
        };
        let base = node * layout.slots;
        let mut mark = |slot: usize| {
            if let Some(dof) = layout.index.get(slot).copied().flatten() {
                constrained[dof] = true;
            }
        };
        if support.dx {
            mark(base);
        }
        if support.dy {
            mark(base + 1);
        }
        if model.dimension == 3 && support.dz {
            mark(base + 2);
        }
        if layout.uses_rotation && support.rz {
            mark(base + 2);
        }
        if let Some(values) = &support.prescribed {
            let mut assign = |slot: usize, value: f64| {
                if let Some(dof) = layout.index.get(slot).copied().flatten() {
                    prescribed[dof] = value;
                }
            };
            if support.dx {
                assign(base, values.ux);
            }
            if support.dy {
                assign(base + 1, values.uy);
            }
            if model.dimension == 3 && support.dz {
                assign(base + 2, values.uz);
            }
            if layout.uses_rotation && support.rz {
                assign(base + 2, values.rz);
            }
        }
    }

    for support in &model.supports {
        let Some(&node) = index.get(support.node.as_str())
        else {
            continue;
        };
        let Some(spring) = support.spring
        else {
            continue;
        };
        let base = node * layout.slots;
        let mut add = |slot: usize, stiffness_value: f64| {
            if stiffness_value > 0.0 {
                if let Some(dof) = layout.index.get(slot).copied().flatten() {
                    stiffness[dof * size + dof] += stiffness_value;
                }
            }
        };
        add(base, spring.kx);
        add(base + 1, spring.ky);
        if model.dimension == 3 {
            add(base + 2, spring.kz);
        }
        else if layout.uses_rotation {
            add(base + 2, spring.kr);
        }
    }

    Ok(System { layout, stiffness, forces, constrained, prescribed, elements })
}

fn reduce(system: &System, free: &[usize]) -> (Vec<f64>, Vec<f64>) {
    let count = free.len();
    let mut matrix = vec![0.0; count * count];
    let mut forces = vec![0.0; count];
    for (row, &row_dof) in free.iter().enumerate() {
        forces[row] = system.forces[row_dof];
        for (column, &column_dof) in free.iter().enumerate() {
            matrix[row * count + column] = system.stiffness[row_dof * system.layout.size + column_dof];
        }
    }
    (matrix, forces)
}

fn node_value(layout: &Layout, values: &[f64], node: usize, component: usize) -> f64 {
    layout.index.get(node * layout.slots + component).copied().flatten().map(|dof| values[dof]).unwrap_or(0.0)
}

fn resolve_material(model: &Model, element: &Element) -> Result<(f64, f64), CaeError> {
    let youngs_modulus =
        element.material.as_ref().and_then(|material| material.youngs_modulus).unwrap_or(model.material.youngs_modulus);
    let area = element.material.as_ref().and_then(|material| material.area).unwrap_or(model.material.area);
    if !positive_finite(youngs_modulus) {
        return Err(CaeError::new(
            "material/invalid",
            format!("E of element `{}` must be a positive finite value", element.id),
        ));
    }
    if !positive_finite(area) {
        return Err(CaeError::new(
            "material/invalid",
            format!("A of element `{}` must be a positive finite value", element.id),
        ));
    }
    Ok((youngs_modulus, area))
}

fn beam_stiffness(youngs_modulus: f64, area: f64, inertia: f64, length: f64) -> [[f64; 6]; 6] {
    let axial = youngs_modulus * area / length;
    let bending = youngs_modulus * inertia;
    let length2 = length * length;
    let length3 = length2 * length;
    let mut matrix = [[0.0_f64; 6]; 6];
    matrix[0][0] = axial;
    matrix[0][3] = -axial;
    matrix[3][0] = -axial;
    matrix[3][3] = axial;
    let twelve = 12.0 * bending / length3;
    let six = 6.0 * bending / length2;
    let four = 4.0 * bending / length;
    let two = 2.0 * bending / length;
    matrix[1][1] = twelve;
    matrix[1][2] = six;
    matrix[1][4] = -twelve;
    matrix[1][5] = six;
    matrix[2][1] = six;
    matrix[2][2] = four;
    matrix[2][4] = -six;
    matrix[2][5] = two;
    matrix[4][1] = -twelve;
    matrix[4][2] = -six;
    matrix[4][4] = twelve;
    matrix[4][5] = -six;
    matrix[5][1] = six;
    matrix[5][2] = two;
    matrix[5][4] = -six;
    matrix[5][5] = four;
    matrix
}

fn beam_transform(c: f64, s: f64) -> [[f64; 6]; 6] {
    let mut transform = [[0.0_f64; 6]; 6];
    transform[0][0] = c;
    transform[0][1] = s;
    transform[1][0] = -s;
    transform[1][1] = c;
    transform[2][2] = 1.0;
    transform[3][3] = c;
    transform[3][4] = s;
    transform[4][3] = -s;
    transform[4][4] = c;
    transform[5][5] = 1.0;
    transform
}

/// Adds the consistent nodal vector of a linearly varying transverse load
/// `q(x) = q1 + (q2 − q1)·x/L` in the local `+y` direction.
///
/// The entries are the exact integrals of the Euler-Bernoulli shape functions
/// against the linear load; `q1 = q2` reproduces the uniform `[wL/2, wL²/12,
/// wL/2, −wL²/12]` vector.
fn add_linear_transverse(load: &mut [f64; 6], q1: f64, q2: f64, length: f64) {
    let length2 = length * length;
    load[1] += length * (7.0 * q1 + 3.0 * q2) / 20.0;
    load[2] += length2 * (3.0 * q1 + 2.0 * q2) / 60.0;
    load[4] += length * (3.0 * q1 + 7.0 * q2) / 20.0;
    load[5] -= length2 * (2.0 * q1 + 3.0 * q2) / 60.0;
}

fn mass_matrix(model: &Model, layout: &Layout) -> Result<Vec<f64>, CaeError> {
    let mut mass = vec![0.0; layout.size];
    let density = model.material.density;
    let translations = if model.dimension == 3 { 3 } else { 2 };
    for element in &model.elements {
        let from = model.nodes.iter().position(|node| node.id == element.from).ok_or_else(|| {
            CaeError::new("element/unknown-node", format!("element `{}` references an unknown node", element.id))
        })?;
        let to = model.nodes.iter().position(|node| node.id == element.to).ok_or_else(|| {
            CaeError::new("element/unknown-node", format!("element `{}` references an unknown node", element.id))
        })?;
        let (_youngs_modulus, area) = resolve_material(model, element)?;
        let delta = [
            model.nodes[to].x - model.nodes[from].x,
            model.nodes[to].y - model.nodes[from].y,
            model.nodes[to].z - model.nodes[from].z,
        ];
        let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        if !positive_finite(length) {
            return Err(CaeError::new("element/zero-length", format!("element `{}` has zero length", element.id)));
        }
        let translational = 0.5 * density * area * length;
        for &node in &[from, to] {
            for component in 0..translations {
                if let Some(dof) = layout.index.get(node * layout.slots + component).copied().flatten() {
                    mass[dof] += translational;
                }
            }
        }
        if element.kind == ElementKind::Beam {
            let inertia = element.inertia.filter(|value| positive_finite(*value)).ok_or_else(|| {
                CaeError::new(
                    "element/invalid-inertia",
                    format!("beam element `{}` requires a positive finite inertia", element.id),
                )
            })?;
            let rotary = 0.5 * density * inertia * length;
            for &node in &[from, to] {
                if let Some(dof) = layout.index.get(node * layout.slots + 2).copied().flatten() {
                    mass[dof] += rotary;
                }
            }
        }
    }
    Ok(mass)
}

/// Jacobi eigenvalue iteration for a real symmetric matrix.
///
/// Returns eigenvectors as columns. The iteration stops when the largest
/// off-diagonal entry is below `1e-12` times the largest absolute entry, or
/// after 100 sweeps.
fn jacobi_eigen(matrix: &mut [f64], size: usize) -> Vec<f64> {
    let mut vectors = vec![0.0; size * size];
    for index in 0..size {
        vectors[index * size + index] = 1.0;
    }
    if size == 0 {
        return vectors;
    }
    let scale = matrix.iter().fold(0.0_f64, |largest, value| largest.max(value.abs()));
    if !positive_finite(scale) {
        return vectors;
    }
    let tolerance = 1e-12 * scale;

    for _sweep in 0..100 {
        let mut off_diagonal = 0.0_f64;
        for row in 0..size {
            for column in (row + 1)..size {
                off_diagonal = off_diagonal.max(matrix[row * size + column].abs());
            }
        }
        if off_diagonal <= tolerance {
            break;
        }
        for p in 0..size {
            for q in (p + 1)..size {
                let off = matrix[p * size + q];
                if off.abs() <= tolerance {
                    continue;
                }
                let diagonal_p = matrix[p * size + p];
                let diagonal_q = matrix[q * size + q];
                let theta = (diagonal_q - diagonal_p) / (2.0 * off);
                let tangent = if theta == 0.0 { 1.0 } else { theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt()) };
                let cosine = 1.0 / (tangent * tangent + 1.0).sqrt();
                let sine = tangent * cosine;
                for k in 0..size {
                    if k != p && k != q {
                        let left = matrix[k * size + p];
                        let right = matrix[k * size + q];
                        matrix[k * size + p] = cosine * left - sine * right;
                        matrix[k * size + q] = sine * left + cosine * right;
                    }
                }
                for k in 0..size {
                    if k != p && k != q {
                        let left = matrix[p * size + k];
                        let right = matrix[q * size + k];
                        matrix[p * size + k] = cosine * left - sine * right;
                        matrix[q * size + k] = sine * left + cosine * right;
                    }
                }
                matrix[p * size + p] = cosine * cosine * diagonal_p - 2.0 * sine * cosine * off + sine * sine * diagonal_q;
                matrix[q * size + q] = sine * sine * diagonal_p + 2.0 * sine * cosine * off + cosine * cosine * diagonal_q;
                matrix[p * size + q] = 0.0;
                matrix[q * size + p] = 0.0;
                for k in 0..size {
                    let left = vectors[k * size + p];
                    let right = vectors[k * size + q];
                    vectors[k * size + p] = cosine * left - sine * right;
                    vectors[k * size + q] = sine * left + cosine * right;
                }
            }
        }
    }
    vectors
}

/// Dense Gaussian elimination with partial pivoting.
///
/// Returns `Err(())` when the matrix is numerically singular, defined as a
/// pivot smaller than `1e-12` times the largest absolute matrix entry.
fn solve_dense(mut matrix: Vec<f64>, mut rhs: Vec<f64>, size: usize) -> Result<Vec<f64>, ()> {
    if size == 0 {
        return Ok(Vec::new());
    }
    let scale = matrix.iter().fold(0.0_f64, |largest, value| largest.max(value.abs()));
    if !scale.is_finite() || scale <= 0.0 {
        return Err(());
    }
    let tolerance = 1e-12 * scale;

    for column in 0..size {
        let mut pivot = column;
        let mut pivot_value = matrix[column * size + column].abs();
        for row in (column + 1)..size {
            let candidate = matrix[row * size + column].abs();
            if candidate > pivot_value {
                pivot = row;
                pivot_value = candidate;
            }
        }
        if !pivot_value.is_finite() || pivot_value < tolerance {
            return Err(());
        }
        if pivot != column {
            for entry in 0..size {
                matrix.swap(column * size + entry, pivot * size + entry);
            }
            rhs.swap(column, pivot);
        }
        let diagonal = matrix[column * size + column];
        for row in (column + 1)..size {
            let factor = matrix[row * size + column] / diagonal;
            if factor == 0.0 {
                continue;
            }
            matrix[row * size + column] = 0.0;
            for entry in (column + 1)..size {
                matrix[row * size + entry] -= factor * matrix[column * size + entry];
            }
            rhs[row] -= factor * rhs[column];
        }
    }

    let mut solution = vec![0.0; size];
    for row in (0..size).rev() {
        let mut sum = rhs[row];
        for column in (row + 1)..size {
            sum -= matrix[row * size + column] * solution[column];
        }
        solution[row] = sum / matrix[row * size + row];
    }
    Ok(solution)
}

/// Groups the reaction residual by support and checks global force balance.
///
/// Rigid constraints report the residual `K·u − F`; elastic supports report the
/// spring force `−k·u`. The two are mutually exclusive per degree of freedom,
/// which validation enforces, so no component is counted twice.
fn summarize_reactions(model: &Model, system: &System, residual: &[f64], displacement: &[f64]) -> ReactionSummary {
    let layout = &system.layout;
    let translation_components = if model.dimension == 3 { 3 } else { 2 };
    let mut applied = ForceResultant::default();
    for node in 0..model.nodes.len() {
        let mut loads = [0.0_f64; 3];
        for (component, value) in loads.iter_mut().enumerate().take(translation_components) {
            *value = node_value(layout, &system.forces, node, component);
        }
        applied.fx += loads[0];
        applied.fy += loads[1];
        applied.fz += loads[2];
    }
    let supports: Vec<SupportReaction> = model
        .supports
        .iter()
        .filter_map(|support| {
            let node = model.nodes.iter().position(|entry| entry.id == support.node)?;
            let spring = support.spring.unwrap_or_default();
            let component = |rigid: bool, stiffness: f64, index: usize| -> f64 {
                if rigid {
                    node_value(layout, residual, node, index)
                }
                else if stiffness > 0.0 {
                    -stiffness * node_value(layout, displacement, node, index)
                }
                else {
                    0.0
                }
            };
            Some(SupportReaction {
                node: support.node.clone(),
                fx: component(support.dx, spring.kx, 0),
                fy: component(support.dy, spring.ky, 1),
                fz: if model.dimension == 3 { component(support.dz, spring.kz, 2) } else { 0.0 },
                mz: if layout.uses_rotation { component(support.rz, spring.kr, 2) } else { 0.0 },
            })
        })
        .collect();
    let mut reaction = ForceResultant::default();
    for support in &supports {
        reaction.fx += support.fx;
        reaction.fy += support.fy;
        reaction.fz += support.fz;
    }
    let unbalanced =
        ForceResultant { fx: reaction.fx + applied.fx, fy: reaction.fy + applied.fy, fz: reaction.fz + applied.fz };
    let scale = reaction
        .fx
        .abs()
        .max(reaction.fy.abs())
        .max(reaction.fz.abs())
        .max(applied.fx.abs())
        .max(applied.fy.abs())
        .max(applied.fz.abs())
        .max(1.0);
    let tolerance = 1e-8 * scale;
    let balanced = unbalanced.fx.abs() <= tolerance && unbalanced.fy.abs() <= tolerance && unbalanced.fz.abs() <= tolerance;
    ReactionSummary { supports, reaction, applied, unbalanced, tolerance, balanced }
}

/// Cholesky factor of a symmetric positive-definite matrix stored row-major.
///
/// Returns `None` when the matrix is not numerically positive definite.
fn cholesky(matrix: &[f64], size: usize) -> Option<Vec<f64>> {
    let scale = matrix.iter().fold(0.0_f64, |largest, value| largest.max(value.abs()));
    if !positive_finite(scale) {
        return None;
    }
    let tolerance = 1e-12 * scale;
    let mut lower = vec![0.0; size * size];
    for row in 0..size {
        for column in 0..=row {
            let mut sum = matrix[row * size + column];
            for k in 0..column {
                sum -= lower[row * size + k] * lower[column * size + k];
            }
            if row == column {
                if sum <= tolerance || sum.is_nan() {
                    return None;
                }
                lower[row * size + column] = sum.sqrt();
            }
            else {
                lower[row * size + column] = sum / lower[column * size + column];
            }
        }
    }
    Some(lower)
}

fn forward_substitution(lower: &[f64], rhs: &[f64], size: usize) -> Vec<f64> {
    let mut solution = vec![0.0; size];
    for row in 0..size {
        let mut sum = rhs[row];
        for column in 0..row {
            sum -= lower[row * size + column] * solution[column];
        }
        solution[row] = sum / lower[row * size + row];
    }
    solution
}

fn back_substitution(lower: &[f64], rhs: &[f64], size: usize) -> Vec<f64> {
    let mut solution = vec![0.0; size];
    for row in (0..size).rev() {
        let mut sum = rhs[row];
        for column in (row + 1)..size {
            sum -= lower[column * size + row] * solution[column];
        }
        solution[row] = sum / lower[row * size + row];
    }
    solution
}

fn residual(system: &System, displacement: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0; system.layout.size];
    for (row, value) in result.iter_mut().enumerate() {
        let start = row * system.layout.size;
        let sum: f64 = system.stiffness[start..start + system.layout.size]
            .iter()
            .zip(displacement)
            .map(|(stiffness, displacement)| stiffness * displacement)
            .sum();
        *value = sum - system.forces[row];
    }
    result
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
