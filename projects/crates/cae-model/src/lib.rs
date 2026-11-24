//! OpenCAE: an honest small-displacement finite element solver for trusses and
//! two-dimensional beams.
//!
//! This crate implements a deliberately verifiable subset of structural
//! analysis:
//!
//! * two- or three-dimensional truss members carrying axial force only;
//! * two-dimensional Euler-Bernoulli beams with axial, shear and bending
//!   stiffness, uniform transverse loads and endpoint moments;
//! * linear elastic material behaviour and a single static load case;
//! * self-weight body loads;
//! * uniform member temperature changes with linear thermal expansion
//!   (`α·ΔT`) and the matching equivalent nodal loads;
//! * prescribed (enforced) nodal displacements and support settlements;
//! * steady-state axial heat conduction with fixed nodal temperatures,
//!   concentrated nodal heat flows, optional surface convection to an ambient
//!   temperature, element heat fluxes and boundary reactions;
//! * modal analysis with a diagonal (lumped) mass matrix and a symmetric
//!   Jacobi eigen-solver, including steady-state harmonic (frequency-response)
//!   sweeps by modal superposition;
//! * linear buckling of two- and three-dimensional truss models and
//!   two-dimensional beam models using a geometric stiffness assembled from
//!   the static axial forces, including per-load-combination buckling;
//! * support reaction summaries with a global force equilibrium check;
//! * linear elastic translational and rotational spring supports;
//! * built-in material presets and parametric rectangular, circular and
//!   I-section properties, plus custom closed-polygon section properties
//!   (area, centroid, second moments, polar moment and radii of gyration) and
//!   standalone I, rectangular-tube, channel, L (angle), T, solid ellipse and
//!   equal-thickness Z section calculators (area, centroid, second moments,
//!   torsion constant and radii of gyration);
//! * construction-stage (element birth/death) summaries that solve the active
//!   subset of elements stage by stage;
//! * a large-displacement Newton iteration for truss models with the exact
//!   corotational bar tangent;
//! * direct stiffness assembly with a dense, partially pivoted Gaussian
//!   elimination solver written in this crate.
//!
//! Three-dimensional beams, frames, material nonlinearity, transient heat
//! transfer (radiation and time stepping), contact and solver-specific import
//! and export formats (STEP, Nastran, ...) are **not** implemented. A documented
//! subset of Abaqus `.inp` files can be imported with [`import_abaqus_file`];
//! every recognised but unrepresented feature is reported as an
//! [`ImportLoss`]. See [`CAPABILITIES_JSON`] for the machine-readable
//! capability matrix and the repository Readme for the user facing description.

#![deny(missing_docs)]

mod abaqus;
mod envelope;
mod error;
mod export;
mod library;
mod model;
mod session;
mod solver;
mod validate;

