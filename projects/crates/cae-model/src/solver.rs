use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    CaeError, Severity,
    model::{Element, ElementKind, Load, LoadCombination, Model, PatternFactor, Units, rotation_nodes},
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

/// Version of the JSON report produced by [`solve_deflection`].
pub const DEFLECTION_REPORT_VERSION: u32 = 1;

/// Format tag of the deflected-shape JSON report.
pub const DEFLECTION_REPORT_FORMAT: &str = "open-cae-deflection-report";

/// Version of the JSON report produced by [`solve_nodal_stress`].
pub const NODAL_STRESS_REPORT_VERSION: u32 = 1;

/// Format tag of the averaged nodal-stress JSON report.
pub const NODAL_STRESS_REPORT_FORMAT: &str = "open-cae-nodal-stress-report";

/// Version of the JSON report produced by [`summarize_load_cases`].
pub const LOAD_CASE_REPORT_VERSION: u32 = 1;

/// Format tag of the load-case summary JSON report.
pub const LOAD_CASE_REPORT_FORMAT: &str = "open-cae-load-case-report";

/// Version of the JSON report produced by [`solve_matrices`].
pub const MATRIX_REPORT_VERSION: u32 = 1;

/// Format tag of the assembled stiffness and mass matrix report.
pub const MATRIX_REPORT_FORMAT: &str = "open-cae-matrix-report";

/// Version of the JSON report produced by [`solve_response_spectrum`].
pub const RESPONSE_SPECTRUM_REPORT_VERSION: u32 = 1;

/// Format tag of the response-spectrum JSON report.
pub const RESPONSE_SPECTRUM_REPORT_FORMAT: &str = "open-cae-response-spectrum-report";

/// Version of the JSON report produced by [`solve_frequency_response`].
pub const FREQUENCY_RESPONSE_REPORT_VERSION: u32 = 1;

/// Format tag of the harmonic frequency-response JSON report.
pub const FREQUENCY_RESPONSE_REPORT_FORMAT: &str = "open-cae-frequency-response-report";

/// Version of the JSON report produced by [`solve_construction_stages`].
pub const CONSTRUCTION_STAGE_REPORT_VERSION: u32 = 1;

/// Format tag of the construction-stage JSON report.
pub const CONSTRUCTION_STAGE_REPORT_FORMAT: &str = "open-cae-construction-stage-report";

/// Version of the JSON report produced by [`solve_nonlinear`].
pub const NONLINEAR_REPORT_VERSION: u32 = 1;

/// Format tag of the geometric-nonlinear JSON report.
pub const NONLINEAR_REPORT_FORMAT: &str = "open-cae-nonlinear-report";

/// Version of the JSON report produced by [`solve_tension_only`].
pub const TENSION_ONLY_REPORT_VERSION: u32 = 1;

/// Format tag of the tension-only JSON report.
pub const TENSION_ONLY_REPORT_FORMAT: &str = "open-cae-tension-only-report";

/// Version of the JSON report produced by [`solve_heat`].
pub const HEAT_REPORT_VERSION: u32 = 1;

/// Format tag of the steady-state heat-conduction JSON report.
pub const HEAT_REPORT_FORMAT: &str = "open-cae-heat-report";

/// Value identifying a heat-conduction result in [`HeatSolution::analysis`].
pub const HEAT_ANALYSIS: &str = "heat";

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

/// Solved temperature of one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatTemperature {
    /// Node identifier.
    pub id: String,
    /// Solved temperature, in kelvin or degrees Celsius.
    pub temperature: f64,
    /// `true` when the temperature was fixed by a Dirichlet boundary.
    pub fixed: bool,
}

/// Surface convection of one member in a heat-conduction solution.
///
/// The member loses `h·P·L·(T_avg − T_a)` to the surrounding medium, where
/// `T_avg` is the mean of its two end temperatures. A positive `heat` leaves
/// the member; the same positive value is supplied at a fixed-temperature
/// boundary when no other heat enters or leaves the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatConvection {
    /// Convection coefficient `h` in `W/(m²·K)`.
    pub coefficient: f64,
    /// Exposed perimeter `P` in the document length unit.
    pub perimeter: f64,
    /// Ambient temperature `Ta` in kelvin or degrees Celsius.
    pub ambient_temperature: f64,
    /// Convective heat flow `h·P·L·(T_avg − Ta)`, positive out of the member.
    pub heat: f64,
}

/// Steady-state axial heat flow of one member.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatElementFlux {
    /// Element identifier.
    pub id: String,
    /// Start node identifier.
    pub from: String,
    /// End node identifier.
    pub to: String,
    /// Element length.
    pub length: f64,
    /// Thermal conductivity `k` used by the element.
    pub conductivity: f64,
    /// Cross-sectional area `A` used by the element.
    pub area: f64,
    /// Solved temperature at the start node.
    pub temperature_from: f64,
    /// Solved temperature at the end node.
    pub temperature_to: f64,
    /// Axial heat flow `q = k·A·(T_from − T_to)/L`, positive from `from` to
    /// `to`.
    pub flux: f64,
    /// Surface convection of the member, when the element declares it.
    ///
    /// The field is omitted from the JSON for conduction-only members, so
    /// existing reports keep their exact shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub convection: Option<HeatConvection>,
}

/// Heat flow supplied by one fixed-temperature boundary.
///
/// A positive value enters the model at that node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatReaction {
    /// Node identifier.
    pub node: String,
    /// Boundary heat flow in watts.
    pub heat: f64,
}

/// Complete steady-state heat-conduction solution.
///
/// This report is independent of the structural [`Solution`]: it contains no
/// displacement, strain or force fields, and the `analysis` tag prevents it
/// being mistaken for a static result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatSolution {
    /// Report format tag, always `open-cae-heat-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Analysis identifier, always `heat`.
    pub analysis: String,
    /// Unit system declared by the model.
    pub units: String,
    /// Nodal temperatures in model node order.
    pub temperatures: Vec<HeatTemperature>,
    /// Member heat flows in model element order.
    pub element_flux: Vec<HeatElementFlux>,
    /// Fixed-temperature reactions in model node order.
    pub reactions: Vec<HeatReaction>,
    /// Largest absolute free-degree-of-freedom balance residual.
    pub residual: f64,
    /// `true` when the residual is within the deterministic tolerance.
    pub ok: bool,
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
    /// Signed modal participation factor `φᵀ·M·r` in the global X, Y and Z
    /// directions.
    ///
    /// The shape is mass-normalized, so the factor already carries the units of
    /// `sqrt(mass)` and its square is [`Mode::effective_mass`]. The sign follows
    /// the dominant translation of the mode and is therefore useful for
    /// combining modal responses that add or subtract.
    pub participation_factor: [f64; 3],
    /// Effective modal mass `(φᵀ·M·r)²` in the global X, Y and Z directions.
    ///
    /// The ratio to `ModalSolution::total_mass` is the effective mass fraction
    /// of the mode.
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

/// Modal combination rule of a response-spectrum analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpectrumCombination {
    /// Square root of the sum of the squares; the default.
    #[default]
    Srss,
    /// Complete quadratic combination with the Der Kiureghian correlation
    /// coefficient.
    Cqc,
}

impl SpectrumCombination {
    /// Parses the wire value `srss` or `cqc`.
    ///
    /// An unknown value is rejected with the stable
    /// `spectrum/invalid-combination` code.
    pub fn parse(value: &str) -> Result<Self, CaeError> {
        match value {
            "srss" => Ok(Self::Srss),
            "cqc" => Ok(Self::Cqc),
            other => Err(CaeError::new(
                "spectrum/invalid-combination",
                format!("unknown spectrum combination `{other}`; use `srss` or `cqc`"),
            )),
        }
    }

    /// Wire value of the combination, `srss` or `cqc`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Srss => "srss",
            Self::Cqc => "cqc",
        }
    }
}

/// One point of a response spectrum.
///
/// The spectrum gives the spectral acceleration of a single-degree-of-freedom
/// oscillator as a function of its natural period; values between points are
/// interpolated linearly and the end values are held constant beyond the first
/// and last point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumPoint {
    /// Natural period in seconds; must be finite and non-negative.
    pub period: f64,
    /// Spectral acceleration in document length per second squared.
    pub acceleration: f64,
}

/// One mode of a response-spectrum analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumMode {
    /// Natural frequency in hertz.
    pub frequency_hz: f64,
    /// Natural period `1 / frequency` in seconds.
    pub period: f64,
    /// Spectral acceleration read from the input spectrum at `period`.
    pub spectral_acceleration: f64,
    /// Signed participation factor of the mode in the analysed direction.
    pub participation_factor: f64,
    /// Spectral displacement `Sa / ω²`.
    pub spectral_displacement: f64,
    /// Modal displacement amplitude `Γ · Sd`.
    pub scale: f64,
}

/// Result of [`solve_response_spectrum`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSpectrumSolution {
    /// Report format tag, always `open-cae-response-spectrum-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Analysed global direction (`x`, `y` or `z`).
    pub axis: String,
    /// Modal combination that was applied (`srss` or `cqc`).
    pub combination: String,
    /// Modal damping ratio used by the CQC combination; absent for SRSS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damping: Option<f64>,
    /// Requested number of modes.
    pub requested: usize,
    /// Modes used in the combination, in ascending frequency order.
    pub modes: Vec<SpectrumMode>,
    /// Combined nodal displacements after the modal combination. Components
    /// are combined before the displacement magnitude is taken.
    pub nodes: Vec<NodeDisplacement>,
    /// Nodal displacement with the largest combined magnitude.
    pub max_displacement: NodeDisplacement,
}

/// Steady-state response of the model at one excitation frequency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyResponsePoint {
    /// Excitation frequency in hertz.
    pub frequency_hz: f64,
    /// Largest translational displacement amplitude over all nodes.
    pub max_amplitude: f64,
    /// Node that carries `max_amplitude`.
    pub governing_node: String,
}

/// Peak steady-state response of one node over the sampled frequency range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeFrequencyResponse {
    /// Node identifier.
    pub node: String,
    /// Sampled frequency that produced `max_amplitude`.
    pub peak_frequency_hz: f64,
    /// Largest translational displacement amplitude of the node.
    pub max_amplitude: f64,
}

/// Result of [`solve_frequency_response`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyResponseSolution {
    /// Report format tag, always `open-cae-frequency-response-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Analysed global direction (`x`, `y` or `z`).
    pub axis: String,
    /// Requested number of modes.
    pub requested: usize,
    /// Modal damping ratio applied to every mode.
    pub damping_ratio: f64,
    /// First sampled frequency in hertz.
    pub start_hz: f64,
    /// Last sampled frequency in hertz.
    pub end_hz: f64,
    /// Number of equally spaced sampled frequencies, including both ends.
    pub points: usize,
    /// Steady-state amplitude at every sampled frequency.
    pub sweep: Vec<FrequencyResponsePoint>,
    /// Sampled frequency with the largest amplitude.
    pub peak: FrequencyResponsePoint,
    /// Peak amplitude of every node over the sampled range.
    pub nodes: Vec<NodeFrequencyResponse>,
}

/// One construction stage of a staged (birth/death) analysis.
///
/// `elements` lists the element identifiers that are active in this stage;
/// omitting it keeps every element active, so leaving an element out is the
/// deactivation (death) operation. `loads` and `gravity` scale the applied
/// loads and self-weight of the stage exactly like a load combination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionStage {
    /// Unique, non-empty stage name.
    pub name: String,
    /// Active element identifiers; `None` keeps all elements active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elements: Option<Vec<String>>,
    /// Factor applied to nodal and element applied loads (default 1).
    #[serde(default = "default_stage_loads")]
    pub loads: f64,
    /// Factor applied to the gravity vector (default 0).
    #[serde(default)]
    pub gravity: f64,
}

fn default_stage_loads() -> f64 {
    1.0
}

/// Summary of one solved construction stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionStageResult {
    /// Stage name.
    pub name: String,
    /// Number of elements active in the stage.
    pub active_elements: usize,
    /// Factor applied to nodal and element applied loads.
    pub loads: f64,
    /// Factor applied to the gravity vector.
    pub gravity: f64,
    /// Sum of the applied external loads of this stage.
    pub applied: ForceResultant,
    /// Largest nodal displacement magnitude.
    pub max_displacement: f64,
    /// Node that carries `max_displacement`.
    pub max_displacement_node: String,
    /// Signed axial force with the largest magnitude.
    pub max_axial_force: f64,
    /// Element that carries `max_axial_force`.
    pub max_axial_element: String,
    /// Largest absolute bending moment.
    pub max_abs_moment: f64,
    /// Element that carries `max_abs_moment`.
    pub max_moment_element: String,
    /// Total strain energy of the stage.
    pub strain_energy: f64,
    /// `true` when `reaction + applied` is within tolerance.
    pub balanced: bool,
}

/// Result of [`solve_construction_stages`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionStageSolution {
    /// Report format tag, always `open-cae-construction-stage-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Stage summaries in the requested order.
    pub stages: Vec<ConstructionStageResult>,
}

/// Result of [`solve_nonlinear`].
///
/// The solver applies the load in equal increments and runs a Newton iteration
/// with the corotational (large-displacement) tangent of every truss bar at each
/// increment. `iterations` counts every residual evaluation that advanced a
/// Newton step, and `residual_norm` is the free residual norm of the last
/// accepted state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NonlinearSolution {
    /// Report format tag, always `open-cae-nonlinear-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of equal load increments used by the path-following loop.
    pub increments: usize,
    /// Total number of Newton iterations over all increments.
    pub iterations: usize,
    /// Relative convergence tolerance that was requested.
    pub tolerance: f64,
    /// Free residual norm of the final accepted state.
    pub residual_norm: f64,
    /// Nodal displacements of the final state.
    pub nodes: Vec<NodeDisplacement>,
    /// Nodal displacement with the largest magnitude.
    pub max_displacement: NodeDisplacement,
}

/// Active state of one tension-only truss in [`TensionOnlySolution`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TensionOnlyElement {
    /// Element identifier.
    pub id: String,
    /// `true` when the member contributed stiffness and loads in the final
    /// active set; `false` when it was slack and removed.
    pub active: bool,
    /// Axial force of the member in the final solution (zero when slack).
    pub axial_force: f64,
}

/// Result of [`solve_tension_only`].
///
/// The report wraps the final static [`Solution`] and lists the active state of
/// every tension-only member. `iterations` counts the static solves performed;
/// `converged` is `false` when the active set was still changing after the
/// requested maximum number of iterations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TensionOnlySolution {
    /// Report format tag, always `open-cae-tension-only-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of static solves performed by the active-set iteration.
    pub iterations: usize,
    /// `true` when the active set reached a stable state.
    pub converged: bool,
    /// Final static solution with the accepted active set.
    pub solution: Solution,
    /// Active state of every tension-only member, in model element order.
    pub elements: Vec<TensionOnlyElement>,
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

/// Deflected-shape sample at one station along an element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeflectionStation {
    /// Distance from the `from` node along the member.
    pub position: f64,
    /// Local axial displacement `u`, positive from `from` to `to`.
    pub axial: f64,
    /// Local transverse deflection `v`, positive along the local `+y` axis.
    pub transverse: f64,
    /// Section rotation about the local z axis in radians.
    pub rotation: f64,
    /// Deformed global X coordinate of the centreline sample.
    pub x: f64,
    /// Deformed global Y coordinate of the centreline sample.
    pub y: f64,
    /// Deformed global Z coordinate of the centreline sample.
    pub z: f64,
}

/// Deflected shape of one element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementDeflection {
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
    /// Largest absolute local transverse deflection among the stations.
    pub max_abs_transverse: f64,
    /// Sampled stations from the `from` node to the `to` node.
    pub stations: Vec<DeflectionStation>,
}

/// Governing transverse deflection of a deflected-shape report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeflectionGoverning {
    /// Element that carries the largest absolute transverse deflection.
    pub element: String,
    /// Position along that element.
    pub position: f64,
    /// Signed local transverse deflection at that position.
    pub transverse: f64,
}

