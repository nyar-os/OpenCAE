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
//! * prescribed (enforced) nodal displacements and support settlements;
//! * modal analysis with a diagonal (lumped) mass matrix and a symmetric
//!   Jacobi eigen-solver;
//! * linear buckling of two- and three-dimensional truss models and
//!   two-dimensional beam models using a geometric stiffness assembled from
//!   the static axial forces;
//! * support reaction summaries with a global force equilibrium check;
//! * built-in material presets and parametric rectangular, circular and
//!   I-section properties;
//! * direct stiffness assembly with a dense, partially pivoted Gaussian
//!   elimination solver written in this crate.
//!
//! Three-dimensional beams, frames, nonlinear analysis, thermal effects,
//! contact and solver-specific import formats (Abaqus `.inp`, STEP, ...) are
//! **not** implemented. See [`CAPABILITIES_JSON`] for the machine-readable
//! capability matrix and the repository Readme for the user facing
//! description.

#![deny(missing_docs)]

mod error;
mod export;
mod library;
mod model;
mod session;
mod solver;
mod validate;

pub use crate::{
    error::CaeError,
    export::{export_csv, export_md, export_vtk, write_atomic},
    library::{MATERIAL_PRESETS, MaterialPreset, SectionProperties, library_json, material_preset},
    model::{
        Element, ElementKind, ElementMaterial, FORMAT, Load, LoadCombination, Material, Model, Node, PrescribedDisplacement,
        Section, Support, Units, VERSION,
    },
    session::{
        DocumentSummary, ExportFormat, ExportReport, OpenCae, OpenCaeOptions, Report, ValidationReport, not_implemented,
    },
    solver::{
        BUCKLING_REPORT_FORMAT, BUCKLING_REPORT_VERSION, BucklingMode, BucklingSolution, COMBINATION_REPORT_FORMAT,
        COMBINATION_REPORT_VERSION, CombinationCase, CombinationEnvelope, CombinationSolution, ElementDiagram, ElementEnvelope,
        ElementResult, EndForces, EnvelopeValue, ForceResultant, INTERNAL_FORCE_REPORT_FORMAT, INTERNAL_FORCE_REPORT_VERSION,
        InternalForceSolution, MODAL_REPORT_FORMAT, MODAL_REPORT_VERSION, MaxAxialForce, MaxMoment, ModalSolution, Mode,
        ModeShape, NodeDisplacement, NodeEnvelope, NodeReaction, REPORT_FORMAT, REPORT_VERSION, ReactionSummary, SectionForces,
        Solution, SupportReaction, solve, solve_buckling, solve_combinations, solve_internal_forces, solve_modal,
    },
    validate::{Issue, Severity},
};

/// Stable machine-readable capability matrix of this build.
///
/// The value is a pretty-printed JSON object with two sections: `solver` and
/// `io`. Supported entries are marked `"supported"`, everything else is
/// `"unsupported"` and must not be silently treated as working.
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
    "internalForces": "supported",
    "loadCombinations": "supported",
    "materialLibrary": "supported",
    "thermal": "unsupported"
  },
  "io": {
    "ocae": "supported",
    "csv": "supported",
    "vtk": "supported",
    "md": "supported",
    "svg": "unsupported",
    "inp": "unsupported",
    "step": "unsupported"
  }
}"#;