pub use crate::{
    abaqus::{
        ABAQUS_IMPORT_REPORT_FORMAT, ABAQUS_IMPORT_REPORT_VERSION, AbaqusImport, ImportLoss, import_abaqus,
        import_abaqus_file,
    },
    envelope::{
        ENVELOPE_REPORT_FORMAT, ENVELOPE_REPORT_VERSION, EnvelopeElement, EnvelopeExtreme, EnvelopeNode, EnvelopeReaction,
        EnvelopeReport, envelope_csv, solve_envelope,
    },
    error::CaeError,
    export::{
        ContourQuantity, export_csv, export_csv_with_stations, export_html, export_md, export_svg, export_svg_with_contour,
        export_vtk, write_atomic,
    },
    library::{
        MATERIAL_PRESETS, MaterialPreset, PolygonSectionReport, PrincipalSectionReport, SectionProperties, SectionReport,
        StandardSectionReport, angle_section_report, channel_standard_section_report, ellipse_section_report,
        i_standard_section_report, library_json, material_preset, polygon_section_report, principal_section_report,
        section_report, t_section_report, triangle_section_report, tube_standard_section_report, z_section_report,
    },
    model::{
        Element, ElementKind, ElementMaterial, ElementSplit, ElementUpdate, EndRelease, FORMAT, Load, LoadCombination,
        LoadPattern, Material, Model, Node, PatternFactor, PrescribedDisplacement, Section, SpringStiffness, Support, Units,
        VERSION,
    },
    session::{
        DocumentSummary, ElementSplitReport, ExportFormat, ExportOptions, ExportReport, ImportReport, OpenCae, OpenCaeOptions,
        Report, ValidationReport, not_implemented,
    },
    solver::{
        BUCKLING_COMBINATION_REPORT_FORMAT, BUCKLING_COMBINATION_REPORT_VERSION, BUCKLING_REPORT_FORMAT,
        BUCKLING_REPORT_VERSION, BucklingCase, BucklingCombinationSolution, BucklingEnvelope, BucklingMode, BucklingSolution,
        COMBINATION_REPORT_FORMAT, COMBINATION_REPORT_VERSION, CONSTRUCTION_STAGE_REPORT_FORMAT,
        CONSTRUCTION_STAGE_REPORT_VERSION, CombinationCase, CombinationEnvelope, CombinationSolution, ConstructionStage,
        ConstructionStageResult, ConstructionStageSolution, DEFLECTION_REPORT_FORMAT, DEFLECTION_REPORT_VERSION,
        DeflectionGoverning, DeflectionSolution, DeflectionStation, ElementDeflection, ElementDiagram, ElementEnvelope,
        ElementResult, ElementStress, EndForces, EnvelopeValue, FREQUENCY_RESPONSE_REPORT_FORMAT,
        FREQUENCY_RESPONSE_REPORT_VERSION, ForceResultant, FrequencyResponsePoint, FrequencyResponseSolution, HEAT_ANALYSIS,
        HEAT_REPORT_FORMAT, HEAT_REPORT_VERSION, HeatConvection, HeatElementFlux, HeatReaction, HeatSolution, HeatTemperature,
        INFLUENCE_ANALYSIS, INFLUENCE_LINE_REPORT_FORMAT, INFLUENCE_LINE_REPORT_VERSION, INFLUENCE_TRAIN_ANALYSIS,
        INFLUENCE_TRAIN_REPORT_FORMAT, INFLUENCE_TRAIN_REPORT_VERSION, INTERNAL_FORCE_REPORT_FORMAT,
        INTERNAL_FORCE_REPORT_VERSION, InfluenceDirection, InfluenceExtreme, InfluenceLineSolution, InfluenceQuantity,
        InfluenceSample, InfluenceTrainExtreme, InfluenceTrainRequest, InfluenceTrainSolution, InternalForceSolution,
        LOAD_CASE_REPORT_FORMAT, LOAD_CASE_REPORT_VERSION, LoadCaseResult, LoadCaseSummary, MATRIX_REPORT_FORMAT,
        MATRIX_REPORT_VERSION, MODAL_REPORT_FORMAT, MODAL_REPORT_VERSION, MODEL_SUMMARY_FORMAT, MODEL_SUMMARY_VERSION,
        MatrixDof, MaxAxialForce, MaxMoment, ModalSolution, Mode, ModeShape, ModelSummaryReport, NODAL_STRESS_REPORT_FORMAT,
        NODAL_STRESS_REPORT_VERSION, NONLINEAR_REPORT_FORMAT, NONLINEAR_REPORT_VERSION, NodalStressGoverning,
        NodalStressSolution, NodeDisplacement, NodeEnvelope, NodeFrequencyResponse, NodeReaction, NodeStress,
        NonlinearSolution, REPORT_FORMAT, REPORT_VERSION, RESPONSE_SPECTRUM_REPORT_FORMAT, RESPONSE_SPECTRUM_REPORT_VERSION,
        ReactionComponent, ReactionSummary, ResponseSpectrumSolution, STRESS_REPORT_FORMAT, STRESS_REPORT_VERSION,
        SectionComponent, SectionForces, Solution, SpectrumCombination, SpectrumMode, SpectrumPoint, StationStress,
        StressGoverning, StressSolution, SupportReaction, SystemMatrices, TENSION_ONLY_REPORT_FORMAT,
        TENSION_ONLY_REPORT_VERSION, TensionOnlyElement, TensionOnlySolution, solve, solve_buckling,
        solve_buckling_combinations, solve_combinations, solve_construction_stages, solve_deflection, solve_frequency_response,
        solve_heat, solve_influence_line, solve_influence_line_train, solve_internal_forces, solve_matrices, solve_modal,
        solve_nodal_stress, solve_nonlinear, solve_response_spectrum, solve_stress, solve_tension_only, summarize_load_cases,
        summarize_model,
    },
    validate::{Issue, Severity},
};

/// Stable machine-readable capability matrix of this build.
///
/// The value is a pretty-printed JSON object with three sections: `solver`,
/// `edits` and `io`. Supported entries are marked `"supported"`, everything else
/// is `"unsupported"` and must not be silently treated as working.
pub const CAPABILITIES_JSON: &str = r#"{
  "solver": {
    "truss2d": "supported",
    "truss3d": "supported",
    "beam2d": "supported",
    "beam3d": "unsupported",
    "frame3d": "unsupported",
    "modal": "supported",
    "selfWeight": "supported",
    "prescribedDisplacement": "supported",
    "nonlinear": "unsupported",
    "buckling": "supported",
    "bucklingCombinations": "supported",
    "internalForces": "supported",
    "influenceLine": "supported",
    "influenceLineTrain": "supported",
    "stressRecovery": "supported",
    "nodalStress": "supported",
    "loadCases": "supported",
    "combinationEnvelope": "supported",
    "modelSummary": "supported",
    "deflections": "supported",
    "loadCombinations": "supported",
    "loadPatterns": "supported",
    "materialLibrary": "supported",
    "polygonSection": "supported",
    "standardSection": "supported",
    "principalSection": "supported",
    "elasticSupports": "supported",
    "inclinedSupports": "supported",
    "matrixExport": "supported",
    "responseSpectrum": "supported",
    "frequencyResponse": "supported",
    "constructionStages": "supported",
    "geometricNonlinear": "supported",
    "tensionOnly": "supported",
    "thermalExpansion": "supported",
    "thermalGradient": "supported",
    "partialSpanLoad": "supported",
    "thermal": "supported",
    "heatTransfer": "supported",
    "heatConvection": "supported"
  },
  "edits": {
    "elementSplit": "supported",
    "elementUpdate": "supported"
  },
  "io": {
    "ocae": "supported",
    "csv": "supported",
    "vtk": "supported",
    "md": "supported",
    "html": "supported",
    "svg": "supported",
    "svgContour": "supported",
    "inp": "supported",
    "step": "unsupported"
  }
}"#;