/// Result of [`solve_deflection`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeflectionSolution {
    /// Report format tag, always `open-cae-deflection-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of stations sampled per element, including both ends.
    pub stations: usize,
    /// Element with the largest absolute transverse deflection.
    pub governing: DeflectionGoverning,
    /// Per-element shapes in model element order.
    pub elements: Vec<ElementDeflection>,
}

/// Averaged stress state at one node.
///
/// Element-end stresses from every connected member are averaged at the node.
/// `min_stress` and `max_stress` are the extreme values of the connected
/// element ends, so they bound the stress range at the node instead of
/// averaging it away.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStress {
    /// Node identifier.
    pub node: String,
    /// Number of connected element ends that contributed.
    pub connected_elements: usize,
    /// Arithmetic mean of the axial stresses at the connected element ends.
    pub mean_axial_stress: f64,
    /// Smallest extreme-fibre stress among the connected element ends.
    pub min_stress: f64,
    /// Largest extreme-fibre stress among the connected element ends.
    pub max_stress: f64,
    /// Largest absolute extreme-fibre stress among the connected element ends.
    pub max_abs_stress: f64,
    /// Element that produced `max_abs_stress`.
    pub governing_element: String,
}

/// Governing node of an averaged nodal-stress report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodalStressGoverning {
    /// Node with the largest absolute extreme-fibre stress.
    pub node: String,
    /// Element that produced that stress at the node.
    pub element: String,
    /// Largest absolute extreme-fibre stress.
    pub max_abs_stress: f64,
    /// `maxAbsStress / allowableStress` when the material defines one.
    pub utilization: Option<f64>,
}

/// Result of [`solve_nodal_stress`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodalStressSolution {
    /// Report format tag, always `open-cae-nodal-stress-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of stations sampled per element before averaging.
    pub stations: usize,
    /// Allowable stress used for utilisation checks, when declared.
    pub allowable_stress: Option<f64>,
    /// Governing node.
    pub governing: NodalStressGoverning,
    /// Per-node averaged stresses in model node order.
    pub nodes: Vec<NodeStress>,
}

/// One row of a load-case summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadCaseResult {
    /// Case name; the unfactored static solve is always called `base`.
    pub name: String,
    /// Factor applied to nodal and element applied loads.
    pub loads: f64,
    /// Factor applied to the gravity vector.
    pub gravity: f64,
    /// Sum of the applied external loads of this case.
    pub applied: ForceResultant,
    /// Largest nodal displacement magnitude.
    pub max_displacement: f64,
    /// Node that carries `max_displacement`.
    pub max_displacement_node: String,
    /// Signed axial force with the largest magnitude.
    pub max_axial_force: f64,
    /// Element that carries `max_axial_force`.
    pub max_axial_element: String,
    /// Largest absolute bending moment.
    pub max_abs_moment: f64,
    /// Element that carries `max_abs_moment`.
    pub max_moment_element: String,
    /// Total strain energy of the case.
    pub strain_energy: f64,
    /// `true` when `reaction + applied` is within tolerance.
    pub balanced: bool,
}

/// Result of [`summarize_load_cases`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadCaseSummary {
    /// Report format tag, always `open-cae-load-case-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Unfactored case followed by every stored load combination.
    pub cases: Vec<LoadCaseResult>,
}

/// One active degree of freedom of an assembled system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDof {
    /// Node identifier.
    pub node: String,
    /// Component name (`ux`, `uy`, `uz` or `rz`).
    pub component: String,
    /// `true` when a support constrains this degree of freedom.
    pub constrained: bool,
}

/// Assembled global stiffness and mass matrices in active degree-of-freedom
/// order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMatrices {
    /// Report format tag, always `open-cae-matrix-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of active degrees of freedom.
    pub size: usize,
    /// Degree-of-freedom labels in matrix row/column order.
    pub dofs: Vec<MatrixDof>,
    /// Dense global stiffness matrix in `dofs` order.
    pub stiffness: Vec<Vec<f64>>,
    /// Dense global mass matrix; the lumped mass model is diagonal.
    pub mass: Vec<Vec<f64>>,
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
    /// Named load patterns mixed into this combination.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<PatternFactor>,
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
    /// `false` for a slack tension-only member excluded from stiffness and
    /// loads while still appearing in the result list.
    active: bool,
    /// Global degree of freedom of every local element degree of freedom.
    ///
    /// `None` marks a released beam rotation that is condensed out and has no
    /// global counterpart.
    displacement_dofs: Vec<Option<usize>>,
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
    /// Original continuous 6×6 stiffness, used to recover the internal rotation
    /// of released ends.
    stiffness: [[f64; 6]; 6],
    /// Original consistent local load vector.
    load: [f64; 6],
    /// Released local degrees of freedom (only indices 2 and 5).
    released: [bool; 6],
    /// Combined user and gravity transverse intensity at the start node.
    transverse_start: f64,
    /// Combined user and gravity transverse intensity at the end node.
    transverse_end: f64,
    /// Uniform distributed axial load per unit length (gravity component).
    axial_uniform: f64,
    /// Distributed load limited to a fraction of the member length.
    partial: Option<PartialLoad>,
}

/// Distributed load acting between two fractions of the member length.
#[derive(Debug, Clone, Copy)]
struct PartialLoad {
    /// Start of the loaded segment as a fraction of the length.
    start: f64,
    /// End of the loaded segment as a fraction of the length.
    end: f64,
    /// Load intensity at [`PartialLoad::start`].
    load_start: f64,
    /// Load intensity at [`PartialLoad::end`].
    load_end: f64,
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
    /// Per-node rotation into the local frame of an inclined support.
    ///
    /// `stiffness`, `forces` and `prescribed` follow this frame; the helpers
    /// [`System::to_global_vector`] and [`System::to_global_matrix`] convert
    /// results back to the global X–Y axes.
    rotations: Vec<Option<[[f64; 2]; 2]>>,
}

impl System {
    /// Maps a vector from the assembled (possibly rotated) frame to global
    /// coordinates.
    fn to_global_vector(&self, values: &[f64]) -> Vec<f64> {
        let mut global = values.to_vec();
        for (node, rotation) in self.rotations.iter().enumerate() {
            let Some(rotation) = rotation
            else {
                continue;
            };
            let Some(first) = self.layout.index.get(node * self.layout.slots).copied().flatten()
            else {
                continue;
            };
            let Some(second) = self.layout.index.get(node * self.layout.slots + 1).copied().flatten()
            else {
                continue;
            };
            let a = values[first];
            let b = values[second];
            global[first] = rotation[0][0] * a + rotation[0][1] * b;
            global[second] = rotation[1][0] * a + rotation[1][1] * b;
        }
        global
    }

    /// Maps a square matrix from the assembled frame to global coordinates.
    fn to_global_matrix(&self, matrix: &[f64]) -> Vec<f64> {
        let size = self.layout.size;
        let mut global = matrix.to_vec();
        for (node, rotation) in self.rotations.iter().enumerate() {
            let Some(rotation) = rotation
            else {
                continue;
            };
            let Some(first) = self.layout.index.get(node * self.layout.slots).copied().flatten()
            else {
                continue;
            };
            let Some(second) = self.layout.index.get(node * self.layout.slots + 1).copied().flatten()
            else {
                continue;
            };
            for row in 0..size {
                let a = global[row * size + first];
                let b = global[row * size + second];
                global[row * size + first] = a * rotation[0][0] + b * rotation[0][1];
                global[row * size + second] = a * rotation[1][0] + b * rotation[1][1];
            }
            for column in 0..size {
                let a = global[first * size + column];
                let b = global[second * size + column];
                global[first * size + column] = rotation[0][0] * a + rotation[0][1] * b;
                global[second * size + column] = rotation[1][0] * a + rotation[1][1] * b;
            }
        }
        global
    }
    /// Maps a square matrix from global coordinates into the assembled frame.
    fn to_local_matrix(&self, matrix: &[f64]) -> Vec<f64> {
        let size = self.layout.size;
        let mut local = matrix.to_vec();
        for (node, rotation) in self.rotations.iter().enumerate() {
            let Some(rotation) = rotation
            else {
                continue;
            };
            let Some(first) = self.layout.index.get(node * self.layout.slots).copied().flatten()
            else {
                continue;
            };
            let Some(second) = self.layout.index.get(node * self.layout.slots + 1).copied().flatten()
            else {
                continue;
            };
            for row in 0..size {
                let a = local[row * size + first];
                let b = local[row * size + second];
                local[row * size + first] = a * rotation[0][0] + b * rotation[1][0];
                local[row * size + second] = a * rotation[0][1] + b * rotation[1][1];
            }
            for column in 0..size {
                let a = local[first * size + column];
                let b = local[second * size + column];
                local[first * size + column] = rotation[0][0] * a + rotation[1][0] * b;
                local[second * size + column] = rotation[0][1] * a + rotation[1][1] * b;
            }
        }
        local
    }
}

/// Rotates the two translational degrees of freedom of every inclined support
/// node into its local frame: `K ← Qᵀ·K·Q` and `F ← Qᵀ·F`.
fn apply_node_rotation(stiffness: &mut [f64], forces: &mut [f64], layout: &Layout, rotations: &[Option<[[f64; 2]; 2]>]) {
    let size = layout.size;
    for (node, rotation) in rotations.iter().enumerate() {
        let Some(rotation) = rotation
        else {
            continue;
        };
        let Some(first) = layout.index.get(node * layout.slots).copied().flatten()
        else {
            continue;
        };
        let Some(second) = layout.index.get(node * layout.slots + 1).copied().flatten()
        else {
            continue;
        };
        for row in 0..size {
            let a = stiffness[row * size + first];
            let b = stiffness[row * size + second];
            stiffness[row * size + first] = a * rotation[0][0] + b * rotation[1][0];
            stiffness[row * size + second] = a * rotation[0][1] + b * rotation[1][1];
        }
        for column in 0..size {
            let a = stiffness[first * size + column];
            let b = stiffness[second * size + column];
            stiffness[first * size + column] = rotation[0][0] * a + rotation[1][0] * b;
            stiffness[second * size + column] = rotation[0][1] * a + rotation[1][1] * b;
        }
        let a = forces[first];
        let b = forces[second];
        forces[first] = rotation[0][0] * a + rotation[1][0] * b;
        forces[second] = rotation[0][1] * a + rotation[1][1] * b;
    }
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
    solve_assembled(model, system)
}

