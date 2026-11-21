//! Path-backed editing session: the shared core of the CLI, the Node-API
//! surface and the TypeScript SDK.
//!
//! [`OpenCae`] owns at most one document at a time. Every editing call loads the
//! document it already opened, mutates the in-memory model, writes the file
//! atomically and returns a fresh [`DocumentSummary`], so the CLI, the native
//! addon and the SDK cannot drift apart. Argument parsing, help text and exit
//! codes stay outside this module.

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::Serialize;

use crate::{
    CAPABILITIES_JSON, CaeError,
    export::{export_csv, export_md, export_vtk, write_atomic},
    model::{ElementKind, Material, Model, Units},
    solver::{ModalSolution, REPORT_FORMAT, REPORT_VERSION, Solution, solve, solve_modal},
    validate::{Issue, Severity},
};

/// Options accepted by [`OpenCae::new`] and [`OpenCae::create`].
///
/// Values given to [`OpenCae::create`] override the session defaults from
/// [`OpenCae::new`]. `None` means "not supplied", so the session default or the
/// document default (`mm`, two dimensions) applies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpenCaeOptions {
    /// Unit system of a newly created document.
    pub units: Option<Units>,
    /// Spatial dimension of a newly created document; `2` or `3`.
    pub dimension: Option<u32>,
}

impl OpenCaeOptions {
    /// Returns these defaults with the explicitly supplied values of `other`
    /// taking precedence.
    fn merged_with(self, other: Self) -> Self {
        Self { units: other.units.or(self.units), dimension: other.dimension.or(self.dimension) }
    }
}

/// Summary of the document a session currently owns.
///
/// The shape is stable and serialises with camelCase keys so the Node-API
/// surface and the TypeScript SDK can decode it directly.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSummary {
    /// Opaque document identity, derived from the document path.
    pub id: String,
    /// Absolute or caller supplied path of the document file.
    pub path: String,
    /// Declared unit system, or `null` when the file does not declare one.
    pub units: Option<Units>,
    /// Spatial dimension, `2` or `3`.
    pub dimension: u32,
    /// Monotonically increasing edit counter.
    pub revision: u32,
    /// Number of nodes.
    pub nodes: usize,
    /// Number of truss elements.
    pub elements: usize,
    /// Number of support entries.
    pub supports: usize,
    /// Number of concentrated load entries.
    pub loads: usize,
    /// Global material and section data.
    pub material: Material,
}

/// Result of [`OpenCae::validate`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    /// `true` when no finding has [`Severity::Error`].
    pub ok: bool,
    /// Every finding, in the order the validator produced them.
    pub issues: Vec<Issue>,
}

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Two CSV files (`nodes.csv`, `elements.csv`) inside a directory.
    Csv,
    /// One legacy ASCII VTK `UNSTRUCTURED_GRID` file.
    Vtk,
    /// One deterministic Markdown engineering report.
    Md,
}

impl ExportFormat {
    /// Returns the short identifier used by the CLI and by reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Vtk => "vtk",
            Self::Md => "md",
        }
    }
}

impl FromStr for ExportFormat {
    type Err = CaeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "csv" => Ok(Self::Csv),
            "vtk" => Ok(Self::Vtk),
            "md" => Ok(Self::Md),
            "inp" | "abaqus" | "step" | "stp" => Err(not_implemented(&format!("`{value}` format export"))),
            other => Err(CaeError::new(
                "cli/unknown-format",
                format!("unknown export format `{other}`; only csv, vtk or md is supported"),
            )),
        }
    }
}

/// Report returned by [`OpenCae::export`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// Format that was written.
    pub format: String,
    /// Target path exactly as the caller supplied it.
    pub path: String,
    /// Concrete files created by the export.
    pub files: Vec<String>,
}

/// Solved report returned by [`OpenCae::report`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Report format tag, always `open-cae-truss-report`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Document the solution was produced from.
    pub document: DocumentSummary,
    /// Warnings and notices; an error would have stopped the solve.
    pub issues: Vec<Issue>,
    /// Linear-static solution.
    pub solution: Solution,
    /// Lowest five modal results when the material density is positive.
    pub modal: Option<ModalSolution>,
}