/// Solves an already assembled system and builds the static result report.
fn solve_assembled(model: &Model, system: System) -> Result<Solution, CaeError> {
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

    let global_displacement = system.to_global_vector(&displacement);
    let global_residual = system.to_global_vector(&residual(&system, &displacement));
    let layout = &system.layout;
    let nodes: Vec<NodeDisplacement> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let ux = node_value(layout, &global_displacement, index, 0);
            let uy = node_value(layout, &global_displacement, index, 1);
            let uz = if model.dimension == 3 { node_value(layout, &global_displacement, index, 2) } else { 0.0 };
            let rz = if layout.uses_rotation { node_value(layout, &global_displacement, index, 2) } else { 0.0 };
            NodeDisplacement { id: node.id.clone(), ux, uy, uz, rz, magnitude: (ux * ux + uy * uy + uz * uz).sqrt() }
        })
        .collect();
    let reactions = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let fx = node_value(layout, &global_residual, index, 0);
            let fy = node_value(layout, &global_residual, index, 1);
            let fz = if model.dimension == 3 { node_value(layout, &global_residual, index, 2) } else { 0.0 };
            let mz = if layout.uses_rotation { node_value(layout, &global_residual, index, 2) } else { 0.0 };
            NodeReaction { node: node.id.clone(), fx, fy, fz, mz }
        })
        .collect();
    let reaction_summary = summarize_reactions(model, &system, &global_residual, &global_displacement);

    let mut elements = Vec::with_capacity(system.elements.len());
    let mut strain_energy = 0.0;
    for element in &system.elements {
        if !element.active {
            elements.push(ElementResult {
                id: element.id.clone(),
                from: model.nodes[element.from].id.clone(),
                to: model.nodes[element.to].id.clone(),
                kind: element.kind,
                length: element.length,
                axial_force: 0.0,
                stress: 0.0,
                strain: 0.0,
                end_forces: EndForces { n1: 0.0, v1: 0.0, m1: 0.0, n2: 0.0, v2: 0.0, m2: 0.0 },
                max_abs_moment: 0.0,
            });
            continue;
        }
        let local_displacement: Vec<f64> =
            element.displacement_dofs.iter().map(|slot| slot.map(|dof| displacement[dof]).unwrap_or(0.0)).collect();
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
                restore_released_rotations(beam, &mut local);
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

/// Solves steady-state heat conduction through the axial members of a model.
///
/// Every member conducts along its own axis with the 1D element matrix
/// `k·A/L·[[1, -1], [-1, 1]]`, where `k` is the material `conductivity` and `A`
/// is the element area override or the global material area. Nodes that declare
/// a `temperature` are Dirichlet boundaries; every other node is free and may
/// carry a concentrated `heat` flow. The assembly, reduction and dense solve are
/// the same numerical path as the structural solver, but the result carries
/// temperatures and heat flows instead of displacements and forces.
///
/// The reported element flux is `q = k·A·(T_from − T_to)/L`, positive when heat
/// flows from `from` to `to`. A fixed-temperature reaction is the boundary heat
/// the support supplies to the model, so `Σreaction + Σapplied heat = 0` for
/// conduction-only models.
///
/// A member that declares `convection` (and a `perimeter`) also exchanges heat
/// with the model `ambient_temperature` `Ta`. Its element matrix becomes
/// `k·A/L·[[1, -1], [-1, 1]] + h·P·L/6·[[2, 1], [1, 2]]` and its load vector
/// receives `h·P·L·Ta/2·[1, 1]`. The member reports the convective heat
/// `h·P·L·(T_avg − Ta)` with `T_avg = (T_from + T_to)/2`, positive out of the
/// member. Summing the element nodal balances therefore gives
/// `Σfixed-temperature reaction + Σapplied nodal heat = Σconvective heat`, so a
/// convection-only model balances its reactions against the convective loss.
///
/// The analysis is deterministic and contains no timestamps. It fails with
/// `model/no-nodes`, `model/no-elements`, `material/invalid-conductivity`,
/// `load/invalid-heat`, `node/invalid-temperature`,
/// `heat/no-fixed-temperature`, `heat/invalid-convection`,
/// `heat/invalid-perimeter`, `heat/missing-perimeter`, `heat/missing-ambient`
/// or `heat/invalid-ambient` instead of approximating a solution.
pub fn solve_heat(model: &Model) -> Result<HeatSolution, CaeError> {
    if model.nodes.is_empty() {
        return Err(CaeError::new("model/no-nodes", "the model requires at least 1 node"));
    }
    if model.elements.is_empty() {
        return Err(CaeError::new("model/no-elements", "the model requires at least 1 element"));
    }
    let conductivity = match model.material.conductivity {
        Some(value) if positive_finite(value) => value,
        other => {
            return Err(CaeError::new(
                "material/invalid-conductivity",
                format!("heat-conduction analysis requires a positive finite thermal conductivity (currently {other:?})"),
            ));
        }
    };
    if model.ambient_temperature.is_some_and(|value| !value.is_finite()) {
        return Err(CaeError::new(
            "heat/invalid-ambient",
            format!("ambient temperature must be finite (currently {:?})", model.ambient_temperature),
        ));
    }
    let ambient = model.ambient_temperature;
    let size = model.nodes.len();
    let index: HashMap<&str, usize> =
        model.nodes.iter().enumerate().map(|(position, node)| (node.id.as_str(), position)).collect();
    let mut fixed = vec![false; size];
    let mut temperatures = vec![0.0; size];
    for (position, node) in model.nodes.iter().enumerate() {
        if !node.heat.is_finite() {
            return Err(CaeError::new(
                "load/invalid-heat",
                format!("heat flow of node `{}` must be finite (currently {})", node.id, node.heat),
            ));
        }
        if let Some(value) = node.temperature {
            if !value.is_finite() {
                return Err(CaeError::new(
                    "node/invalid-temperature",
                    format!("fixed temperature of node `{}` must be finite", node.id),
                ));
            }
            fixed[position] = true;
            temperatures[position] = value;
        }
    }
    if !fixed.iter().any(|value| *value) {
        return Err(CaeError::new(
            "heat/no-fixed-temperature",
            "steady-state heat conduction requires at least one node with a fixed temperature",
        ));
    }

    let mut stiffness = vec![0.0; size * size];
    let mut forces: Vec<f64> = model.nodes.iter().map(|node| node.heat).collect();
    let mut conductors = Vec::with_capacity(model.elements.len());
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
        if from == to {
            return Err(CaeError::new(
                "element/degenerate",
                format!("element `{}` must not have identical start and end nodes", element.id),
            ));
        }
        let start = &model.nodes[from];
        let end = &model.nodes[to];
        let length = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2) + (end.z - start.z).powi(2)).sqrt();
        if !positive_finite(length) {
            return Err(CaeError::new("element/zero-length", format!("element `{}` has zero length", element.id)));
        }
        let area = element.material.as_ref().and_then(|material| material.area).unwrap_or(model.material.area);
        if !positive_finite(area) {
            return Err(CaeError::new(
                "material/invalid",
                format!("cross-sectional area of element `{}` must be positive and finite (currently {area})", element.id),
            ));
        }
        let conductance = conductivity * area / length;
        stiffness[from * size + from] += conductance;
        stiffness[to * size + to] += conductance;
        stiffness[from * size + to] -= conductance;
        stiffness[to * size + from] -= conductance;
        let convection = match element.convection {
            None => None,
            Some(coefficient) => {
                if !positive_finite(coefficient) {
                    return Err(CaeError::new(
                        "heat/invalid-convection",
                        format!(
                            "convection coefficient of element `{}` must be positive and finite (currently {coefficient})",
                            element.id
                        ),
                    ));
                }
                let perimeter = element.perimeter.ok_or_else(|| {
                    CaeError::new(
                        "heat/missing-perimeter",
                        format!("element `{}` declares convection but no perimeter", element.id),
                    )
                })?;
                if !positive_finite(perimeter) {
                    return Err(CaeError::new(
                        "heat/invalid-perimeter",
                        format!("perimeter of element `{}` must be positive and finite (currently {perimeter})", element.id),
                    ));
                }
                let ambient = ambient.ok_or_else(|| {
                    CaeError::new("heat/missing-ambient", "surface convection requires a finite model ambient temperature")
                })?;
                let surface = coefficient * perimeter * length;
                stiffness[from * size + from] += surface / 3.0;
                stiffness[from * size + to] += surface / 6.0;
                stiffness[to * size + from] += surface / 6.0;
                stiffness[to * size + to] += surface / 3.0;
                forces[from] += surface * ambient / 2.0;
                forces[to] += surface * ambient / 2.0;
                Some((coefficient, perimeter, ambient))
            }
        };
        conductors.push((element.id.clone(), element.from.clone(), element.to.clone(), from, to, length, area, convection));
    }

    let free: Vec<usize> = (0..size).filter(|position| !fixed[*position]).collect();
    if !free.is_empty() {
        let mut reduced = vec![0.0; free.len() * free.len()];
        let mut rhs = vec![0.0; free.len()];
        for (row, &dof) in free.iter().enumerate() {
            rhs[row] = forces[dof];
            for (column, &other) in free.iter().enumerate() {
                reduced[row * free.len() + column] = stiffness[dof * size + other];
            }
            for other in (0..size).filter(|position| fixed[*position]) {
                rhs[row] -= stiffness[dof * size + other] * temperatures[other];
            }
        }
        let solved = solve_dense(reduced, rhs, free.len()).map_err(|()| {
            CaeError::new(
                "heat/no-fixed-temperature",
                "the conduction matrix is singular; every connected component needs at least one fixed temperature",
            )
        })?;
        for (position, &dof) in free.iter().enumerate() {
            temperatures[dof] = solved[position];
        }
        if solved.iter().any(|value| !value.is_finite()) {
            return Err(CaeError::new(
                "heat/no-fixed-temperature",
                "the temperature solution is not finite; check the fixed temperatures and heat flows",
            ));
        }
    }
    let mut element_flux = Vec::with_capacity(conductors.len());
    let mut peak = 0.0_f64;
    for (id, from_id, to_id, from, to, length, area, convection) in conductors {
        let flux = conductivity * area / length * (temperatures[from] - temperatures[to]);
        peak = peak.max(flux.abs());
        let convection = convection.map(|(coefficient, perimeter, ambient_temperature)| {
            let average = (temperatures[from] + temperatures[to]) / 2.0;
            HeatConvection {
                coefficient,
                perimeter,
                ambient_temperature,
                heat: coefficient * perimeter * length * (average - ambient_temperature),
            }
        });
        element_flux.push(HeatElementFlux {
            id,
            from: from_id,
            to: to_id,
            length,
            conductivity,
            area,
            temperature_from: temperatures[from],
            temperature_to: temperatures[to],
            flux,
            convection,
        });
    }

    let mut reactions = Vec::new();
    for (position, node) in model.nodes.iter().enumerate() {
        if !fixed[position] {
            continue;
        }
        let balance: f64 =
            (0..size).map(|column| stiffness[position * size + column] * temperatures[column]).sum::<f64>() - forces[position];
        reactions.push(HeatReaction { node: node.id.clone(), heat: balance });
    }

    let mut residual = 0.0_f64;
    for &dof in &free {
        let balance: f64 =
            (0..size).map(|column| stiffness[dof * size + column] * temperatures[column]).sum::<f64>() - forces[dof];
        residual = residual.max(balance.abs());
    }
    let scale = forces.iter().fold(peak, |current, value| current.max(value.abs())).max(1.0);
    let ok = residual.is_finite() && residual <= 1e-9 * scale;

    Ok(HeatSolution {
        format: HEAT_REPORT_FORMAT.to_string(),
        version: HEAT_REPORT_VERSION,
        analysis: HEAT_ANALYSIS.to_string(),
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        temperatures: model
            .nodes
            .iter()
            .enumerate()
            .map(|(position, node)| HeatTemperature {
                id: node.id.clone(),
                temperature: temperatures[position],
                fixed: fixed[position],
            })
            .collect(),
        element_flux,
        reactions,
        residual,
        ok,
    })
}

/// Solves a linear-static model with tension-only (cable) truss members.
///
/// The analysis starts with every member active. It repeatedly solves the
/// assembled static system and deactivates each tension-only member that
/// carries compression (an axial force below `-tolerance × the largest absolute
/// axial force`), then reassembles the stiffness, self-weight and thermal loads
/// without those members. The loop stops when the active set no longer changes,
/// or after `max_iterations` solves, in which case `converged` is `false`.
///
/// Slack members remain in the result list with zero axial force. A model that
/// loses stability when cables go slack fails with `model/mechanism`, which is
/// the same mechanism check as the ordinary static solver. Only truss elements
/// can be tension-only; validation rejects the flag on beams.
pub fn solve_tension_only(model: &Model, max_iterations: usize, tolerance: f64) -> Result<TensionOnlySolution, CaeError> {
    if max_iterations == 0 {
        return Err(CaeError::new("analysis/invalid-options", "tension-only analysis requires at least one iteration"));
    }
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(CaeError::new(
            "analysis/invalid-options",
            format!("tension-only tolerance must be finite and non-negative (currently {tolerance})"),
        ));
    }
    if let Some(issue) = model.validate().into_iter().find(|issue| issue.severity == Severity::Error) {
        return Err(CaeError::new(issue.code, issue.message));
    }
    let mut active = vec![true; model.elements.len()];
    let mut iterations = 0;
    loop {
        iterations += 1;
        let system = assemble_with_active(model, Some(&active))?;
        let solution = solve_assembled(model, system)?;
        let scale = solution.elements.iter().map(|element| element.axial_force.abs()).fold(0.0_f64, f64::max);
        let mut changed = false;
        for (index, element) in model.elements.iter().enumerate() {
            if !element.tension_only || !active[index] {
                continue;
            }
            let axial_force = solution.elements[index].axial_force;
            if scale > 0.0 && axial_force < -tolerance * scale {
                active[index] = false;
                changed = true;
            }
        }
        if !changed || iterations >= max_iterations {
            let elements = model
                .elements
                .iter()
                .enumerate()
                .filter(|(_, element)| element.tension_only)
                .map(|(index, element)| TensionOnlyElement {
                    id: element.id.clone(),
                    active: active[index],
                    axial_force: if active[index] { solution.elements[index].axial_force } else { 0.0 },
                })
                .collect();
            return Ok(TensionOnlySolution {
                format: TENSION_ONLY_REPORT_FORMAT.to_string(),
                version: TENSION_ONLY_REPORT_VERSION,
                units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
                iterations,
                converged: !changed,
                solution,
                elements,
            });
        }
    }
}

/// Exports the assembled global stiffness and lumped mass matrices.
///
/// Every active degree of freedom of the model is included, in the same order
/// the solver uses, and each row records whether a support constrains it. The
/// matrices are dense and expressed in the global coordinate system, so their
/// rows and columns can be checked against hand-assembled values. The mass
/// matrix follows the lumped model of [`solve_modal`], includes nodal point
/// masses and is diagonal; a density of zero without nodal masses therefore
/// yields a zero mass matrix rather than an error.
pub fn solve_matrices(model: &Model) -> Result<SystemMatrices, CaeError> {
    if let Some(issue) = model.validate().into_iter().find(|issue| issue.severity == Severity::Error) {
        return Err(CaeError::new(issue.code, issue.message));
    }
    let system = assemble(model)?;
    let layout = &system.layout;
    let size = layout.size;
    let mass = mass_matrix(model, layout)?;
    let mut dofs = Vec::with_capacity(size);
    for node in 0..model.nodes.len() {
        for component in 0..layout.slots {
            let Some(dof) = layout.index.get(node * layout.slots + component).copied().flatten()
            else {
                continue;
            };
            let name = component_name(model, layout, component);
            dofs.push(MatrixDof {
                node: model.nodes[node].id.clone(),
                component: name.to_string(),
                constrained: system.constrained[dof],
            });
        }
    }
    let stiffness = system.to_global_matrix(&system.stiffness);
    let stiffness = (0..size).map(|row| stiffness[row * size..(row + 1) * size].to_vec()).collect();
    let mut mass_rows = vec![vec![0.0; size]; size];
    for (index, value) in mass.iter().enumerate().take(size) {
        mass_rows[index][index] = *value;
    }
    Ok(SystemMatrices {
        format: MATRIX_REPORT_FORMAT.to_string(),
        version: MATRIX_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        size,
        dofs,
        stiffness,
        mass: mass_rows,
    })
}

fn component_name(model: &Model, layout: &Layout, component: usize) -> &'static str {
    if model.dimension == 3 {
        ["ux", "uy", "uz"][component]
    }
    else if layout.uses_rotation {
        ["ux", "uy", "rz"][component]
    }
    else {
        ["ux", "uy"][component]
    }
}

/// Solves the generalized eigenproblem `K·φ = ω²·M·φ` with a diagonal mass
/// matrix and returns the lowest `modes` frequencies in hertz.
///
/// Mass must come from a strictly positive material density or from at least
/// one positive nodal `Node::mass`. When neither is available the analysis is
/// rejected with `material/invalid-density` instead of being approximated.
pub fn solve_modal(model: &Model, modes: usize) -> Result<ModalSolution, CaeError> {
    if modes == 0 {
        return Err(CaeError::new("analysis/invalid-modes", "the number of modes must be at least 1"));
    }
    if let Some(issue) = model.validate().into_iter().find(|issue| issue.severity == Severity::Error) {
        return Err(CaeError::new(issue.code, issue.message));
    }
    let has_node_mass = model.nodes.iter().any(|node| node.mass.is_finite() && node.mass > 0.0);
    if !positive_finite(model.material.density) && !has_node_mass {
        return Err(CaeError::new(
            "material/invalid-density",
            format!(
                "modal analysis requires a positive mass density or at least one positive nodal mass (density is {})",
                model.material.density
            ),
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
        let shape_values = system.to_global_vector(&shape_values);
        let mut participation_factor = [0.0_f64; 3];
        for node in 0..model.nodes.len() {
            for (component, value) in participation_factor.iter_mut().enumerate().take(translations) {
                if let Some(dof) = system.layout.index.get(node * system.layout.slots + component).copied().flatten() {
                    *value += mass[dof] * shape_values[dof];
                }
            }
        }
        let mut effective_mass = participation_factor;
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
            participation_factor,
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

/// Solves a response-spectrum analysis by modal superposition.
///
/// The model must have a positive density and be solvable by [`solve_modal`],
/// which supplies the mass-normalized mode shapes and signed participation
/// factors. For every mode the spectral acceleration `Sa(T)` is read from
/// `spectrum` at its natural period, converted into the spectral displacement
/// `Sd = Sa / ω²`, and scaled by the participation factor `Γ` in the requested
/// global direction (`0 = x`, `1 = y`, `2 = z`).
///
/// `combination` selects how the modal nodal contributions are combined.
/// [`SpectrumCombination::Srss`] takes the square root of the sum of the
/// squares of every component. [`SpectrumCombination::Cqc`] uses the complete
/// quadratic combination with the Der Kiureghian correlation coefficient
/// `ρ_ij = 8ζ²(1+r)r^(3/2) / ((1+r²)² + 4ζ²r(1+r)²)` for `r = ω_j/ω_i` and
/// `ρ_ij = 1` when `i = j`; the component value is
/// `sqrt(Σ_i Σ_j ρ_ij R_i R_j)` for the modal nodal contributions `R_i`.
/// CQC requires `damping`, one modal damping ratio in `(0, 1]`; SRSS ignores
/// it.
///
/// Spectral accelerations use the document length unit per second squared, for
/// example `mm/s²` with millimetre documents. An empty spectrum, a negative or
/// non-finite period, a non-finite acceleration, an unavailable direction or a
/// mode count of zero is rejected with a stable `analysis/...` code. An
/// unknown combination reports `spectrum/invalid-combination` and a missing or
/// out-of-range CQC damping ratio reports `spectrum/invalid-damping`.
pub fn solve_response_spectrum(
    model: &Model,
    modes: usize,
    axis: usize,
    spectrum: &[SpectrumPoint],
    combination: SpectrumCombination,
    damping: Option<f64>,
) -> Result<ResponseSpectrumSolution, CaeError> {
    let translations = if model.dimension == 3 { 3 } else { 2 };
    if axis >= translations {
        return Err(CaeError::new(
            "analysis/invalid-axis",
            format!(
                "response-spectrum axis {axis} is not available in a {}-dimensional model; use axis 0 or 1",
                model.dimension
            ),
        ));
    }
    if spectrum.is_empty() {
        return Err(CaeError::new("analysis/invalid-spectrum", "the response spectrum requires at least one point"));
    }
    for point in spectrum {
        if !point.period.is_finite() || point.period < 0.0 {
            return Err(CaeError::new(
                "analysis/invalid-spectrum",
                format!("spectrum period must be finite and non-negative (currently {})", point.period),
            ));
        }
        if !point.acceleration.is_finite() {
            return Err(CaeError::new(
                "analysis/invalid-spectrum",
                format!("spectrum acceleration must be finite (currently {})", point.acceleration),
            ));
        }
    }
    let damping = match combination {
        SpectrumCombination::Srss => None,
        SpectrumCombination::Cqc => {
            let Some(value) = damping
            else {
                return Err(CaeError::new("spectrum/invalid-damping", "the CQC combination requires a modal damping ratio"));
            };
            if !value.is_finite() || value <= 0.0 || value > 1.0 {
                return Err(CaeError::new(
                    "spectrum/invalid-damping",
                    format!("the CQC modal damping ratio must be finite and in (0, 1] (currently {value})"),
                ));
            }
            Some(value)
        }
    };
    let mut sorted = spectrum.to_vec();
    sorted.sort_by(|left, right| left.period.partial_cmp(&right.period).unwrap_or(std::cmp::Ordering::Equal));

    let modal = solve_modal(model, modes)?;
    let mut accumulated = vec![[0.0_f64; 4]; model.nodes.len()];
    let mut contributions: Vec<Vec<[f64; 4]>> = Vec::new();
    let mut frequencies: Vec<f64> = Vec::new();
    let mut result_modes = Vec::with_capacity(modal.modes.len());
    for mode in &modal.modes {
        if !positive_finite(mode.frequency_hz) || !positive_finite(mode.eigenvalue) {
            continue;
        }
        let period = 1.0 / mode.frequency_hz;
        let spectral_acceleration = interpolate_spectrum(&sorted, period);
        let spectral_displacement = spectral_acceleration / mode.eigenvalue;
        let participation_factor = mode.participation_factor[axis];
        let scale = participation_factor * spectral_displacement;
        let mut scaled = Vec::with_capacity(mode.shape.len());
        for (node, shape) in mode.shape.iter().enumerate() {
            let values = [scale * shape.ux, scale * shape.uy, scale * shape.uz, scale * shape.rz];
            accumulated[node][0] += values[0].powi(2);
            accumulated[node][1] += values[1].powi(2);
            accumulated[node][2] += values[2].powi(2);
            accumulated[node][3] += values[3].powi(2);
            scaled.push(values);
        }
        if damping.is_some() {
            contributions.push(scaled);
            frequencies.push(mode.eigenvalue.sqrt());
        }
        result_modes.push(SpectrumMode {
            frequency_hz: mode.frequency_hz,
            period,
            spectral_acceleration,
            participation_factor,
            spectral_displacement,
            scale,
        });
    }

    let combined: Vec<[f64; 4]> = match damping {
        Some(ratio) => combine_cqc(&frequencies, &contributions, ratio, model.nodes.len()),
        None => accumulated.into_iter().map(|values| values.map(f64::sqrt)).collect(),
    };
    let nodes: Vec<NodeDisplacement> = model
        .nodes
        .iter()
        .zip(&combined)
        .map(|(node, values)| {
            let ux = values[0];
            let uy = values[1];
            let uz = if model.dimension == 3 { values[2] } else { 0.0 };
            let rz = if model.dimension == 2 { values[3] } else { 0.0 };
            NodeDisplacement { id: node.id.clone(), ux, uy, uz, rz, magnitude: (ux * ux + uy * uy + uz * uz).sqrt() }
        })
        .collect();
    let max_displacement = nodes
        .iter()
        .max_by(|left, right| left.magnitude.partial_cmp(&right.magnitude).unwrap_or(std::cmp::Ordering::Equal))
        .cloned()
        .ok_or_else(|| CaeError::new("model/no-nodes", "the model requires at least 1 node"))?;

    Ok(ResponseSpectrumSolution {
        format: RESPONSE_SPECTRUM_REPORT_FORMAT.to_string(),
        version: RESPONSE_SPECTRUM_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        axis: ["x", "y", "z"][axis].to_string(),
        combination: combination.as_str().to_string(),
        damping,
        requested: modes,
        modes: result_modes,
        nodes,
        max_displacement,
    })
}

/// Der Kiureghian CQC correlation coefficient between two modes.
fn cqc_correlation(omega_i: f64, omega_j: f64, damping: f64) -> f64 {
    if omega_i == omega_j {
        return 1.0;
    }
    let r = omega_j / omega_i;
    let numerator = 8.0 * damping * damping * (1.0 + r) * r.powf(1.5);
    let denominator = (1.0 + r * r).powi(2) + 4.0 * damping * damping * r * (1.0 + r).powi(2);
    if denominator <= 0.0 { 0.0 } else { numerator / denominator }
}

/// Combines per-mode nodal contribution vectors component by component.
fn combine_cqc(frequencies: &[f64], contributions: &[Vec<[f64; 4]>], damping: f64, nodes: usize) -> Vec<[f64; 4]> {
    let mut combined = vec![[0.0_f64; 4]; nodes];
    for (node, values) in combined.iter_mut().enumerate() {
        for (component, value) in values.iter_mut().enumerate() {
            let mut sum = 0.0_f64;
            for (i, left) in contributions.iter().enumerate() {
                for (j, right) in contributions.iter().enumerate() {
                    let weight = if i == j { 1.0 } else { cqc_correlation(frequencies[i], frequencies[j], damping) };
                    sum += weight * left[node][component] * right[node][component];
                }
            }
            *value = if sum > 0.0 { sum.sqrt() } else { 0.0 };
        }
    }
    combined
}

/// Piecewise-linear spectral acceleration with constant end values.
fn interpolate_spectrum(points: &[SpectrumPoint], period: f64) -> f64 {
    let Some(first) = points.first()
    else {
        return 0.0;
    };
    if period <= first.period {
        return first.acceleration;
    }
    for window in points.windows(2) {
        let (left, right) = (window[0], window[1]);
        if period <= right.period {
            let span = right.period - left.period;
            if span <= 0.0 {
                return right.acceleration;
            }
            let fraction = (period - left.period) / span;
            return left.acceleration + fraction * (right.acceleration - left.acceleration);
        }
    }
    points.last().map(|point| point.acceleration).unwrap_or(0.0)
}

/// Solves the steady-state harmonic response of a model by modal
/// superposition.
///
/// The applied static load vector of the document (nodal forces, element
/// distributed loads, self-weight and thermal actions) is treated as a
/// harmonic excitation `f(t) = f·sin(ω·t)` in the requested global direction,
/// so the modal force of mode `r` is `p_r = φ_rᵀ·f` with the mass-normalized
/// shapes of [`solve_modal`]. The steady-state complex amplitude of mode `r` is
/// `p_r / (ω_r² − ω² + 2·i·ζ·ω_r·ω)`, and the nodal amplitudes are obtained by
/// summing the modes and taking the modulus.
///
/// The response is sampled at `points` equally spaced frequencies from
/// `start_hz` to `end_hz` in hertz. Every mode uses the same viscous damping
/// ratio `damping_ratio`, which must be strictly positive: zero damping makes
/// the amplitude unbounded at resonance, which this build does not report.
/// Peaks are read from the sampled frequencies, so a finer sweep resolves the
/// peak more accurately. Models with non-zero prescribed displacements or
/// inclined supports are rejected because the excitation vector and the modal
/// expansion do not share a single homogeneous frame.
pub fn solve_frequency_response(
    model: &Model,
    modes: usize,
    axis: usize,
    damping_ratio: f64,
    start_hz: f64,
    end_hz: f64,
    points: usize,
) -> Result<FrequencyResponseSolution, CaeError> {
    if modes == 0 {
        return Err(CaeError::new("analysis/invalid-modes", "the number of modes must be at least 1"));
    }
    if points < 2 {
        return Err(CaeError::new(
            "analysis/invalid-points",
            format!("the frequency sweep requires at least 2 points (currently {points})"),
        ));
    }
    if !start_hz.is_finite() || start_hz < 0.0 {
        return Err(CaeError::new(
            "analysis/invalid-range",
            format!("the start frequency must be finite and non-negative (currently {start_hz})"),
        ));
    }
    if !end_hz.is_finite() || end_hz <= start_hz {
        return Err(CaeError::new(
            "analysis/invalid-range",
            format!("the end frequency must be finite and greater than the start frequency (currently {start_hz}..{end_hz})"),
        ));
    }
    if !damping_ratio.is_finite() || damping_ratio <= 0.0 {
        return Err(CaeError::new(
            "analysis/invalid-damping",
            format!("the modal damping ratio must be finite and positive (currently {damping_ratio})"),
        ));
    }
    let translations = if model.dimension == 3 { 3 } else { 2 };
    if axis >= translations {
        return Err(CaeError::new(
            "analysis/invalid-axis",
            format!(
                "frequency-response axis {axis} is not available in a {}-dimensional model; use axis 0 or 1",
                model.dimension
            ),
        ));
    }
    if let Some(issue) = model.validate().into_iter().find(|issue| issue.severity == Severity::Error) {
        return Err(CaeError::new(issue.code, issue.message));
    }
    let has_prescribed = model.supports.iter().any(|support| {
        support
            .prescribed
            .as_ref()
            .is_some_and(|values| values.ux != 0.0 || values.uy != 0.0 || values.uz != 0.0 || values.rz != 0.0)
    });
    if has_prescribed {
        return Err(CaeError::new(
            "analysis/unsupported-boundary",
            "frequency-response analysis requires homogeneous supports; non-zero prescribed displacements are not supported",
        ));
    }
    let system = assemble(model)?;
    if system.rotations.iter().any(Option::is_some) {
        return Err(CaeError::new(
            "capability/not-implemented",
            "frequency-response analysis with inclined supports is not implemented",
        ));
    }
    let excitation = system.to_global_vector(&system.forces);
    let modal = solve_modal(model, modes)?;
    if modal.modes.is_empty() {
        return Err(CaeError::new(
            "modal/no-modes",
            "modal analysis returned no positive eigenvalue for the frequency response",
        ));
    }

    let size = system.layout.size;
    let mut modal_terms: Vec<(f64, f64, Vec<f64>)> = Vec::with_capacity(modal.modes.len());
    for mode in &modal.modes {
        if !positive_finite(mode.eigenvalue) {
            continue;
        }
        let mut shape = vec![0.0; size];
        for (node, entry) in mode.shape.iter().enumerate() {
            let base = node * system.layout.slots;
            if let Some(dof) = system.layout.index.get(base).copied().flatten() {
                shape[dof] = entry.ux;
            }
            if let Some(dof) = system.layout.index.get(base + 1).copied().flatten() {
                shape[dof] = entry.uy;
            }
            if model.dimension == 3 {
                if let Some(dof) = system.layout.index.get(base + 2).copied().flatten() {
                    shape[dof] = entry.uz;
                }
            }
            else if system.layout.uses_rotation {
                if let Some(dof) = system.layout.index.get(base + 2).copied().flatten() {
                    shape[dof] = entry.rz;
                }
            }
        }
        let modal_force: f64 = shape.iter().zip(excitation.iter()).map(|(shape, force)| shape * force).sum();
        modal_terms.push((mode.eigenvalue, modal_force, shape));
    }
    if modal_terms.is_empty() {
        return Err(CaeError::new(
            "modal/no-modes",
            "modal analysis returned no positive eigenvalue for the frequency response",
        ));
    }

    let step = (end_hz - start_hz) / (points - 1) as f64;
    let mut sweep = Vec::with_capacity(points);
    let mut node_peak = vec![(start_hz, 0.0_f64); model.nodes.len()];
    let mut peak = FrequencyResponsePoint { frequency_hz: start_hz, max_amplitude: 0.0, governing_node: String::new() };
    for sample in 0..points {
        let frequency_hz = start_hz + step * sample as f64;
        let angular = 2.0 * std::f64::consts::PI * frequency_hz;
        let mut real = vec![0.0_f64; size];
        let mut imaginary = vec![0.0_f64; size];
        for (eigenvalue, modal_force, shape) in &modal_terms {
            let natural = eigenvalue.sqrt();
            let real_denominator = eigenvalue - angular * angular;
            let imaginary_denominator = 2.0 * damping_ratio * natural * angular;
            let norm = real_denominator * real_denominator + imaginary_denominator * imaginary_denominator;
            if norm <= 0.0 {
                return Err(CaeError::new(
                    "analysis/invalid-damping",
                    format!("the steady-state denominator vanished at {frequency_hz} Hz"),
                ));
            }
            let scale = modal_force / norm;
            for (dof, value) in shape.iter().enumerate() {
                if *value == 0.0 {
                    continue;
                }
                real[dof] += scale * real_denominator * value;
                imaginary[dof] -= scale * imaginary_denominator * value;
            }
        }
        let mut governing = String::new();
        let mut max_amplitude = 0.0_f64;
        for (node, entry) in model.nodes.iter().enumerate() {
            let base = node * system.layout.slots;
            let mut squares = 0.0;
            for component in 0..translations {
                if let Some(dof) = system.layout.index.get(base + component).copied().flatten() {
                    squares += real[dof] * real[dof] + imaginary[dof] * imaginary[dof];
                }
            }
            let magnitude = squares.sqrt();
            if !magnitude.is_finite() {
                return Err(CaeError::new(
                    "analysis/non-finite-response",
                    format!("the steady-state amplitude of node `{}` is not finite at {frequency_hz} Hz", entry.id),
                ));
            }
            if magnitude > node_peak[node].1 {
                node_peak[node] = (frequency_hz, magnitude);
            }
            if magnitude > max_amplitude {
                max_amplitude = magnitude;
                governing = entry.id.clone();
            }
        }
        if max_amplitude > peak.max_amplitude {
            peak = FrequencyResponsePoint { frequency_hz, max_amplitude, governing_node: governing.clone() };
        }
        sweep.push(FrequencyResponsePoint { frequency_hz, max_amplitude, governing_node: governing });
    }

    let nodes = model
        .nodes
        .iter()
        .zip(node_peak)
        .map(|(node, (peak_frequency_hz, max_amplitude))| NodeFrequencyResponse {
            node: node.id.clone(),
            peak_frequency_hz,
            max_amplitude,
        })
        .collect();
    Ok(FrequencyResponseSolution {
        format: FREQUENCY_RESPONSE_REPORT_FORMAT.to_string(),
        version: FREQUENCY_RESPONSE_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        axis: ["x", "y", "z"][axis].to_string(),
        requested: modes,
        damping_ratio,
        start_hz,
        end_hz,
        points,
        sweep,
        peak,
        nodes,
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
                    let Some(row_dof) = row_dof
                    else {
                        continue;
                    };
                    for (column, &column_dof) in element.displacement_dofs.iter().enumerate() {
                        let Some(column_dof) = column_dof
                        else {
                            continue;
                        };
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
                        let row_dof = element.displacement_dofs[row].expect("truss degree of freedom");
                        let column_dof = element.displacement_dofs[column].expect("truss degree of freedom");
                        geometric[row_dof * size + column_dof] += scale * sign * projector;
                    }
                }
            }
        }
    }

    let geometric = system.to_local_matrix(&geometric);
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
        let mut shape_values = system.to_global_vector(&shape_values);
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
        let scaled = combination_model(model, combination)?;
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
                    let (load_force, load_moment) = beam_load_resultants(q1, slope, beam.partial.as_ref(), length, position);
                    let shear = result.end_forces.v1 + load_force;
                    let moment = -result.end_forces.m1 + result.end_forces.v1 * position + position * load_force - load_moment;
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

/// Samples the deflected centreline of every element at equally spaced
/// stations.
///
/// For beams the transverse deflection and rotation are recovered exactly from
/// the nodal solution: the cubic Hermite interpolation of the end
/// displacements and rotations is completed by the fixed-end particular
/// solution of the member's linearly varying transverse load,
/// `EI·v'''' = q(x)`. The axial displacement adds the quadratic term of a
/// uniform axial (self-weight) load to the linear interpolation. Released ends
/// use the recovered internal hinge rotation, so a hinged member keeps its
/// physical shape. Truss elements are sampled as straight members with a linear
/// axial displacement and no transverse deflection.
///
/// Each station reports the local axial and transverse displacement, the
/// section rotation and the deformed global coordinates, so the result can be
/// plotted directly. `stations` counts both ends and must be at least `2`.
pub fn solve_deflection(model: &Model, stations: usize) -> Result<DeflectionSolution, CaeError> {
    if stations < 2 {
        return Err(CaeError::new(
            "analysis/invalid-stations",
            format!("deflection sampling requires at least 2 stations, but {stations} were requested"),
        ));
    }
    let solution = solve(model)?;
    let system = assemble(model)?;
    let mut elements = Vec::with_capacity(system.elements.len());
    let mut governing: Option<DeflectionGoverning> = None;

    for (index, (element, result)) in system.elements.iter().zip(&solution.elements).enumerate() {
        let start = &model.nodes[element.from];
        let end = &model.nodes[element.to];
        let delta = [end.x - start.x, end.y - start.y, end.z - start.z];
        let length = element.length;
        let c = delta[0] / length;
        let s = delta[1] / length;
        let tz = delta[2] / length;
        let from = &solution.nodes[element.from];
        let to = &solution.nodes[element.to];

        let (u1, v1, theta1, u2, v2, theta2) = match &element.model {
            ElementModel::Truss { direction, components, .. } => {
                let axial = |node: &NodeDisplacement| {
                    let mut value = 0.0;
                    for (component, cosine) in direction.iter().enumerate().take(*components) {
                        let displacement = match component {
                            0 => node.ux,
                            1 => node.uy,
                            _ => node.uz,
                        };
                        value += displacement * cosine;
                    }
                    value
                };
                (axial(from), 0.0, 0.0, axial(to), 0.0, 0.0)
            }
            ElementModel::Beam(beam) => {
                let global = [from.ux, from.uy, from.rz, to.ux, to.uy, to.rz];
                let mut local = [0.0; 6];
                for (row, value) in local.iter_mut().enumerate() {
                    *value = (0..6).map(|column| beam.transform[row][column] * global[column]).sum();
                }
                restore_released_rotations(beam, &mut local);
                (local[0], local[1], local[2], local[3], local[4], local[5])
            }
        };

        let mut flexural_rigidity = 0.0;
        let mut load_start = 0.0;
        let mut load_end = 0.0;
        let mut axial_uniform = 0.0;
        if let ElementModel::Beam(beam) = &element.model {
            let inertia = resolve_inertia(model, &model.elements[index]).ok_or_else(|| {
                CaeError::new(
                    "element/invalid-inertia",
                    format!("beam element `{}` requires a positive finite inertia", element.id),
                )
            })?;
            flexural_rigidity = element.youngs_modulus * inertia;
            load_start = beam.transverse_start;
            load_end = beam.transverse_end;
            axial_uniform = beam.axial_uniform;
        }
        let load_slope = (load_end - load_start) / length;
        let quartic = load_start / (24.0 * flexural_rigidity.max(f64::MIN_POSITIVE));
        let quintic = load_slope / (120.0 * flexural_rigidity.max(f64::MIN_POSITIVE));
        let fixed_cubic = -length * (2.0 * quartic + 3.0 * quintic * length);
        let fixed_quadratic = length * length * (quartic + 2.0 * quintic * length);
        // A partial load needs its own fixed-end correction: the particular
        // solution is cancelled at both ends and both end rotations with a
        // cubic/quadratic correction so it can be superposed on the full-span
        // particular solution.
        let partial_correction = match &element.model {
            ElementModel::Beam(beam) => beam.partial.map(|partial| {
                let rigidity = flexural_rigidity.max(f64::MIN_POSITIVE);
                let (cubic_end, quadratic_end) = partial_deflection_integrals(&partial, length, length);
                let end_deflection = cubic_end / (6.0 * rigidity);
                let end_rotation = quadratic_end / (2.0 * rigidity);
                let cubic = (2.0 * end_deflection / length - end_rotation) / (length * length);
                let quadratic = (end_rotation * length - 3.0 * end_deflection) / (length * length);
                (partial, rigidity, cubic, quadratic)
            }),
            ElementModel::Truss { .. } => None,
        };

        let mut samples = Vec::with_capacity(stations);
        let mut maximum = 0.0_f64;
        let mut maximum_signed = 0.0;
        let mut maximum_position = 0.0;
        for sample in 0..stations {
            let position = length * sample as f64 / (stations - 1) as f64;
            let xi = position / length;
            let xi2 = xi * xi;
            let xi3 = xi2 * xi;
            let n1 = 1.0 - 3.0 * xi2 + 2.0 * xi3;
            let n2 = length * (xi - 2.0 * xi2 + xi3);
            let n3 = 3.0 * xi2 - 2.0 * xi3;
            let n4 = length * (-xi2 + xi3);
            let d1 = (-6.0 * xi + 6.0 * xi2) / length;
            let d2 = 1.0 - 4.0 * xi + 3.0 * xi2;
            let d3 = (6.0 * xi - 6.0 * xi2) / length;
            let d4 = -2.0 * xi + 3.0 * xi2;
            let power3 = position * position * position;
            let power4 = power3 * position;
            let mut particular =
                quartic * power4 + quintic * power4 * position + fixed_cubic * power3 + fixed_quadratic * position * position;
            let mut particular_slope = 4.0 * quartic * power3
                + 5.0 * quintic * power4
                + 3.0 * fixed_cubic * position * position
                + 2.0 * fixed_quadratic * position;
            if let Some((partial, rigidity, cubic, quadratic)) = partial_correction {
                let (cubic_integral, quadratic_integral) = partial_deflection_integrals(&partial, length, position);
                particular += cubic_integral / (6.0 * rigidity) + cubic * power3 + quadratic * position * position;
                particular_slope +=
                    quadratic_integral / (2.0 * rigidity) + 3.0 * cubic * position * position + 2.0 * quadratic * position;
            }
            let transverse = n1 * v1 + n2 * theta1 + n3 * v2 + n4 * theta2 + particular;
            let rotation = d1 * v1 + d2 * theta1 + d3 * v2 + d4 * theta2 + particular_slope;
            let axial_particular = if element.youngs_modulus > 0.0 && element.area > 0.0 {
                axial_uniform * position * (length - position) / (2.0 * element.youngs_modulus * element.area)
            }
            else {
                0.0
            };
            let axial = u1 + (u2 - u1) * xi + axial_particular;
            let normal_x = -s;
            let normal_y = c;
            samples.push(DeflectionStation {
                position,
                axial,
                transverse,
                rotation,
                x: start.x + (position + axial) * c + transverse * normal_x,
                y: start.y + (position + axial) * s + transverse * normal_y,
                z: start.z + (position + axial) * tz,
            });
            if transverse.abs() > maximum {
                maximum = transverse.abs();
                maximum_signed = transverse;
                maximum_position = position;
            }
        }

        if governing.as_ref().is_none_or(|current| maximum > current.transverse.abs()) {
            governing = Some(DeflectionGoverning {
                element: element.id.clone(),
                position: maximum_position,
                transverse: maximum_signed,
            });
        }

        elements.push(ElementDeflection {
            id: element.id.clone(),
            from: result.from.clone(),
            to: result.to.clone(),
            kind: element.kind,
            length,
            max_abs_transverse: maximum,
            stations: samples,
        });
    }

    let governing = governing.ok_or_else(|| CaeError::new("model/no-elements", "the model requires at least 1 element"))?;
    Ok(DeflectionSolution {
        format: DEFLECTION_REPORT_FORMAT.to_string(),
        version: DEFLECTION_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        stations,
        governing,
        elements,
    })
}

struct NodeStressAccumulator {
    ends: usize,
    axial_sum: f64,
    min_stress: f64,
    max_stress: f64,
    max_abs_stress: f64,
    governing_element: String,
}

/// Averages element-end stresses at shared nodes and reports the extremes.
///
/// Each connected element contributes the stress state of its end station
/// (the `from` end to the `from` node and the `to` end to the `to` node). A
/// node therefore receives the arithmetic mean of the axial stresses and the
/// minimum/maximum extreme-fibre stresses of all connected ends, together with
/// the element that produces the largest absolute value. This turns a
/// per-element sampled field into a compact nodal summary for screening and
/// for comparing against allowable stress.
///
/// Beam elements require a parametric material section, exactly as
/// [`solve_stress`] does. `stations` counts both ends and must be at least `2`.
pub fn solve_nodal_stress(model: &Model, stations: usize) -> Result<NodalStressSolution, CaeError> {
    let stress = solve_stress(model, stations)?;
    let positions: HashMap<&str, usize> =
        model.nodes.iter().enumerate().map(|(position, node)| (node.id.as_str(), position)).collect();
    let mut accumulators: Vec<Option<NodeStressAccumulator>> = (0..model.nodes.len()).map(|_| None).collect();

    for (element, result) in model.elements.iter().zip(&stress.elements) {
        let Some(first) = result.stations.first()
        else {
            continue;
        };
        let Some(last) = result.stations.last()
        else {
            continue;
        };
        for (node_id, station) in [(&element.from, first), (&element.to, last)] {
            let Some(&position) = positions.get(node_id.as_str())
            else {
                continue;
            };
            let accumulator = accumulators[position].get_or_insert_with(|| NodeStressAccumulator {
                ends: 0,
                axial_sum: 0.0,
                min_stress: f64::INFINITY,
                max_stress: f64::NEG_INFINITY,
                max_abs_stress: 0.0,
                governing_element: String::new(),
            });
            accumulator.ends += 1;
            accumulator.axial_sum += station.axial_stress;
            accumulator.min_stress = accumulator.min_stress.min(station.min_stress);
            accumulator.max_stress = accumulator.max_stress.max(station.max_stress);
            let absolute = station.min_stress.abs().max(station.max_stress.abs());
            if absolute > accumulator.max_abs_stress || accumulator.governing_element.is_empty() {
                accumulator.max_abs_stress = absolute;
                accumulator.governing_element = element.id.clone();
            }
        }
    }

    let nodes: Vec<NodeStress> = model
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(position, node)| {
            accumulators[position].as_ref().map(|accumulator| NodeStress {
                node: node.id.clone(),
                connected_elements: accumulator.ends,
                mean_axial_stress: accumulator.axial_sum / accumulator.ends as f64,
                min_stress: accumulator.min_stress,
                max_stress: accumulator.max_stress,
                max_abs_stress: accumulator.max_abs_stress,
                governing_element: accumulator.governing_element.clone(),
            })
        })
        .collect();

    let mut governing: Option<&NodeStress> = None;
    for node in &nodes {
        if governing.is_none_or(|current| node.max_abs_stress > current.max_abs_stress) {
            governing = Some(node);
        }
    }
    let governing = governing.ok_or_else(|| CaeError::new("model/no-elements", "the model requires at least 1 element"))?;
    let allowable = stress.allowable_stress;
    Ok(NodalStressSolution {
        format: NODAL_STRESS_REPORT_FORMAT.to_string(),
        version: NODAL_STRESS_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        stations,
        allowable_stress: allowable,
        governing: NodalStressGoverning {
            node: governing.node.clone(),
            element: governing.governing_element.clone(),
            max_abs_stress: governing.max_abs_stress,
            utilization: allowable.map(|allowable| governing.max_abs_stress / allowable),
        },
        nodes,
    })
}

fn load_case_row(name: &str, loads: f64, gravity: f64, solution: &Solution) -> LoadCaseResult {
    LoadCaseResult {
        name: name.to_string(),
        loads,
        gravity,
        applied: solution.reaction_summary.applied,
        max_displacement: solution.max_displacement.magnitude,
        max_displacement_node: solution.max_displacement.id.clone(),
        max_axial_force: solution.max_axial_force.axial_force,
        max_axial_element: solution.max_axial_force.element.clone(),
        max_abs_moment: solution.max_moment.moment.abs(),
        max_moment_element: solution.max_moment.element.clone(),
        strain_energy: solution.strain_energy,
        balanced: solution.reaction_summary.balanced,
    }
}

/// Builds a compact summary table of the unfactored static case and every
/// stored load combination.
///
/// The unfactored solve is always the first row and is called `base` with
/// factors `1/1`. Each combination is solved on the model scaled by its `loads`
/// and `gravity` factors, exactly like [`solve_combinations`], and contributes
/// one row with the applied resultant, the largest displacement and its node,
/// the extreme axial force and bending moment with their elements, the strain
/// energy and the reaction balance flag. The table is intended for reports and
/// quick screening; per-element diagrams come from [`solve_internal_forces`].
pub fn summarize_load_cases(model: &Model) -> Result<LoadCaseSummary, CaeError> {
    let base = solve(model)?;
    let mut cases = vec![load_case_row("base", 1.0, 1.0, &base)];
    for combination in &model.combinations {
        if combination.name.trim().is_empty() {
            return Err(CaeError::new("combination/invalid-name", "a load combination requires a non-empty name"));
        }
        if !combination.loads.is_finite() || !combination.gravity.is_finite() {
            return Err(CaeError::new(
                "combination/invalid-factor",
                format!("load combination `{}` requires finite loads and gravity factors", combination.name),
            ));
        }
        let scaled = combination_model(model, combination)?;
        let solution = solve(&scaled)?;
        cases.push(load_case_row(&combination.name, combination.loads, combination.gravity, &solution));
    }
    Ok(LoadCaseSummary {
        format: LOAD_CASE_REPORT_FORMAT.to_string(),
        version: LOAD_CASE_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        cases,
    })
}

/// Solves a sequence of construction stages by element activation and
/// deactivation.
///
/// Each stage keeps only the elements listed in [`ConstructionStage::elements`]
/// (all elements when the list is omitted), scales the applied loads and gravity
/// by the stage factors exactly like a load combination, and performs one
/// linear static solve of that active submodel. The stages are solved in the
/// requested order and every row reports the applied resultant, the largest
/// displacement and its node, the extreme axial force and bending moment with
/// their elements, the strain energy and the reaction balance.
///
/// This is a stage-by-stage linear analysis of the active topology: internal
/// forces are not locked into the members that carry them into later stages, so
/// a true creep/shake-down style staged construction with locked-in stress is
/// outside this build. The summary is intended for erection sequencing checks
/// and deactivation screens, not for a multi-step stress-history analysis.
pub fn solve_construction_stages(model: &Model, stages: &[ConstructionStage]) -> Result<ConstructionStageSolution, CaeError> {
    if stages.is_empty() {
        return Err(CaeError::new("stage/none", "no construction stages are defined; add at least one stage before solving"));
    }
    let mut names = std::collections::HashSet::new();
    let mut rows = Vec::with_capacity(stages.len());
    for stage in stages {
        if stage.name.trim().is_empty() {
            return Err(CaeError::new("stage/invalid-name", "a construction stage requires a non-empty name"));
        }
        if !names.insert(stage.name.as_str()) {
            return Err(CaeError::new(
                "stage/duplicate-name",
                format!("construction stage `{}` is defined more than once", stage.name),
            ));
        }
        if !stage.loads.is_finite() || !stage.gravity.is_finite() {
            return Err(CaeError::new(
                "stage/invalid-factor",
                format!("construction stage `{}` requires finite loads and gravity factors", stage.name),
            ));
        }
        let mut active_model = model.clone();
        if let Some(ids) = &stage.elements {
            if ids.is_empty() {
                return Err(CaeError::new(
                    "stage/no-elements",
                    format!("construction stage `{}` activates no element; omit `elements` to keep every element", stage.name),
                ));
            }
            let mut active = std::collections::HashSet::with_capacity(ids.len());
            for id in ids {
                if !active_model.elements.iter().any(|element| &element.id == id) {
                    return Err(CaeError::new(
                        "stage/unknown-element",
                        format!("construction stage `{}` references unknown element `{id}`", stage.name),
                    ));
                }
                active.insert(id.as_str());
            }
            active_model.elements.retain(|element| active.contains(element.id.as_str()));
        }
        let active_elements = active_model.elements.len();
        let scaled = scaled_model(&active_model, stage.loads, stage.gravity);
        let solution = solve(&scaled)?;
        rows.push(ConstructionStageResult {
            name: stage.name.clone(),
            active_elements,
            loads: stage.loads,
            gravity: stage.gravity,
            applied: solution.reaction_summary.applied,
            max_displacement: solution.max_displacement.magnitude,
            max_displacement_node: solution.max_displacement.id.clone(),
            max_axial_force: solution.max_axial_force.axial_force,
            max_axial_element: solution.max_axial_force.element.clone(),
            max_abs_moment: solution.max_moment.moment.abs(),
            max_moment_element: solution.max_moment.element.clone(),
            strain_energy: solution.strain_energy,
            balanced: solution.reaction_summary.balanced,
        });
    }
    Ok(ConstructionStageSolution {
        format: CONSTRUCTION_STAGE_REPORT_FORMAT.to_string(),
        version: CONSTRUCTION_STAGE_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        stages: rows,
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
        let scaled = combination_model(model, combination)?;
        let solution = solve(&scaled)?;
        cases.push(CombinationCase {
            name: combination.name.clone(),
            loads: combination.loads,
            gravity: combination.gravity,
            patterns: combination.patterns.clone(),
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

struct NonlinearBar {
    from: usize,
    to: usize,
    length: f64,
    axial_stiffness: f64,
}

/// Current configuration of one large-displacement truss bar.
struct NonlinearBarState {
    direction: [f64; 3],
    length: f64,
    axial_force: f64,
}

fn nonlinear_bar_state(
    model: &Model,
    bar: &NonlinearBar,
    layout: &Layout,
    displacement: &[f64],
    components: usize,
) -> Result<NonlinearBarState, CaeError> {
    let start = &model.nodes[bar.from];
    let end = &model.nodes[bar.to];
    let mut delta = [0.0_f64; 3];
    for component in 0..components {
        let start_displacement = node_value(layout, displacement, bar.from, component);
        let end_displacement = node_value(layout, displacement, bar.to, component);
        let origin = [start.x, start.y, start.z][component];
        let target = [end.x, end.y, end.z][component];
        delta[component] = (target + end_displacement) - (origin + start_displacement);
    }
    let length = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
    if !positive_finite(length) {
        return Err(CaeError::new(
            "analysis/degenerate-element",
            format!(
                "the bar between nodes `{}` and `{}` collapses to zero length during the nonlinear iteration",
                model.nodes[bar.from].id, model.nodes[bar.to].id
            ),
        ));
    }
    let direction = [delta[0] / length, delta[1] / length, delta[2] / length];
    // Corotational small-strain bar: the axial force follows the current chord
    // length, so the only nonlinearity is the change of geometry.
    let axial_force = bar.axial_stiffness * (length - bar.length);
    Ok(NonlinearBarState { direction, length, axial_force })
}

fn nonlinear_internal_forces(
    model: &Model,
    bars: &[NonlinearBar],
    layout: &Layout,
    displacement: &[f64],
    components: usize,
) -> Result<Vec<f64>, CaeError> {
    let mut internal = vec![0.0_f64; layout.size];
    for bar in bars {
        let state = nonlinear_bar_state(model, bar, layout, displacement, components)?;
        for component in 0..components {
            let value = state.axial_force * state.direction[component];
            if let Some(dof) = layout.index.get(bar.from * layout.slots + component).copied().flatten() {
                internal[dof] -= value;
            }
            if let Some(dof) = layout.index.get(bar.to * layout.slots + component).copied().flatten() {
                internal[dof] += value;
            }
        }
    }
    Ok(internal)
}

fn nonlinear_tangent(
    model: &Model,
    bars: &[NonlinearBar],
    layout: &Layout,
    displacement: &[f64],
    components: usize,
) -> Result<Vec<f64>, CaeError> {
    let size = layout.size;
    let mut tangent = vec![0.0_f64; size * size];
    for bar in bars {
        let state = nonlinear_bar_state(model, bar, layout, displacement, components)?;
        let geometric = state.axial_force / state.length;
        let nodes = [bar.from, bar.to];
        let signs = [1.0_f64, -1.0_f64];
        for row in 0..2 {
            for column in 0..2 {
                for i in 0..components {
                    let Some(row_dof) = layout.index.get(nodes[row] * layout.slots + i).copied().flatten()
                    else {
                        continue;
                    };
                    for j in 0..components {
                        let Some(column_dof) = layout.index.get(nodes[column] * layout.slots + j).copied().flatten()
                        else {
                            continue;
                        };
                        let value = bar.axial_stiffness * state.direction[i] * state.direction[j]
                            + if i == j { geometric } else { 0.0 }
                            - geometric * state.direction[i] * state.direction[j];
                        tangent[row_dof * size + column_dof] += signs[row] * signs[column] * value;
                    }
                }
            }
        }
    }
    Ok(tangent)
}

fn vector_norm(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}

/// Solves a large-displacement (geometric nonlinear) truss model.
///
/// Only truss elements are supported: the corotational formulation updates the
/// chord direction and length of every bar from the current nodal positions and
/// derives the axial force `N = E·A·(L − L₀)/L₀`, so the analysis follows large
/// rotations and the stiffening/softening of a shallow truss. Bending, material
/// plasticity and member buckling are outside this formulation, and a model
/// that contains a beam element is rejected with `capability/not-implemented`.
///
/// The applied load and self-weight are split into `increments` equal load
/// steps. Every step runs a Newton iteration with the exact corotational tangent
/// `K = E·A/L₀·n·nᵀ + N/L·(I − n·nᵀ)` until the free residual norm is below
/// `tolerance × max(1, |λ·f|)`. The report contains the total iteration count,
/// the final residual norm and the nodal displacements, which makes the
/// convergence evidence part of the result instead of hidden state.
pub fn solve_nonlinear(
    model: &Model,
    increments: usize,
    max_iterations: usize,
    tolerance: f64,
) -> Result<NonlinearSolution, CaeError> {
    if increments == 0 {
        return Err(CaeError::new("analysis/invalid-increments", "the nonlinear load increments must be at least 1"));
    }
    if max_iterations == 0 {
        return Err(CaeError::new("analysis/invalid-iterations", "the nonlinear iteration limit must be at least 1"));
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(CaeError::new(
            "analysis/invalid-tolerance",
            format!("the nonlinear convergence tolerance must be finite and positive (currently {tolerance})"),
        ));
    }
    if model.elements.iter().any(|element| element.kind == ElementKind::Beam) {
        return Err(CaeError::new(
            "capability/not-implemented",
            "geometric nonlinear analysis of beam elements is not implemented; only truss models are supported",
        ));
    }
    if let Some(issue) = model.validate().into_iter().find(|issue| issue.severity == Severity::Error) {
        return Err(CaeError::new(issue.code, issue.message));
    }
    let system = assemble(model)?;
    if system.rotations.iter().any(Option::is_some) {
        return Err(CaeError::new(
            "capability/not-implemented",
            "geometric nonlinear analysis with inclined supports is not implemented",
        ));
    }
    let size = system.layout.size;
    let free: Vec<usize> = (0..size).filter(|&dof| !constrained(&system, dof)).collect();
    if free.is_empty() {
        return Err(CaeError::new("model/mechanism", "no free degrees of freedom available for nonlinear analysis"));
    }
    let components = if model.dimension == 3 { 3 } else { 2 };
    let bars: Vec<NonlinearBar> = system
        .elements
        .iter()
        .map(|element| NonlinearBar {
            from: element.from,
            to: element.to,
            length: element.length,
            axial_stiffness: element.youngs_modulus * element.area / element.length,
        })
        .collect();
    if bars.is_empty() {
        return Err(CaeError::new("model/no-elements", "the model requires at least 1 element"));
    }
    let external: Vec<f64> = free.iter().map(|&dof| system.forces[dof]).collect();
    let external_norm = vector_norm(&external);
    let mut displacement = system.prescribed.clone();
    let mut iterations = 0;
    let mut residual_norm = 0.0;

    for step in 1..=increments {
        let factor = step as f64 / increments as f64;
        let mut converged = false;
        for _ in 0..max_iterations {
            let internal = nonlinear_internal_forces(model, &bars, &system.layout, &displacement, components)?;
            let residual: Vec<f64> =
                free.iter().enumerate().map(|(index, &dof)| factor * external[index] - internal[dof]).collect();
            residual_norm = vector_norm(&residual);
            let scale = 1.0_f64.max(factor * external_norm);
            if residual_norm <= tolerance * scale {
                converged = true;
                break;
            }
            iterations += 1;
            let tangent = nonlinear_tangent(model, &bars, &system.layout, &displacement, components)?;
            let count = free.len();
            let mut reduced = vec![0.0_f64; count * count];
            for (row, &row_dof) in free.iter().enumerate() {
                for (column, &column_dof) in free.iter().enumerate() {
                    reduced[row * count + column] = tangent[row_dof * size + column_dof];
                }
            }
            let delta = solve_dense(reduced, residual, count).map_err(|()| {
                CaeError::new(
                    "analysis/singular-tangent",
                    format!(
                        "the nonlinear tangent is singular at load step {step}/{increments}; the configuration is unstable"
                    ),
                )
            })?;
            for (index, &dof) in free.iter().enumerate() {
                displacement[dof] += delta[index];
            }
            if displacement.iter().any(|value| !value.is_finite()) {
                return Err(CaeError::new(
                    "analysis/non-finite-displacement",
                    format!("the nonlinear iteration produced non-finite displacements at load step {step}/{increments}"),
                ));
            }
        }
        if !converged {
            return Err(CaeError::new(
                "analysis/not-converged",
                format!(
                    "geometric nonlinear analysis did not converge after {max_iterations} iterations at load step {step}/{increments} (residual {residual_norm:.6e}, tolerance {tolerance:.6e})"
                ),
            ));
        }
    }

    let nodes: Vec<NodeDisplacement> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let ux = node_value(&system.layout, &displacement, index, 0);
            let uy = node_value(&system.layout, &displacement, index, 1);
            let uz = if model.dimension == 3 { node_value(&system.layout, &displacement, index, 2) } else { 0.0 };
            NodeDisplacement { id: node.id.clone(), ux, uy, uz, rz: 0.0, magnitude: (ux * ux + uy * uy + uz * uz).sqrt() }
        })
        .collect();
    let max_displacement = nodes
        .iter()
        .max_by(|left, right| left.magnitude.partial_cmp(&right.magnitude).unwrap_or(std::cmp::Ordering::Equal))
        .cloned()
        .ok_or_else(|| CaeError::new("model/no-nodes", "the model requires at least 1 node"))?;

    Ok(NonlinearSolution {
        format: NONLINEAR_REPORT_FORMAT.to_string(),
        version: NONLINEAR_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        increments,
        iterations,
        tolerance,
        residual_norm,
        nodes,
        max_displacement,
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

/// Builds the model solved for one load combination.
///
/// The applied load case and gravity are scaled by the combination factors
/// exactly like [`scaled_model`], then every referenced load pattern adds
/// its own nodal loads multiplied by its factor. An unknown pattern name is
/// rejected with `combination/unknown-pattern` and a non-finite factor with
/// `combination/invalid-factor`.
fn combination_model(model: &Model, combination: &LoadCombination) -> Result<Model, CaeError> {
    let mut scaled = scaled_model(model, combination.loads, combination.gravity);
    for entry in &combination.patterns {
        if !entry.factor.is_finite() {
            return Err(CaeError::new(
                "combination/invalid-factor",
                format!("pattern `{}` of load combination `{}` requires a finite factor", entry.pattern, combination.name),
            ));
        }
        let pattern = model.patterns.iter().find(|pattern| pattern.name == entry.pattern).ok_or_else(|| {
            CaeError::new(
                "combination/unknown-pattern",
                format!("load combination `{}` references unknown load pattern `{}`", combination.name, entry.pattern),
            )
        })?;
        for load in &pattern.loads {
            scaled.loads.push(Load {
                node: load.node.clone(),
                fx: load.fx * entry.factor,
                fy: load.fy * entry.factor,
                fz: load.fz * entry.factor,
                mz: load.mz * entry.factor,
            });
        }
    }
    Ok(scaled)
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
            !model.supports.iter().any(|support| {
                support.node == node.id && (support.dx || support.dy || support.dz || support.rz || support.direction.is_some())
            })
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
    let beams = rotation_nodes(model);
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
    assemble_with_active(model, None)
}

/// Assembles the system with an optional element activity mask.
///
/// An inactive element contributes no stiffness, no self-weight and no thermal
/// load but is still reported with zero forces. Only the tension-only iteration
/// passes a mask; every other caller keeps every element active.
fn assemble_with_active(model: &Model, active_elements: Option<&[bool]>) -> Result<System, CaeError> {
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

    for (element_index, element) in model.elements.iter().enumerate() {
        let element_active = active_elements.is_none_or(|mask| mask[element_index]);
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
                let user_start = element.distributed_load.unwrap_or(0.0);
                let user_end = element.distributed_load_end.unwrap_or(user_start);
                let mut load_start = 0.0;
                let mut load_end = 0.0;
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
                let partial = match (element.distributed_load_from, element.distributed_load_to) {
                    (Some(start), Some(end)) => {
                        if !start.is_finite() || !end.is_finite() || start < 0.0 || end > 1.0 || start >= end {
                            return Err(CaeError::new(
                                "element/invalid-load-span",
                                format!(
                                    "load span of element `{}` must satisfy 0 <= start < end <= 1 (currently {start}..{end})",
                                    element.id
                                ),
                            ));
                        }
                        let partial = PartialLoad { start, end, load_start: user_start, load_end: user_end };
                        add_partial_transverse(&mut local_load, partial, length);
                        Some(partial)
                    }
                    (None, None) => {
                        load_start += user_start;
                        load_end += user_end;
                        None
                    }
                    _ => {
                        return Err(CaeError::new(
                            "element/invalid-load-span",
                            format!(
                                "element `{}` must declare both the start and the end fraction of a distributed load span",
                                element.id
                            ),
                        ));
                    }
                };
                if load_start != 0.0 || load_end != 0.0 {
                    add_linear_transverse(&mut local_load, load_start, load_end, length);
                }
                // Uniform thermal expansion enters as the consistent axial
                // load `E·A·alpha·ΔT·[-1, +1]` in local coordinates.
                if thermal != 0.0 {
                    local_load[0] -= thermal;
                    local_load[3] += thermal;
                }
                // A through-depth gradient produces the free curvature
                // `alpha·ΔT/h`; restraining it needs the equal and opposite end
                // moments `E·I·alpha·ΔT/h`.
                if let Some(gradient) = element.temperature_gradient {
                    if !gradient.is_finite() {
                        return Err(CaeError::new(
                            "element/invalid-temperature-gradient",
                            format!("temperature gradient of element `{}` must be finite", element.id),
                        ));
                    }
                    let alpha = model.material.alpha.filter(|value| value.is_finite()).ok_or_else(|| {
                        CaeError::new(
                            "material/missing-alpha",
                            format!(
                                "element `{}` declares a temperature gradient but the material does not define a finite coefficient of thermal expansion (alpha)",
                                element.id
                            ),
                        )
                    })?;
                    let depth = model
                        .material
                        .section
                        .and_then(|section| section.properties().ok())
                        .map(|properties| 2.0 * properties.extreme_fiber)
                        .ok_or_else(|| {
                            CaeError::new(
                                "material/missing-section",
                                format!(
                                    "element `{}` declares a temperature gradient but the material does not define a section depth",
                                    element.id
                                ),
                            )
                        })?;
                    let moment = youngs_modulus * inertia * alpha * gradient / depth;
                    local_load[2] += moment;
                    local_load[5] -= moment;
                }
                let local_stiffness = beam_stiffness(youngs_modulus, area, inertia, length);
                let transform = beam_transform(c, s);
                let release = element.release.unwrap_or_default();
                let released = [false, false, release.start, false, false, release.end];
                let (condensed_stiffness, condensed_load) = condense_releases(&local_stiffness, &local_load, released);
                let displacement_dofs = vec![
                    Some(layout.dof(from * layout.slots)?),
                    Some(layout.dof(from * layout.slots + 1)?),
                    if release.start { None } else { Some(layout.dof(from * layout.slots + 2)?) },
                    Some(layout.dof(to * layout.slots)?),
                    Some(layout.dof(to * layout.slots + 1)?),
                    if release.end { None } else { Some(layout.dof(to * layout.slots + 2)?) },
                ];
                let mut global_stiffness = [[0.0_f64; 6]; 6];
                for (row, row_values) in global_stiffness.iter_mut().enumerate() {
                    for (column, value) in row_values.iter_mut().enumerate() {
                        *value = (0..6)
                            .map(|a| {
                                (0..6)
                                    .map(|b| transform[a][row] * condensed_stiffness[a][b] * transform[b][column])
                                    .sum::<f64>()
                            })
                            .sum();
                    }
                }
                for (row, &row_dof) in displacement_dofs.iter().enumerate() {
                    let Some(row_dof) = row_dof
                    else {
                        continue;
                    };
                    for (column, &column_dof) in displacement_dofs.iter().enumerate() {
                        if let Some(column_dof) = column_dof {
                            stiffness[row_dof * size + column_dof] += global_stiffness[row][column];
                        }
                    }
                    let load: f64 = (0..6).map(|a| transform[a][row] * condensed_load[a]).sum();
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
                    active: true,
                    displacement_dofs,
                    model: ElementModel::Beam(Box::new(BeamLocal {
                        transform,
                        stiffness: local_stiffness,
                        load: local_load,
                        released,
                        transverse_start: load_start,
                        transverse_end: load_end,
                        axial_uniform,
                        partial,
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
                        displacement_dofs.push(Some(layout.dof(node * layout.slots + component)?));
                    }
                }
                let active: Vec<usize> = displacement_dofs.iter().map(|slot| slot.expect("truss degree of freedom")).collect();
                if element_active {
                    for row in 0..2 * translation_components {
                        let row_end = row / translation_components;
                        for column in 0..2 * translation_components {
                            let column_end = column / translation_components;
                            let sign = if row_end == column_end { 1.0 } else { -1.0 };
                            let value = sign
                                * direction[row % translation_components]
                                * direction[column % translation_components]
                                * axial_stiffness;
                            stiffness[active[row] * size + active[column]] += value;
                        }
                    }
                }
                if let Some(gravity) = gravity.filter(|_| element_active) {
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
                if element_active && thermal != 0.0 {
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
                    active: element_active,
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
        if support.direction.is_some() {
            // The node frame is rotated below, so the constrained component is
            // the local first translational degree of freedom.
            mark(base);
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

    let mut rotations: Vec<Option<[[f64; 2]; 2]>> = vec![None; model.nodes.len()];
    for support in &model.supports {
        let (Some(&node), Some(direction)) = (index.get(support.node.as_str()), support.direction)
        else {
            continue;
        };
        if model.dimension != 2 {
            return Err(CaeError::new(
                "capability/not-implemented",
                format!(
                    "an inclined support direction on node `{}` is only available in two-dimensional documents",
                    support.node
                ),
            ));
        }
        let norm = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
        if !norm.is_finite() || norm <= 0.0 {
            return Err(CaeError::new(
                "support/invalid-direction",
                format!("inclined support direction of node `{}` must be a finite non-zero vector", support.node),
            ));
        }
        let (nx, ny) = (direction[0] / norm, direction[1] / norm);
        // Columns are the local axes `n = (nx, ny)` and `t = (-ny, nx)`.
        rotations[node] = Some([[nx, -ny], [ny, nx]]);
    }
    apply_node_rotation(&mut stiffness, &mut forces, &layout, &rotations);

    Ok(System { layout, stiffness, forces, constrained, prescribed, elements, rotations })
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
/// Inverts the rotational block of the released degrees of freedom.
///
/// Only one or two bending rotations (local indices 2 and 5) can be released,
/// so the block is at most 2×2 and is inverted directly.
fn invert_release_block(stiffness: &[[f64; 6]; 6], indices: &[usize]) -> Vec<f64> {
    let count = indices.len();
    let mut block = vec![0.0; count * count];
    for (a, &i) in indices.iter().enumerate() {
        for (b, &j) in indices.iter().enumerate() {
            block[a * count + b] = stiffness[i][j];
        }
    }
    if count == 1 {
        vec![1.0 / block[0]]
    }
    else {
        let determinant = block[0] * block[3] - block[1] * block[2];
        vec![block[3] / determinant, -block[1] / determinant, -block[2] / determinant, block[0] / determinant]
    }
}

/// Condenses released bending rotations out of a beam stiffness and load.
///
/// `K_cond = K − K_er·K_rr⁻¹·K_re` and `f_cond = f − K_er·K_rr⁻¹·f_r`; the rows
/// and columns of released local degrees of freedom become zero because they
/// have no global counterpart.
fn condense_releases(stiffness: &[[f64; 6]; 6], load: &[f64; 6], released: [bool; 6]) -> ([[f64; 6]; 6], [f64; 6]) {
    let indices: Vec<usize> = (0..6).filter(|&index| released[index]).collect();
    if indices.is_empty() {
        return (*stiffness, *load);
    }
    let count = indices.len();
    let inverse = invert_release_block(stiffness, &indices);
    let mut condensed_stiffness = *stiffness;
    let mut condensed_load = *load;
    for row in 0..6 {
        if released[row] {
            condensed_stiffness[row] = [0.0; 6];
            continue;
        }
        // `z = K_rr⁻¹·K_erᵀ` for this row.
        let mut z = vec![0.0; count];
        for (a, value) in z.iter_mut().enumerate() {
            for b in 0..count {
                *value += inverse[a * count + b] * stiffness[row][indices[b]];
            }
        }
        for (column, value) in condensed_stiffness[row].iter_mut().enumerate() {
            if released[column] {
                *value = 0.0;
            }
            else {
                let correction: f64 = (0..count).map(|k| z[k] * stiffness[indices[k]][column]).sum();
                *value = stiffness[row][column] - correction;
            }
        }
        let correction: f64 = (0..count).map(|k| z[k] * load[indices[k]]).sum();
        condensed_load[row] = load[row] - correction;
    }
    for &index in &indices {
        condensed_load[index] = 0.0;
    }
    (condensed_stiffness, condensed_load)
}

/// Recovers the internal rotation of each released beam end.
///
/// A released end carries no moment, so `K_re·d_e + K_rr·θ_r = f_r` is solved
/// for `θ_r` before the full end forces are recovered.
fn restore_released_rotations(beam: &BeamLocal, local: &mut [f64; 6]) {
    let indices: Vec<usize> = (0..6).filter(|&index| beam.released[index]).collect();
    if indices.is_empty() {
        return;
    }
    let count = indices.len();
    let inverse = invert_release_block(&beam.stiffness, &indices);
    let mut rhs = vec![0.0; count];
    for (a, &index) in indices.iter().enumerate() {
        let mut value = beam.load[index];
        for (column, local_value) in local.iter().enumerate() {
            if !beam.released[column] {
                value -= beam.stiffness[index][column] * local_value;
            }
        }
        rhs[a] = value;
    }
    for a in 0..count {
        let rotation: f64 = (0..count).map(|b| inverse[a * count + b] * rhs[b]).sum();
        local[indices[a]] = rotation;
    }
}

fn add_linear_transverse(load: &mut [f64; 6], q1: f64, q2: f64, length: f64) {
    let length2 = length * length;
    load[1] += length * (7.0 * q1 + 3.0 * q2) / 20.0;
    load[2] += length2 * (3.0 * q1 + 2.0 * q2) / 60.0;
    load[4] += length * (3.0 * q1 + 7.0 * q2) / 20.0;
    load[5] -= length2 * (2.0 * q1 + 3.0 * q2) / 60.0;
}

/// Adds the consistent nodal vector of a linearly varying transverse load
/// acting between two fractions of the member length.
///
/// The integral of the cubic Hermite shape functions against the linear load is
/// evaluated with three-point Gauss-Legendre quadrature, which is exact for the
/// quartic integrand.
fn add_partial_transverse(load: &mut [f64; 6], partial: PartialLoad, length: f64) {
    const POINTS: [f64; 3] = [-0.774_596_669_241_483_4, 0.0, 0.774_596_669_241_483_4];
    const WEIGHTS: [f64; 3] = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];
    let start = partial.start * length;
    let end = partial.end * length;
    let half = 0.5 * (end - start);
    let middle = 0.5 * (end + start);
    for (point, weight) in POINTS.iter().zip(WEIGHTS) {
        let position = middle + half * point;
        let xi = position / length;
        let intensity = partial.load_start + (partial.load_end - partial.load_start) * (position - start) / (end - start);
        let shape = [
            1.0 - 3.0 * xi * xi + 2.0 * xi * xi * xi,
            length * (xi - 2.0 * xi * xi + xi * xi * xi),
            3.0 * xi * xi - 2.0 * xi * xi * xi,
            length * (-xi * xi + xi * xi * xi),
        ];
        let scale = weight * half * intensity;
        load[1] += scale * shape[0];
        load[2] += scale * shape[1];
        load[4] += scale * shape[2];
        load[5] += scale * shape[3];
    }
}

/// Cumulative transverse load and its first moment about the start node at
/// `position`, combining the full-span trapezoid with an optional partial load.
fn beam_load_resultants(
    full_start: f64,
    full_slope: f64,
    partial: Option<&PartialLoad>,
    length: f64,
    position: f64,
) -> (f64, f64) {
    let mut force = full_start * position + 0.5 * full_slope * position * position;
    let mut first_moment = 0.5 * full_start * position * position + full_slope * position.powi(3) / 3.0;
    if let Some(partial) = partial {
        let (partial_force, partial_moment) = partial_load_resultants(partial, length, position);
        force += partial_force;
        first_moment += partial_moment;
    }
    (force, first_moment)
}

/// Resultant and first moment of a partial load up to `position`.
fn partial_load_resultants(partial: &PartialLoad, length: f64, position: f64) -> (f64, f64) {
    let start = partial.start * length;
    let end = partial.end * length;
    let loaded = position.min(end);
    if loaded <= start {
        return (0.0, 0.0);
    }
    let slope = (partial.load_end - partial.load_start) / (end - start);
    let span = loaded - start;
    let force = partial.load_start * span + 0.5 * slope * span * span;
    let first_moment = 0.5 * partial.load_start * (loaded * loaded - start * start)
        + slope * ((loaded.powi(3) - start.powi(3)) / 3.0 - start * (loaded * loaded - start * start) / 2.0);
    (force, first_moment)
}

/// Integrals of a partial load against `(x − s)³` and `(x − s)²` up to
/// `position`, before dividing by `6·E·I` and `2·E·I`.
fn partial_deflection_integrals(partial: &PartialLoad, length: f64, position: f64) -> (f64, f64) {
    let start = partial.start * length;
    let end = partial.end * length;
    let loaded = position.min(end);
    if loaded <= start {
        return (0.0, 0.0);
    }
    let slope = (partial.load_end - partial.load_start) / (end - start);
    let near = position - loaded;
    let far = position - start;
    let base = partial.load_start + slope * (position - start);
    let cubic = base * (far.powi(4) - near.powi(4)) / 4.0 - slope * (far.powi(5) - near.powi(5)) / 5.0;
    let quadratic = base * (far.powi(3) - near.powi(3)) / 3.0 - slope * (far.powi(4) - near.powi(4)) / 4.0;
    (cubic, quadratic)
}

/// Assembles the diagonal lumped mass matrix of the active degrees of freedom.
///
/// Every element contributes half of its mass `ρ·A·L` to the translations of
/// its two nodes (and `ρ·I·L/2` to the rotations of a beam). Each node's
/// concentrated `Node::mass` is then added to all of its translational
/// degrees of freedom; it contributes no rotational inertia. Constrained
/// degrees of freedom have no entry in the layout and are naturally ignored.
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
    for (node, entry) in model.nodes.iter().enumerate() {
        if !entry.mass.is_finite() || entry.mass <= 0.0 {
            continue;
        }
        for component in 0..translations {
            if let Some(dof) = layout.index.get(node * layout.slots + component).copied().flatten() {
                mass[dof] += entry.mass;
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
    let applied_forces = system.to_global_vector(&system.forces);
    let mut applied = ForceResultant::default();
    for node in 0..model.nodes.len() {
        let mut loads = [0.0_f64; 3];
        for (component, value) in loads.iter_mut().enumerate().take(translation_components) {
            *value = node_value(layout, &applied_forces, node, component);
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
            let (fx, fy) = if support.direction.is_some() {
                // An inclined support reacts along an arbitrary direction, so
                // both global components are reported from the equilibrium
                // residual instead of splitting local and global constraints.
                (node_value(layout, residual, node, 0), node_value(layout, residual, node, 1))
            }
            else {
                (component(support.dx, spring.kx, 0), component(support.dy, spring.ky, 1))
            };
            Some(SupportReaction {
                node: support.node.clone(),
                fx,
                fy,
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

/// Version of the JSON report produced by [`solve_influence_line`].
pub const INFLUENCE_LINE_REPORT_VERSION: u32 = 1;

/// Format tag of the influence-line JSON report.
pub const INFLUENCE_LINE_REPORT_FORMAT: &str = "open-cae-influence-line-report";

/// Value identifying an influence-line result in [`InfluenceLineSolution::analysis`].
pub const INFLUENCE_ANALYSIS: &str = "influenceLine";

/// Direction of the moving unit load of an influence line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InfluenceDirection {
    /// Horizontal unit load acting along global X.
    #[serde(rename = "x")]
    X,
    /// Vertical unit load acting along global Y.
    #[serde(rename = "y")]
    Y,
}

impl InfluenceDirection {
    /// Returns the short identifier used by requests and reports (`"x"` or `"y"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }
}

/// Component of a support reaction sampled on an influence line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReactionComponent {
    /// Reaction along global X.
    #[serde(rename = "fx")]
    Fx,
    /// Reaction along global Y.
    #[serde(rename = "fy")]
    Fy,
    /// Reaction moment about the out-of-plane axis.
    #[serde(rename = "mz")]
    Mz,
}

/// Component of a section force sampled on an influence line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SectionComponent {
    /// Axial force, positive in tension.
    #[serde(rename = "axial")]
    Axial,
    /// Local shear force at the section.
    #[serde(rename = "shear")]
    Shear,
    /// Local bending moment at the section.
    #[serde(rename = "moment")]
    Moment,
}

/// Quantity sampled along the track element as the unit load moves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InfluenceQuantity {
    /// Reaction component reported at a supported node.
    #[serde(rename_all = "camelCase")]
    Reaction {
        /// Node carrying the support.
        node: String,
        /// Reaction component reported at that node.
        component: ReactionComponent,
    },
    /// Section force component reported at a fixed position of an element.
    #[serde(rename_all = "camelCase")]
    Section {
        /// Element carrying the section; defaults to the track element.
        #[serde(default)]
        element: Option<String>,
        /// Section location as a fraction of the element length in `[0, 1]`.
        position: f64,
        /// Section force component reported at that position.
        component: SectionComponent,
    },
}

/// Influence-line ordinate at one load position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceSample {
    /// Load position as a fraction of the track length in `[0, 1]`.
    pub position: f64,
    /// Influence ordinate at that load position.
    pub value: f64,
}

/// Position and value of an extreme influence ordinate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceExtreme {
    /// Load position of the extreme, as a fraction of the track length.
    pub position: f64,
    /// Extreme influence ordinate.
    pub value: f64,
}

/// Influence-line solution produced by [`solve_influence_line`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceLineSolution {
    /// Report format tag, always `open-cae-influence-line-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Analysis identifier, always `influenceLine`.
    pub analysis: String,
    /// Identifier of the track element the unit load moves along.
    pub track: String,
    /// Length of the (unsplit) track element.
    pub length: f64,
    /// Direction of the moving unit load (`"x"` or `"y"`).
    pub direction: String,
    /// Sampled quantity with the track element filled in as section default.
    pub quantity: InfluenceQuantity,
    /// Number of sampled load positions, including both track ends.
    pub points: usize,
    /// Influence ordinates in load-position order.
    pub samples: Vec<InfluenceSample>,
    /// Sample with the smallest ordinate.
    pub min: InfluenceExtreme,
    /// Sample with the largest ordinate.
    pub max: InfluenceExtreme,
    /// Sample with the largest absolute ordinate.
    pub governing: InfluenceExtreme,
}

/// Length of one element computed from its node coordinates.
fn influence_element_length(model: &Model, id: &str) -> Result<f64, CaeError> {
    let element = model
        .elements
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{id}`")))?;
    let from = model.node(&element.from).ok_or_else(|| {
        CaeError::new("element/unknown-node", format!("element `{id}` references unknown node `{}`", element.from))
    })?;
    let to = model.node(&element.to).ok_or_else(|| {
        CaeError::new("element/unknown-node", format!("element `{id}` references unknown node `{}`", element.to))
    })?;
    Ok(((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt())
}

/// Builds an influence line by moving a unit load along a track beam element.
///
/// `points` equally spaced load positions including both track ends are
/// sampled. Every sample rebuilds a load-free copy of the model, applies the
/// unit load in `direction` at the load position — splitting the track element
/// at strictly interior positions — and reports the requested reaction or
/// section ordinate of the linear-static solution. The model must not carry
/// any other load, because the influence line is defined for the moving unit
/// load alone; a loaded model reports `influence/loads-present`. When the
/// section position coincides with the interior split point, the from-side
/// limit of the section force is reported.
pub fn solve_influence_line(
    model: &Model,
    track: &str,
    quantity: &InfluenceQuantity,
    direction: InfluenceDirection,
    points: usize,
) -> Result<InfluenceLineSolution, CaeError> {
    if points < 2 {
        return Err(CaeError::new(
            "analysis/invalid-points",
            format!("influence line sampling requires at least 2 points, but {points} were requested"),
        ));
    }
    let track_index = model
        .elements
        .iter()
        .position(|entry| entry.id == track)
        .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{track}`")))?;
    let track_element = &model.elements[track_index];
    if track_element.kind != ElementKind::Beam {
        return Err(CaeError::new(
            "influence/track-not-beam",
            format!(
                "influence line track `{track}` must be a beam element, but it is a `{}` element",
                track_element.kind.as_str()
            ),
        ));
    }
    if model.dimension != 2 {
        return Err(CaeError::new(
            "capability/not-implemented",
            "influence line analysis is only implemented for two-dimensional models",
        ));
    }
    let loaded = !model.loads.is_empty()
        || model.elements.iter().any(|element| {
            [
                element.distributed_load,
                element.distributed_load_from,
                element.distributed_load_to,
                element.temperature,
                element.temperature_gradient,
            ]
            .into_iter()
            .flatten()
            .any(|value| value != 0.0)
        })
        || model.gravity.is_some_and(|gravity| gravity.iter().any(|component| *component != 0.0))
        || model.supports.iter().any(|support| {
            support.prescribed.as_ref().is_some_and(|prescribed| {
                prescribed.ux != 0.0 || prescribed.uy != 0.0 || prescribed.uz != 0.0 || prescribed.rz != 0.0
            })
        });
    if loaded {
        return Err(CaeError::new(
            "influence/loads-present",
            "influence line analysis requires an unloaded model; the influence line is the response to the moving unit load alone",
        ));
    }

    let quantity = match quantity {
        InfluenceQuantity::Reaction { node, component } => {
            if model.node(node).is_none() {
                return Err(CaeError::new("node/unknown-id", format!("unknown node `{node}`")));
            }
            if !model.supports.iter().any(|support| support.node == *node) {
                return Err(CaeError::new("result/no-reaction", format!("node `{node}` has no support reaction")));
            }
            InfluenceQuantity::Reaction { node: node.clone(), component: *component }
        }
        InfluenceQuantity::Section { element, position, component } => {
            let position = *position;
            let element_id = element.as_deref().unwrap_or(track);
            let section_index = model
                .elements
                .iter()
                .position(|entry| entry.id == element_id)
                .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{element_id}`")))?;
            if !position.is_finite() || position < 0.0 || position > 1.0 {
                return Err(CaeError::new(
                    "influence/invalid-position",
                    format!("influence line section position must be finite and within [0, 1] (currently {position})"),
                ));
            }
            if model.elements[section_index].kind == ElementKind::Truss && *component != SectionComponent::Axial {
                let name = match component {
                    SectionComponent::Axial => "axial",
                    SectionComponent::Shear => "shear",
                    SectionComponent::Moment => "moment",
                };
                return Err(CaeError::new(
                    "influence/invalid-component",
                    format!(
                        "section component `{name}` is not defined for truss element `{element_id}`; only `axial` is supported"
                    ),
                ));
            }
            InfluenceQuantity::Section { element: Some(element_id.to_string()), position, component: *component }
        }
    };
    let track_length = influence_element_length(model, track)?;

    let mut samples = Vec::with_capacity(points);
    for index in 0..points {
        let t = index as f64 / (points - 1) as f64;
        let mut working = model.clone();
        let mut split: Option<crate::ElementSplit> = None;
        let load_node = if t == 0.0 {
            track_element.from.clone()
        }
        else if t == 1.0 {
            track_element.to.clone()
        }
        else {
            let produced = working.split_element(track, t)?;
            split = Some(produced.clone());
            produced.node
        };
        working.loads.push(Load {
            node: load_node,
            fx: if direction == InfluenceDirection::X { 1.0 } else { 0.0 },
            fy: if direction == InfluenceDirection::Y { 1.0 } else { 0.0 },
            fz: 0.0,
            mz: 0.0,
        });
        let solution = solve(&working)?;
        let value = match &quantity {
            InfluenceQuantity::Reaction { node, component } => {
                let reaction = solution
                    .reactions
                    .iter()
                    .find(|entry| entry.node == *node)
                    .ok_or_else(|| CaeError::new("node/unknown-id", format!("unknown node `{node}`")))?;
                match component {
                    ReactionComponent::Fx => reaction.fx,
                    ReactionComponent::Fy => reaction.fy,
                    ReactionComponent::Mz => reaction.mz,
                }
            }
            InfluenceQuantity::Section { element, position, component } => {
                let position = *position;
                let element_id = element.as_deref().unwrap_or(track);
                let (evaluated, x) = if element_id != track {
                    (element_id.to_string(), position * influence_element_length(model, element_id)?)
                }
                else if t > 0.0 && t < 1.0 {
                    let split = split.expect("strictly interior positions are split");
                    if position <= t {
                        (track.to_string(), position * track_length)
                    }
                    else {
                        (split.second.clone(), (position - t) / (1.0 - t) * (1.0 - t) * track_length)
                    }
                }
                else {
                    (element_id.to_string(), position * track_length)
                };
                let result = solution
                    .elements
                    .iter()
                    .find(|entry| entry.id == evaluated)
                    .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{evaluated}`")))?;
                let end = &result.end_forces;
                match component {
                    SectionComponent::Axial => -end.n1,
                    SectionComponent::Shear => end.v1,
                    SectionComponent::Moment => -end.m1 + end.v1 * x,
                }
            }
        };
        samples.push(InfluenceSample { position: t, value });
    }

    let min = samples.iter().fold(
        samples[0].clone(),
        |current: InfluenceSample, sample| {
            if sample.value < current.value { sample.clone() } else { current }
        },
    );
    let max = samples.iter().fold(
        samples[0].clone(),
        |current: InfluenceSample, sample| {
            if sample.value > current.value { sample.clone() } else { current }
        },
    );
    let governing = samples.iter().fold(samples[0].clone(), |current: InfluenceSample, sample| {
        if sample.value.abs() > current.value.abs() { sample.clone() } else { current }
    });

    Ok(InfluenceLineSolution {
        format: INFLUENCE_LINE_REPORT_FORMAT.to_string(),
        version: INFLUENCE_LINE_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        analysis: INFLUENCE_ANALYSIS.to_string(),
        track: track.to_string(),
        length: track_length,
        direction: direction.as_str().to_string(),
        quantity,
        points,
        samples,
        min: InfluenceExtreme { position: min.position, value: min.value },
        max: InfluenceExtreme { position: max.position, value: max.value },
        governing: InfluenceExtreme { position: governing.position, value: governing.value },
    })
}

/// Version of the JSON report produced by [`solve_influence_line_train`].
pub const INFLUENCE_TRAIN_REPORT_VERSION: u32 = 1;

/// Format tag of the influence-line load-train JSON report.
pub const INFLUENCE_TRAIN_REPORT_FORMAT: &str = "open-cae-influence-line-train-report";

/// Value identifying a load-train envelope in [`InfluenceTrainSolution::analysis`].
pub const INFLUENCE_TRAIN_ANALYSIS: &str = "influenceLineTrain";

fn default_influence_direction() -> InfluenceDirection {
    InfluenceDirection::Y
}

fn default_influence_points() -> usize {
    11
}

fn default_influence_step() -> f64 {
    0.01
}

/// Moving load train applied to the influence line built by [`solve_influence_line`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceTrainRequest {
    /// Identifier of the track element the train moves along.
    pub track: String,
    /// Reaction or section quantity to envelope.
    pub quantity: InfluenceQuantity,
    /// Direction of the axle loads; defaults to `"y"`.
    #[serde(default = "default_influence_direction")]
    pub direction: InfluenceDirection,
    /// Axle loads in train order, the first entry leading the train.
    pub axles: Vec<f64>,
    /// Distances between consecutive axles along the train direction, `axles.len() - 1` entries.
    pub spacings: Vec<f64>,
    /// Number of influence-line samples including both track ends; defaults to 11.
    #[serde(default = "default_influence_points")]
    pub points: usize,
    /// Train position sampling step as a fraction of the track length; defaults to 0.01.
    #[serde(default = "default_influence_step")]
    pub step: f64,
}

/// Extreme of a load-train envelope with the train position that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceTrainExtreme {
    /// Leading-axle coordinate along the track in model length units.
    pub position: f64,
    /// Envelope value of the sampled quantity at that train position.
    pub value: f64,
    /// Number of axles standing on the track at that train position.
    pub axles_on_track: usize,
}

/// Load-train envelope produced by [`solve_influence_line_train`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InfluenceTrainSolution {
    /// Report format tag, always `open-cae-influence-line-train-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Analysis identifier, always `influenceLineTrain`.
    pub analysis: String,
    /// Identifier of the track element the train moved along.
    pub track: String,
    /// Length of the (unsplit) track element.
    pub length: f64,
    /// Direction of the axle loads (`"x"` or `"y"`).
    pub direction: String,
    /// Enveloped quantity with the track element filled in as section default.
    pub quantity: InfluenceQuantity,
    /// Number of influence-line samples, including both track ends.
    pub points: usize,
    /// Train position sampling step as a fraction of the track length.
    pub step: f64,
    /// Axle loads in train order.
    pub axles: Vec<f64>,
    /// Distances between consecutive axles.
    pub spacings: Vec<f64>,
    /// Distance from the leading to the trailing axle.
    pub train_length: f64,
    /// Piecewise-linear influence ordinates reused from [`solve_influence_line`].
    pub samples: Vec<InfluenceSample>,
    /// Smallest envelope value.
    pub min: InfluenceTrainExtreme,
    /// Largest envelope value.
    pub max: InfluenceTrainExtreme,
    /// Envelope extreme with the largest absolute value.
    pub governing: InfluenceTrainExtreme,
}

/// Evaluates the piecewise-linear influence line at one normalised load position.
fn influence_ordinate(samples: &[InfluenceSample], position: f64) -> f64 {
    let last = samples.len() - 1;
    if position <= samples[0].position {
        return samples[0].value;
    }
    if position >= samples[last].position {
        return samples[last].value;
    }
    for index in 0..last {
        let from = &samples[index];
        let to = &samples[index + 1];
        if position <= to.position {
            let fraction = (position - from.position) / (to.position - from.position);
            return from.value + (to.value - from.value) * fraction;
        }
    }
    samples[last].value
}

/// Envelopes a moving load train over the influence line of [`solve_influence_line`].
///
/// The train is a sequence of axle loads kept at fixed distances, with the
/// first entry leading. It moves along the track element and the response is
/// the sum of every axle load times the influence ordinate below it. The
/// influence line is sampled once as a piecewise-linear polyline and reused
/// for every train position; the structure is never re-solved. Candidates for
/// the envelope are a uniform grid of step [`InfluenceTrainRequest::step`]
/// (a fraction of the track length) plus, for every axle, the exact positions
/// where that axle crosses an influence-line vertex, which is where the
/// envelope of a piecewise-linear ordinate attains its extremes. Axles outside
/// the track do not load the structure and are excluded from the sum, so the
/// train may enter and leave the track partially. The model must not carry any
/// other load, exactly as for [`solve_influence_line`].
pub fn solve_influence_line_train(model: &Model, request: &InfluenceTrainRequest) -> Result<InfluenceTrainSolution, CaeError> {
    if request.axles.is_empty() {
        return Err(CaeError::new("influence/empty-train", "a moving load train requires at least one axle"));
    }
    if request.spacings.len() + 1 != request.axles.len() {
        return Err(CaeError::new(
            "influence/spacing-mismatch",
            format!(
                "a train with {} axles requires exactly {} axle spacings, but {} were given",
                request.axles.len(),
                request.axles.len() - 1,
                request.spacings.len()
            ),
        ));
    }
    if request.axles.iter().any(|axle| !axle.is_finite()) {
        return Err(CaeError::new("influence/invalid-axle-load", "every axle load must be a finite force"));
    }
    if request.spacings.iter().any(|spacing| !spacing.is_finite() || *spacing < 0.0) {
        return Err(CaeError::new("influence/negative-spacing", "every axle spacing must be finite and non-negative"));
    }
    if !request.step.is_finite() || request.step <= 0.0 || request.step > 1.0 {
        return Err(CaeError::new(
            "influence/invalid-step",
            format!(
                "the train sampling step must be finite and within (0, 1] as a fraction of the track length (currently {})",
                request.step
            ),
        ));
    }
    let line = solve_influence_line(model, &request.track, &request.quantity, request.direction, request.points)?;

    let mut offsets = vec![0.0; request.axles.len()];
    for (index, spacing) in request.spacings.iter().enumerate() {
        offsets[index + 1] = offsets[index] + spacing / line.length;
    }
    let last_offset = *offsets.last().expect("the train has at least one axle");
    let maximum = 1.0 + last_offset;

    let mut positions: Vec<f64> = Vec::new();
    let mut position = 0.0;
    while position < maximum {
        positions.push(position);
        position += request.step;
    }
    positions.push(maximum);
    for sample in &line.samples {
        for offset in &offsets {
            let critical = sample.position + offset;
            if critical >= -POSITION_TOLERANCE && critical <= maximum + POSITION_TOLERANCE {
                positions.push(critical.clamp(0.0, maximum));
            }
        }
    }
    positions.sort_by(f64::total_cmp);
    positions.dedup_by(|current, previous| (*current - *previous).abs() <= POSITION_TOLERANCE);

    let extremes: Vec<InfluenceTrainExtreme> = positions
        .iter()
        .map(|position| {
            let mut value = 0.0;
            let mut axles_on_track = 0usize;
            for (axle, offset) in request.axles.iter().zip(&offsets) {
                let ordinate = position - offset;
                if (-POSITION_TOLERANCE..=1.0 + POSITION_TOLERANCE).contains(&ordinate) {
                    value += axle * influence_ordinate(&line.samples, ordinate.clamp(0.0, 1.0));
                    axles_on_track += 1;
                }
            }
            InfluenceTrainExtreme { position: position * line.length, value, axles_on_track }
        })
        .collect();

    let min = extremes.iter().fold(
        extremes[0].clone(),
        |current: InfluenceTrainExtreme, extreme| {
            if extreme.value < current.value { extreme.clone() } else { current }
        },
    );
    let max = extremes.iter().fold(
        extremes[0].clone(),
        |current: InfluenceTrainExtreme, extreme| {
            if extreme.value > current.value { extreme.clone() } else { current }
        },
    );
    let governing = extremes.iter().fold(extremes[0].clone(), |current: InfluenceTrainExtreme, extreme| {
        if extreme.value.abs() > current.value.abs() { extreme.clone() } else { current }
    });

    Ok(InfluenceTrainSolution {
        format: INFLUENCE_TRAIN_REPORT_FORMAT.to_string(),
        version: INFLUENCE_TRAIN_REPORT_VERSION,
        units: line.units.clone(),
        analysis: INFLUENCE_TRAIN_ANALYSIS.to_string(),
        track: line.track.clone(),
        length: line.length,
        direction: line.direction.clone(),
        quantity: line.quantity.clone(),
        points: line.points,
        step: request.step,
        axles: request.axles.clone(),
        spacings: request.spacings.clone(),
        train_length: request.spacings.iter().sum(),
        samples: line.samples,
        min,
        max,
        governing,
    })
}

/// Tolerance for treating normalised load positions as equal.
const POSITION_TOLERANCE: f64 = 1e-12;