/// One open `.ocae` document.
#[derive(Debug, Clone)]
struct Document {
    path: PathBuf,
    model: Model,
}

impl Document {
    fn new(path: &Path, model: Model) -> Self {
        Self { path: path.to_path_buf(), model }
    }

    fn summary(&self) -> DocumentSummary {
        let model = &self.model;
        DocumentSummary {
            id: self.path.display().to_string(),
            path: self.path.display().to_string(),
            units: model.units,
            dimension: model.dimension,
            revision: model.revision,
            nodes: model.nodes.len(),
            elements: model.elements.len(),
            supports: model.supports.len(),
            loads: model.loads.len(),
            material: model.material.clone(),
        }
    }
}

/// Editing session over one `.ocae` document.
///
/// ```no_run
/// use std::path::Path;
///
/// use cae_model::{OpenCae, OpenCaeOptions};
///
/// let mut session = OpenCae::new(OpenCaeOptions::default());
/// session.create(Path::new("demo.ocae"), OpenCaeOptions::default()).unwrap();
/// session.add_node("n1", 0.0, 0.0, None).unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct OpenCae {
    defaults: OpenCaeOptions,
    document: Option<Document>,
}

impl OpenCae {
    /// Creates a session whose [`OpenCae::create`] calls fall back to `options`.
    ///
    /// Constructing a session opens no file.
    pub fn new(options: OpenCaeOptions) -> Self {
        Self { defaults: options, document: None }
    }

    /// Session defaults used when [`OpenCae::create`] omits a value.
    pub fn options(&self) -> &OpenCaeOptions {
        &self.defaults
    }

    /// Stable machine-readable capability matrix of this build.
    pub fn capabilities(&self) -> &'static str {
        CAPABILITIES_JSON
    }

    /// Creates an empty document at `path`, refusing to replace an existing
    /// file, and makes it the open document of this session.
    pub fn create(&mut self, path: &Path, options: OpenCaeOptions) -> Result<DocumentSummary, CaeError> {
        if path.exists() {
            return Err(CaeError::new(
                "file/exists",
                format!("file `{}` already exists; delete it or choose another path", path.display()),
            ));
        }
        let options = self.defaults.merged_with(options);
        let units = options.units.unwrap_or(Units::Millimeter);
        let dimension = options.dimension.unwrap_or(2);
        if dimension != 2 && dimension != 3 {
            return Err(CaeError::new("document/dimension", format!("document dimension must be 2 or 3, but is {dimension}")));
        }
        let model = Model::new_with_dimension(units, dimension);
        model.save_atomic(path)?;
        let document = Document::new(path, model);
        let summary = document.summary();
        self.document = Some(document);
        Ok(summary)
    }

    /// Opens `path` and makes it the open document of this session.
    pub fn open(&mut self, path: &Path) -> Result<DocumentSummary, CaeError> {
        let model = Model::load(path)?;
        let document = Document::new(path, model);
        let summary = document.summary();
        self.document = Some(document);
        Ok(summary)
    }

    /// Writes the open document to `target`, or back to its own path when
    /// `target` is `None`, and updates the session to the written path.
    pub fn save(&mut self, target: Option<&Path>) -> Result<DocumentSummary, CaeError> {
        let path = target.map(Path::to_path_buf);
        let document = self.current_mut()?;
        let path = path.unwrap_or_else(|| document.path.clone());
        document.model.save_atomic(&path)?;
        document.path = path;
        Ok(document.summary())
    }

    /// Summary of the open document.
    pub fn summary(&self) -> Result<DocumentSummary, CaeError> {
        Ok(self.current()?.summary())
    }

    /// Appends a node and saves the document.
    ///
    /// A non-zero `z` is rejected while the document is two-dimensional.
    pub fn add_node(&mut self, id: &str, x: f64, y: f64, z: Option<f64>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.add_node_3d(id, x, y, z.unwrap_or(0.0)))
    }

    /// Appends a truss element and saves the document.
    ///
    /// `youngs_modulus` and `area` are optional per-element overrides of the
    /// global material.
    pub fn add_element(
        &mut self,
        id: &str,
        from: &str,
        to: &str,
        youngs_modulus: Option<f64>,
        area: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            let material = if youngs_modulus.is_some() || area.is_some() {
                Some(crate::ElementMaterial { youngs_modulus, area })
            }
            else {
                None
            };
            model.add_element(id, from, to, material)
        })
    }

    /// Appends a truss or beam element and saves the document.
    ///
    /// Beam elements require `kind = ElementKind::Beam`, a two-dimensional
    /// document and a positive `inertia`. `distributed_load` is a uniform
    /// transverse load per unit length in the element's local `+y` direction
    /// and is only valid on beams.
    #[allow(clippy::too_many_arguments)]
    pub fn add_element_full(
        &mut self,
        id: &str,
        from: &str,
        to: &str,
        youngs_modulus: Option<f64>,
        area: Option<f64>,
        kind: ElementKind,
        inertia: Option<f64>,
        distributed_load: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            let material = if youngs_modulus.is_some() || area.is_some() {
                Some(crate::ElementMaterial { youngs_modulus, area })
            }
            else {
                None
            };
            model.add_element_kind(id, from, to, material, kind, inertia, distributed_load)
        })
    }

    /// Replaces the global material and section data and saves the document.
    pub fn set_material(&mut self, youngs_modulus: f64, area: f64, name: Option<String>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            model.set_material(name, youngs_modulus, area);
            Ok(())
        })
    }

    /// Replaces the global material data including the mass density.
    pub fn set_material_full(
        &mut self,
        youngs_modulus: f64,
        area: f64,
        density: f64,
        name: Option<String>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            model.set_material_full(name, youngs_modulus, area, density);
            Ok(())
        })
    }

    /// Sets the gravity acceleration vector and saves the document.
    ///
    /// The zero vector clears gravity.
    pub fn set_gravity(&mut self, gx: f64, gy: f64, gz: f64) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_gravity(gx, gy, gz))
    }

    /// Adds or merges a support entry and saves the document.
    ///
    /// At least one of `dx`, `dy` or `dz` must constrain a degree of freedom.
    pub fn add_support(&mut self, node: &str, dx: bool, dy: bool, dz: bool) -> Result<DocumentSummary, CaeError> {
        if !dx && !dy && !dz {
            return Err(CaeError::new(
                "support/no-freedom",
                "specify at least one of dx, dy or dz; otherwise the support constrains no degree of freedom",
            ));
        }
        self.edit(|model| model.add_support_3d(node, dx, dy, dz))
    }

    /// Adds or merges a support entry, optionally constraining `rz`, and saves
    /// the document.
    ///
    /// `rz` is only meaningful at nodes touched by a beam element.
    pub fn add_support_full(
        &mut self,
        node: &str,
        dx: bool,
        dy: bool,
        dz: bool,
        rz: bool,
    ) -> Result<DocumentSummary, CaeError> {
        if !dx && !dy && !dz && !rz {
            return Err(CaeError::new(
                "support/no-freedom",
                "specify at least one of dx, dy, dz or rz; otherwise the support constrains no degree of freedom",
            ));
        }
        self.edit(|model| model.add_support_full(node, dx, dy, dz, rz))
    }

    /// Adds or updates a concentrated load and saves the document.
    pub fn add_load(
        &mut self,
        node: &str,
        fx: Option<f64>,
        fy: Option<f64>,
        fz: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.add_load_3d(node, fx, fy, fz))
    }

    /// Adds or updates a concentrated load including an optional moment `mz`.
    pub fn add_load_full(
        &mut self,
        node: &str,
        fx: Option<f64>,
        fy: Option<f64>,
        fz: Option<f64>,
        mz: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.add_load_full(node, fx, fy, fz, mz))
    }

    /// Validates the open document without solving it.
    pub fn validate(&self) -> Result<ValidationReport, CaeError> {
        let issues = self.current()?.model.validate();
        let ok = !issues.iter().any(|issue| issue.severity == Severity::Error);
        Ok(ValidationReport { ok, issues })
    }

    /// Solves the open document.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn solve(&self, out: Option<&Path>) -> Result<Solution, CaeError> {
        let model = &self.current()?.model;
        let solution = solve(model)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&solution)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize result: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(solution)
    }

    /// Solves the open document for `modes` natural frequencies.
    ///
    /// When `out` is given the JSON modal report is written atomically to that
    /// path.
    pub fn solve_modal(&self, modes: usize, out: Option<&Path>) -> Result<ModalSolution, CaeError> {
        let model = &self.current()?.model;
        let modal = solve_modal(model, modes)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&modal)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize modal result: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(modal)
    }

    /// Solves the open document and exports the result.
    pub fn export(&self, format: ExportFormat, target: &Path) -> Result<ExportReport, CaeError> {
        let model = &self.current()?.model;
        let solution = solve(model)?;
        let files = match format {
            ExportFormat::Csv => {
                export_csv(model, &solution, target)?;
                vec![target.join("nodes.csv").display().to_string(), target.join("elements.csv").display().to_string()]
            }
            ExportFormat::Vtk => {
                export_vtk(model, &solution, target)?;
                let file = if target.extension().and_then(|extension| extension.to_str()) == Some("vtk") {
                    target.to_path_buf()
                }
                else {
                    target.join("model.vtk")
                };
                vec![file.display().to_string()]
            }
            ExportFormat::Md => {
                let modal = if positive_density(model) { Some(solve_modal(model, 5)?) } else { None };
                let issues = model.validate();
                export_md(model, &solution, modal.as_ref(), &issues, target)?;
                let file = if target.extension().and_then(|extension| extension.to_str()) == Some("md") {
                    target.to_path_buf()
                }
                else {
                    target.join("report.md")
                };
                vec![file.display().to_string()]
            }
        };
        Ok(ExportReport { format: format.as_str().to_string(), path: target.display().to_string(), files })
    }

    /// Solves the open document and returns the full report.
    pub fn report(&self) -> Result<Report, CaeError> {
        let document = self.current()?;
        let solution = solve(&document.model)?;
        let modal = if positive_density(&document.model) { Some(solve_modal(&document.model, 5)?) } else { None };
        Ok(Report {
            format: REPORT_FORMAT.to_string(),
            version: REPORT_VERSION,
            document: document.summary(),
            issues: document.model.validate(),
            solution,
            modal,
        })
    }

    fn current(&self) -> Result<&Document, CaeError> {
        self.document
            .as_ref()
            .ok_or_else(|| CaeError::new("session/no-document", "this session has no open document; call create or open first"))
    }

    fn current_mut(&mut self) -> Result<&mut Document, CaeError> {
        self.document
            .as_mut()
            .ok_or_else(|| CaeError::new("session/no-document", "this session has no open document; call create or open first"))
    }

    fn edit<F>(&mut self, work: F) -> Result<DocumentSummary, CaeError>
    where
        F: FnOnce(&mut Model) -> Result<(), CaeError>,
    {
        let document = self.current_mut()?;
        work(&mut document.model)?;
        document.model.save_atomic(&document.path)?;
        Ok(document.summary())
    }
}

fn positive_density(model: &Model) -> bool {
    model.material.density.is_finite() && model.material.density > 0.0
}

/// Stable error for a capability this build does not implement.
///
/// The code is always `capability/not-implemented`, which the CLI maps to exit
/// code `2`.
pub fn not_implemented(feature: &str) -> CaeError {
    CaeError::new(
        "capability/not-implemented",
        format!(
            "{feature} is not implemented; this build supports 2D truss/beam, 3D truss, linear static, modal and self-weight analysis (truss2d/beam2d/truss3d/modal)"
        ),
    )
}
