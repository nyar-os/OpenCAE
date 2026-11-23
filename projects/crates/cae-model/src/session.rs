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
    envelope::{EnvelopeReport, solve_envelope},
    export::{
        ContourQuantity, export_csv_with_stations, export_html, export_md, export_svg_with_contour, export_vtk, write_atomic,
    },
    library::material_preset,
    model::{ElementKind, ElementSplit, Material, Model, PatternFactor, Section, Units},
    solver::{
        BucklingCombinationSolution, BucklingSolution, CombinationSolution, ConstructionStage, ConstructionStageSolution,
        DeflectionSolution, ElementResult, FrequencyResponseSolution, HeatSolution, InfluenceDirection, InfluenceLineSolution,
        InfluenceQuantity, InfluenceTrainRequest, InfluenceTrainSolution, InternalForceSolution, LoadCaseSummary,
        ModalSolution, ModelSummaryReport, NodalStressSolution, NodeDisplacement, NonlinearSolution, REPORT_FORMAT,
        REPORT_VERSION, ReactionSummary, ResponseSpectrumSolution, Solution, SpectrumCombination, SpectrumPoint,
        StressSolution, SupportReaction, SystemMatrices, TensionOnlySolution, solve, solve_buckling,
        solve_buckling_combinations, solve_combinations, solve_construction_stages, solve_deflection, solve_frequency_response,
        solve_heat, solve_influence_line, solve_influence_line_train, solve_internal_forces, solve_matrices, solve_modal,
        solve_nodal_stress, solve_nonlinear, solve_response_spectrum, solve_stress, solve_tension_only, summarize_load_cases,
        summarize_model,
    },
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

/// Result of [`OpenCae::split_element`].
///
/// The document summary fields are flattened so the revision and link counts are
/// read exactly like the summary returned by every other edit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementSplitReport {
    /// Summary of the document as written to disk.
    #[serde(flatten)]
    pub summary: DocumentSummary,
    /// Identifiers produced by the split.
    #[serde(flatten)]
    pub split: ElementSplit,
}

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Three CSV files (`nodes.csv`, `elements.csv`, `reactions.csv`) inside a directory.
    Csv,
    /// One legacy ASCII VTK `UNSTRUCTURED_GRID` file.
    Vtk,
    /// One deterministic Markdown engineering report.
    Md,
    /// One deterministic, self-contained HTML engineering report.
    Html,
    /// One deterministic SVG drawing of the model and deformed shape.
    Svg,
}

impl ExportFormat {
    /// Returns the short identifier used by the CLI and by reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Vtk => "vtk",
            Self::Md => "md",
            Self::Html => "html",
            Self::Svg => "svg",
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
            "html" | "htm" => Ok(Self::Html),
            "svg" => Ok(Self::Svg),
            "inp" | "abaqus" | "step" | "stp" => Err(not_implemented(&format!("`{value}` format export"))),
            other => Err(CaeError::new(
                "cli/unknown-format",
                format!("unknown export format `{other}`; only csv, vtk, md, html or svg is supported"),
            )),
        }
    }
}

/// Options of [`OpenCae::export_with_options`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportOptions {
    /// Scale factor applied to nodal displacements when drawing the deformed
    /// shape of an SVG export. Must be finite and positive; defaults to `1.0`.
    pub scale: f64,
    /// Stations sampled per member in the `forces.csv` table of a CSV export.
    ///
    /// `None` keeps the default of 11 stations. The value counts both element
    /// ends and must be at least `2`; other formats ignore it.
    pub stations: Option<usize>,
    /// Result quantity that an SVG export colours the members by.
    ///
    /// `None` keeps the plain deformed-shape drawing. A quantity turns the SVG
    /// into a contour drawing with a colour bar; every other format rejects the
    /// option with `export/invalid-contour`.
    pub contour: Option<ContourQuantity>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self { scale: 1.0, stations: None, contour: None }
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

    /// Moves an existing node and saves the document.
    ///
    /// Only the coordinates supplied as `Some` change; an empty request is
    /// rejected with `node/invalid-coordinate`.
    pub fn update_node(
        &mut self,
        id: &str,
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.update_node(id, x, y, z))
    }

    /// Sets or clears the concentrated mass of a node and saves the document.
    ///
    /// The mass must be finite and non-negative; `0` clears it. Unknown nodes
    /// are rejected with `node/unknown-id` and invalid values with
    /// `node/invalid-mass`.
    pub fn set_node_mass(&mut self, id: &str, mass: f64) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_node_mass(id, mass))
    }

    /// Sets or clears the fixed temperature of a node and saves the document.
    ///
    /// `None` removes the Dirichlet boundary again. The value is used only by
    /// [`OpenCae::solve_heat`].
    pub fn set_node_temperature(&mut self, id: &str, temperature: Option<f64>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_node_temperature(id, temperature))
    }

    /// Sets the concentrated nodal heat flow and saves the document.
    ///
    /// `0` removes the heat load. The value is used only by
    /// [`OpenCae::solve_heat`].
    pub fn set_node_heat(&mut self, id: &str, heat: f64) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_node_heat(id, heat))
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
        self.add_element_tapered(id, from, to, youngs_modulus, area, kind, inertia, distributed_load, None)
    }

    /// Appends a truss or beam element with a linearly varying distributed
    /// load and saves the document.
    ///
    /// `distributed_load` is the transverse load per unit length at the start
    /// node and `distributed_load_end` the value at the end node, both in the
    /// element's local `+y` direction. Beam requirements match
    /// [`OpenCae::add_element_full`].
    #[allow(clippy::too_many_arguments)]
    pub fn add_element_tapered(
        &mut self,
        id: &str,
        from: &str,
        to: &str,
        youngs_modulus: Option<f64>,
        area: Option<f64>,
        kind: ElementKind,
        inertia: Option<f64>,
        distributed_load: Option<f64>,
        distributed_load_end: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.add_element_thermal(
            id,
            from,
            to,
            youngs_modulus,
            area,
            kind,
            inertia,
            distributed_load,
            distributed_load_end,
            None,
        )
    }

    /// Appends a truss or beam element with a uniform temperature change and
    /// saves the document.
    ///
    /// `temperature` is the uniform temperature change `ΔT` of the member; the
    /// material must declare `alpha` when it is supplied. All other arguments
    /// match [`OpenCae::add_element_tapered`].
    #[allow(clippy::too_many_arguments)]
    pub fn add_element_thermal(
        &mut self,
        id: &str,
        from: &str,
        to: &str,
        youngs_modulus: Option<f64>,
        area: Option<f64>,
        kind: ElementKind,
        inertia: Option<f64>,
        distributed_load: Option<f64>,
        distributed_load_end: Option<f64>,
        temperature: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            let material = if youngs_modulus.is_some() || area.is_some() {
                Some(crate::ElementMaterial { youngs_modulus, area })
            }
            else {
                None
            };
            model.add_element_with_loads(id, from, to, material, kind, inertia, distributed_load, distributed_load_end)?;
            if temperature.is_some() {
                model.set_element_temperature(id, temperature)?;
            }
            Ok(())
        })
    }

    /// Sets or clears the uniform temperature change of an existing element and
    /// saves the document.
    ///
    /// `None` removes the thermal load again.
    pub fn set_element_temperature(&mut self, id: &str, temperature: Option<f64>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_element_temperature(id, temperature))
    }

    /// Sets or clears the surface convection coefficient of an existing element
    /// and saves the document.
    ///
    /// The coefficient uses `W/(m²·K)` and is used only by
    /// [`OpenCae::solve_heat`]; `None` removes it. A non-positive or non-finite
    /// value is rejected with `heat/invalid-convection`.
    pub fn set_element_convection(&mut self, id: &str, convection: Option<f64>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_element_convection(id, convection))
    }

    /// Sets or clears the exposed perimeter of an existing element and saves the
    /// document.
    ///
    /// The perimeter uses the document length unit and is required when the
    /// element declares convection. A non-positive or non-finite value is
    /// rejected with `heat/invalid-perimeter`.
    pub fn set_element_perimeter(&mut self, id: &str, perimeter: Option<f64>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_element_perimeter(id, perimeter))
    }

    /// Reconnects an existing element to new endpoint nodes and saves the
    /// document.
    ///
    /// Only the endpoints supplied as `Some` change; an empty request is
    /// rejected with `element/degenerate`.
    pub fn update_element(&mut self, id: &str, from: Option<&str>, to: Option<&str>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.update_element_endpoints(id, from, to))
    }

    /// Applies a partial [`crate::ElementUpdate`] to an existing element and
    /// saves the document.
    ///
    /// Only the named endpoints, load, temperature or convection values change,
    /// and the call bumps the revision exactly once. Conflicting clear flags and
    /// an empty request are rejected with `session/argument-invalid`.
    pub fn update_element_with(&mut self, id: &str, update: &crate::ElementUpdate) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.update_element_with(id, update))
    }

    /// Splits an existing element at the fraction `at` of its length and saves
    /// the document.
    ///
    /// The split point is `from + at · (to − from)`; an existing node there is
    /// reused, otherwise one is created. Both segments keep the material,
    /// section data, uniform distributed load, uniform temperature and
    /// tension-only flag, and the end releases move to the outer ends. The
    /// fraction must be finite and strictly inside `(0, 1)`. Partial-span loads,
    /// linearly varying distributed loads and through-depth temperature
    /// gradients are rejected with `element/split-unsupported-load` because they
    /// cannot be divided losslessly. The report names the interior node and both
    /// element ids and carries the new revision.
    pub fn split_element(&mut self, id: &str, at: f64) -> Result<ElementSplitReport, CaeError> {
        let document = self.current_mut()?;
        let split = document.model.split_element(id, at)?;
        document.model.save_atomic(&document.path)?;
        Ok(ElementSplitReport { summary: document.summary(), split })
    }

    /// Sets or clears the loaded segment of an existing beam's distributed
    /// load and saves the document.
    ///
    /// `span` holds the start and end fractions of the element length; `None`
    /// restores a load over the whole member. Both fractions must be supplied
    /// together.
    pub fn set_element_load_span(&mut self, id: &str, span: Option<(f64, f64)>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_element_load_span(id, span))
    }

    /// Sets or clears the linear temperature gradient of an existing beam and
    /// saves the document.
    ///
    /// `gradient` is the temperature difference between the local `+y` and `-y`
    /// extreme fibres; `None` removes it again. The material must declare
    /// `alpha` and a section depth before the model can be solved.
    pub fn set_element_temperature_gradient(&mut self, id: &str, gradient: Option<f64>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_element_temperature_gradient(id, gradient))
    }

    /// Sets or clears the bending-moment releases of an existing beam and saves
    /// the document.
    ///
    /// `release` frees the start and/or end bending rotation; `None` restores a
    /// fully continuous member. Truss elements cannot be released.
    pub fn set_element_release(&mut self, id: &str, release: Option<crate::EndRelease>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_element_release(id, release))
    }

    /// Sets or clears the tension-only (cable) flag of an existing truss and
    /// saves the document.
    ///
    /// Beam elements are rejected with `element/tension-only-requires-truss`.
    pub fn set_element_tension_only(&mut self, id: &str, tension_only: bool) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_element_tension_only(id, tension_only))
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

    /// Replaces the complete global material, section and allowable-stress data.
    ///
    /// The material is validated before the document is written.
    pub fn set_material_data(&mut self, material: Material) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_material_data(material))
    }

    /// Replaces or clears the parametric cross-section of the global material.
    pub fn set_section(&mut self, section: Option<Section>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_section(section))
    }

    /// Applies a built-in material preset and saves the document.
    ///
    /// The area comes from `area` when supplied, otherwise from the section
    /// geometry, otherwise the existing document area is kept. A section is
    /// written when supplied and otherwise preserved. `allowable_stress`
    /// overrides the preset's permissible stress.
    pub fn apply_material_preset(
        &mut self,
        name: &str,
        area: Option<f64>,
        section: Option<Section>,
        allowable_stress: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        let preset = material_preset(name)?;
        let material = {
            let document = self.current()?;
            let units = document.model.units.unwrap_or(Units::Millimeter);
            let section = section.or(document.model.material.section);
            let area = match area {
                Some(area) => area,
                None => match section {
                    Some(section) => section.properties()?.area,
                    None => document.model.material.area,
                },
            };
            let mut material = preset.material(units, area, section);
            material.alpha = document.model.material.alpha;
            material.conductivity = document.model.material.conductivity;
            if let Some(stress) = allowable_stress {
                material.allowable_stress = Some(stress);
            }
            material
        };
        self.edit(|model| model.set_material_data(material))
    }

    /// Sets the gravity acceleration vector and saves the document.
    ///
    /// The zero vector clears gravity.
    pub fn set_gravity(&mut self, gx: f64, gy: f64, gz: f64) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_gravity(gx, gy, gz))
    }

    /// Sets or clears the ambient temperature used by surface convection and
    /// saves the document.
    ///
    /// The value is finite and uses the same scale as the fixed nodal
    /// temperatures; `None` removes it.
    pub fn set_ambient_temperature(&mut self, temperature: Option<f64>) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.set_ambient_temperature(temperature))
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

    /// Adds or merges an inclined roller support and saves the document.
    ///
    /// `direction = (nx, ny)` is the constrained displacement direction in the
    /// X–Y plane and is normalised by the solver; the document must be
    /// two-dimensional. It cannot be combined with a global constraint,
    /// prescribed displacement or spring on the same entry.
    pub fn add_support_direction(&mut self, node: &str, nx: f64, ny: f64) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.add_support_direction(node, nx, ny))
    }

    /// Sets or merges linear elastic spring stiffness at a support node and
    /// saves the document.
    ///
    /// A spring is a flexible support: the matching degree of freedom stays
    /// free and the stiffness is added to the global matrix. At least one
    /// component must be supplied.
    pub fn set_spring(
        &mut self,
        node: &str,
        kx: Option<f64>,
        ky: Option<f64>,
        kz: Option<f64>,
        kr: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        if kx.is_none() && ky.is_none() && kz.is_none() && kr.is_none() {
            return Err(CaeError::new(
                "support/no-freedom",
                "specify at least one of kx, ky, kz or kr; otherwise the spring carries no stiffness",
            ));
        }
        self.edit(|model| model.set_spring(node, kx, ky, kz, kr))
    }

    /// Prescribes (enforces) non-zero displacement values at a support node and
    /// saves the document.
    ///
    /// Supplying a component constrains the matching degree of freedom, so
    /// `prescribe_displacement("n2", Some(0.5), None, None, None)` creates or
    /// updates a support with `dx = true` and `ux = 0.5`.
    pub fn prescribe_displacement(
        &mut self,
        node: &str,
        ux: Option<f64>,
        uy: Option<f64>,
        uz: Option<f64>,
        rz: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        if ux.is_none() && uy.is_none() && uz.is_none() && rz.is_none() {
            return Err(CaeError::new(
                "support/no-freedom",
                "specify at least one of ux, uy, uz or rz; otherwise no displacement is prescribed",
            ));
        }
        self.edit(|model| model.prescribe_displacement(node, ux, uy, uz, rz))
    }

    /// Adds or replaces a linear load combination and saves the document.
    ///
    /// `loads` scales nodal and element applied loads and `gravity` scales the
    /// gravity vector. An existing combination with the same name is updated in
    /// place.
    pub fn add_combination(&mut self, name: &str, loads: f64, gravity: f64) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.add_combination(name, loads, gravity))
    }

    /// Adds or replaces a load combination that also mixes named load patterns
    /// and saves the document.
    ///
    /// `patterns` replaces the whole pattern list of an existing combination.
    pub fn add_combination_with_patterns(
        &mut self,
        name: &str,
        loads: f64,
        gravity: f64,
        patterns: &[PatternFactor],
    ) -> Result<DocumentSummary, CaeError> {
        self.add_combination_full(name, loads, gravity, 1.0, patterns)
    }

    /// Adds or replaces a load combination with load, gravity, thermal and
    /// named-pattern factors and saves the document.
    ///
    /// `thermal` scales element temperature changes and gradients, and
    /// `patterns` replaces the whole pattern list of an existing combination.
    pub fn add_combination_full(
        &mut self,
        name: &str,
        loads: f64,
        gravity: f64,
        thermal: f64,
        patterns: &[PatternFactor],
    ) -> Result<DocumentSummary, CaeError> {
        let patterns = patterns.to_vec();
        self.edit(move |model| model.add_combination_full(name, loads, gravity, thermal, patterns))
    }

    /// Adds or updates a concentrated load inside a named load pattern and
    /// saves the document.
    ///
    /// The pattern is created when it does not exist yet.
    pub fn add_pattern_load(
        &mut self,
        pattern: &str,
        node: &str,
        fx: Option<f64>,
        fy: Option<f64>,
        fz: Option<f64>,
        mz: Option<f64>,
    ) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.add_pattern_load(pattern, node, fx, fy, fz, mz))
    }

    /// Removes a whole load pattern by name and saves the document.
    ///
    /// A missing pattern is rejected with `pattern/unknown-pattern`.
    pub fn remove_pattern(&mut self, name: &str) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            if model.remove_pattern(name) {
                Ok(())
            }
            else {
                Err(CaeError::new("pattern/unknown-pattern", format!("no load pattern `{name}` exists")))
            }
        })
    }

    /// Removes one nodal load entry from a load pattern and saves the document.
    ///
    /// A missing pattern or load entry is rejected with `load/unknown-load`.
    pub fn remove_pattern_load(&mut self, pattern: &str, node: &str) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            if model.remove_pattern_load(pattern, node) {
                Ok(())
            }
            else {
                Err(CaeError::new("load/unknown-load", format!("load pattern `{pattern}` has no load entry for node `{node}`")))
            }
        })
    }

    /// Removes a load combination by name and saves the document.
    ///
    /// Returns `false` when no combination has that name.
    pub fn remove_combination(&mut self, name: &str) -> Result<bool, CaeError> {
        let removed = self.current()?.model.combinations.iter().any(|combination| combination.name == name);
        if removed {
            self.edit(|model| {
                model.remove_combination(name);
                Ok(())
            })?;
        }
        Ok(removed)
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

    /// Removes a node and saves the document.
    ///
    /// The node must not be referenced by an element, a support or a load;
    /// removing a referenced node is rejected with `node/referenced-by-*`.
    pub fn remove_node(&mut self, id: &str) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.remove_node(id))
    }

    /// Merges one node into another and saves the document.
    ///
    /// The node `into` survives with its coordinates; elements, supports and
    /// loads of `from` are re-pointed, combined or moved. Conflicting supports
    /// and an element that would collapse are rejected without writing.
    pub fn merge_nodes(&mut self, from: &str, into: &str) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.merge_nodes(from, into))
    }

    /// Removes an element and saves the document.
    pub fn remove_element(&mut self, id: &str) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| model.remove_element(id))
    }

    /// Removes the support entry of a node and saves the document.
    ///
    /// A node without a support entry is rejected with
    /// `support/unknown-support`.
    pub fn remove_support(&mut self, node: &str) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            if model.remove_support(node) {
                Ok(())
            }
            else {
                Err(CaeError::new("support/unknown-support", format!("node `{node}` has no support entry to remove")))
            }
        })
    }

    /// Removes the concentrated load entry of a node and saves the document.
    ///
    /// A node without a load entry is rejected with `load/unknown-load`.
    pub fn remove_load(&mut self, node: &str) -> Result<DocumentSummary, CaeError> {
        self.edit(|model| {
            if model.remove_load(node) {
                Ok(())
            }
            else {
                Err(CaeError::new("load/unknown-load", format!("node `{node}` has no load entry to remove")))
            }
        })
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

    /// Solves steady-state heat conduction through the open document.
    ///
    /// When `out` is given the JSON heat report is written atomically to that
    /// path. A failed analysis writes nothing and leaves the document unchanged.
    pub fn solve_heat(&self, out: Option<&Path>) -> Result<HeatSolution, CaeError> {
        let model = &self.current()?.model;
        let heat = solve_heat(model)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&heat)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize heat result: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(heat)
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

    /// Solves the open document for `modes` linear buckling load factors.
    ///
    /// Each factor multiplies the static load case of the same document. When
    /// `out` is given the JSON buckling report is written atomically to that
    /// path.
    pub fn solve_buckling(&self, modes: usize, out: Option<&Path>) -> Result<BucklingSolution, CaeError> {
        let model = &self.current()?.model;
        let buckling = solve_buckling(model, modes)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&buckling)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize buckling result: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(buckling)
    }

    /// Solves the linear buckling eigenproblem for every stored load
    /// combination and returns the individual factors plus the governing
    /// combination.
    ///
    /// Each factor multiplies that combination's load set. When `out` is given
    /// the JSON report is written atomically to that path.
    pub fn solve_buckling_combinations(
        &self,
        modes: usize,
        out: Option<&Path>,
    ) -> Result<BucklingCombinationSolution, CaeError> {
        let model = &self.current()?.model;
        let result = solve_buckling_combinations(model, &model.combinations, modes)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&result).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize buckling combination result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(result)
    }

    /// Solves the open document and samples internal forces at `stations`
    /// equally spaced points per element.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn internal_forces(&self, stations: usize, out: Option<&Path>) -> Result<InternalForceSolution, CaeError> {
        let model = &self.current()?.model;
        let forces = solve_internal_forces(model, stations)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&forces).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize internal force result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(forces)
    }

    /// Returns the support reactions and the global force equilibrium check.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn reactions(&self, out: Option<&Path>) -> Result<ReactionSummary, CaeError> {
        let summary = self.solve(None)?.reaction_summary;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize reaction summary: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(summary)
    }

    /// Returns the solved displacement of one node by identifier.
    ///
    /// An identifier that is not part of the model is rejected with
    /// `result/unknown-node`.
    pub fn node_displacement(&self, id: &str) -> Result<NodeDisplacement, CaeError> {
        let solution = self.solve(None)?;
        solution
            .nodes
            .into_iter()
            .find(|node| node.id == id)
            .ok_or_else(|| CaeError::new("result/unknown-node", format!("the solution has no node `{id}`")))
    }

    /// Returns the solved member result of one element by identifier.
    ///
    /// The result carries the axial force/stress and the local end forces. An
    /// identifier that is not part of the model is rejected with
    /// `result/unknown-element`.
    pub fn element_result(&self, id: &str) -> Result<ElementResult, CaeError> {
        let solution = self.solve(None)?;
        solution
            .elements
            .into_iter()
            .find(|element| element.id == id)
            .ok_or_else(|| CaeError::new("result/unknown-element", format!("the solution has no element `{id}`")))
    }

    /// Returns the support reaction of one node by identifier.
    ///
    /// A node without a support entry is rejected with `result/no-reaction`.
    pub fn node_reaction(&self, node: &str) -> Result<SupportReaction, CaeError> {
        let summary = self.reactions(None)?;
        summary
            .supports
            .into_iter()
            .find(|support| support.node == node)
            .ok_or_else(|| CaeError::new("result/no-reaction", format!("node `{node}` has no support reaction")))
    }

    /// Recovers extreme-fibre stresses from the sampled internal forces.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn stresses(&self, stations: usize, out: Option<&Path>) -> Result<StressSolution, CaeError> {
        let model = &self.current()?.model;
        let stresses = solve_stress(model, stations)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&stresses)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize stress result: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(stresses)
    }

    /// Samples the deflected centreline of every element at `stations` equally
    /// spaced points.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn deflections(&self, stations: usize, out: Option<&Path>) -> Result<DeflectionSolution, CaeError> {
        let model = &self.current()?.model;
        let deflections = solve_deflection(model, stations)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&deflections)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize deflection result: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(deflections)
    }

    /// Builds an influence line as a unit load moves along a track beam element.
    ///
    /// The model must not carry any other load. When `out` is given the JSON
    /// report is written atomically to that path.
    pub fn influence_line(
        &self,
        track: &str,
        quantity: &InfluenceQuantity,
        direction: InfluenceDirection,
        points: usize,
        out: Option<&Path>,
    ) -> Result<InfluenceLineSolution, CaeError> {
        let model = &self.current()?.model;
        let solution = solve_influence_line(model, track, quantity, direction, points)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&solution).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize influence line result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(solution)
    }

    /// Envelopes a moving load train over the influence line of [`OpenCae::influence_line`].
    ///
    /// The influence line is sampled once and reused; the structure is never
    /// re-solved per train position. The model must not carry any other load.
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn influence_line_train(
        &self,
        request: &InfluenceTrainRequest,
        out: Option<&Path>,
    ) -> Result<InfluenceTrainSolution, CaeError> {
        let model = &self.current()?.model;
        let solution = solve_influence_line_train(model, request)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&solution).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize influence line train result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(solution)
    }

    /// Averages element-end stresses at shared nodes and reports the extremes.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn nodal_stresses(&self, stations: usize, out: Option<&Path>) -> Result<NodalStressSolution, CaeError> {
        let model = &self.current()?.model;
        let stresses = solve_nodal_stress(model, stations)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&stresses).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize nodal stress result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(stresses)
    }

    /// Summarises the unfactored static case and every stored load combination.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn load_cases(&self, out: Option<&Path>) -> Result<LoadCaseSummary, CaeError> {
        let model = &self.current()?.model;
        let summary = summarize_load_cases(model)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize load-case summary: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(summary)
    }

    /// Reports member quantities, mass and degree-of-freedom statistics without
    /// solving the document.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn model_summary(&self, out: Option<&Path>) -> Result<ModelSummaryReport, CaeError> {
        let summary = summarize_model(&self.current()?.model)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize model summary: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(summary)
    }

    /// Solves every load combination stored in the open document and returns
    /// the individual solutions plus their envelope.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn solve_combinations(&self, out: Option<&Path>) -> Result<CombinationSolution, CaeError> {
        let model = &self.current()?.model;
        let combinations = solve_combinations(model, &model.combinations)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&combinations).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize combination result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(combinations)
    }

    /// Solves the unfactored case and every stored load combination, then
    /// returns the minimum and maximum of each displacement, force and reaction
    /// component with the governing combination.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn envelope(&self, out: Option<&Path>) -> Result<EnvelopeReport, CaeError> {
        let model = &self.current()?.model;
        let envelope = solve_envelope(model)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&envelope)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize envelope report: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(envelope)
    }

    /// Solves a response-spectrum analysis in the global `axis` direction
    /// (`0 = x`, `1 = y`, `2 = z`) and combines the modes with `combination`.
    ///
    /// CQC requires `damping`, one modal damping ratio in `(0, 1]`; SRSS
    /// ignores it. When `out` is given the JSON report is written atomically to
    /// that path.
    pub fn response_spectrum(
        &self,
        modes: usize,
        axis: usize,
        spectrum: &[SpectrumPoint],
        combination: SpectrumCombination,
        damping: Option<f64>,
        out: Option<&Path>,
    ) -> Result<ResponseSpectrumSolution, CaeError> {
        let model = &self.current()?.model;
        let result = solve_response_spectrum(model, modes, axis, spectrum, combination, damping)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&result).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize response-spectrum result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(result)
    }

    /// Solves the steady-state harmonic response over a frequency sweep by
    /// modal superposition.
    ///
    /// Frequencies are in hertz and `damping_ratio` applies to every mode. When
    /// `out` is given the JSON report is written atomically to that path.
    #[allow(clippy::too_many_arguments)]
    pub fn frequency_response(
        &self,
        modes: usize,
        axis: usize,
        damping_ratio: f64,
        start_hz: f64,
        end_hz: f64,
        points: usize,
        out: Option<&Path>,
    ) -> Result<FrequencyResponseSolution, CaeError> {
        let model = &self.current()?.model;
        let result = solve_frequency_response(model, modes, axis, damping_ratio, start_hz, end_hz, points)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&result).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize frequency-response result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(result)
    }

    /// Solves a sequence of construction stages by activating and deactivating
    /// elements, and returns one summary row per stage.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn construction_stages(
        &self,
        stages: &[ConstructionStage],
        out: Option<&Path>,
    ) -> Result<ConstructionStageSolution, CaeError> {
        let model = &self.current()?.model;
        let result = solve_construction_stages(model, stages)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&result).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize construction-stage result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(result)
    }

    /// Solves the open truss model with a large-displacement Newton iteration.
    ///
    /// `increments` splits the load into equal steps, `max_iterations` bounds
    /// the Newton loop of every step and `tolerance` is the relative residual
    /// target. When `out` is given the JSON report is written atomically to
    /// that path.
    pub fn nonlinear(
        &self,
        increments: usize,
        max_iterations: usize,
        tolerance: f64,
        out: Option<&Path>,
    ) -> Result<NonlinearSolution, CaeError> {
        let model = &self.current()?.model;
        let result = solve_nonlinear(model, increments, max_iterations, tolerance)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&result)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize nonlinear result: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(result)
    }

    /// Solves the open model with tension-only (cable) truss members.
    ///
    /// `max_iterations` bounds the active-set iteration and `tolerance` is the
    /// relative axial-force threshold below which a compressed cable is
    /// considered slack. When `out` is given the JSON report is written
    /// atomically to that path.
    pub fn tension_only(
        &self,
        max_iterations: usize,
        tolerance: f64,
        out: Option<&Path>,
    ) -> Result<TensionOnlySolution, CaeError> {
        let model = &self.current()?.model;
        let result = solve_tension_only(model, max_iterations, tolerance)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&result).map_err(|error| {
                CaeError::new("document/serialize", format!("cannot serialize tension-only result: {error}"))
            })?;
            write_atomic(path, &json)?;
        }
        Ok(result)
    }

    /// Exports the assembled global stiffness and lumped mass matrices.
    ///
    /// When `out` is given the JSON report is written atomically to that path.
    pub fn matrices(&self, out: Option<&Path>) -> Result<SystemMatrices, CaeError> {
        let model = &self.current()?.model;
        let matrices = solve_matrices(model)?;
        if let Some(path) = out {
            let json = serde_json::to_string_pretty(&matrices)
                .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize matrix report: {error}")))?;
            write_atomic(path, &json)?;
        }
        Ok(matrices)
    }

    /// Solves the open document and exports the result.
    pub fn export(&self, format: ExportFormat, target: &Path) -> Result<ExportReport, CaeError> {
        self.export_with_options(format, target, ExportOptions::default())
    }

    /// Solves the open document and exports the result with `options`.
    ///
    /// `options.scale` only affects the SVG format: it multiplies the nodal
    /// displacements of the deformed shape. `options.contour` is also SVG-only
    /// and turns the drawing into a result contour with a colour bar; every
    /// other format rejects it with `export/invalid-contour`.
    pub fn export_with_options(
        &self,
        format: ExportFormat,
        target: &Path,
        options: ExportOptions,
    ) -> Result<ExportReport, CaeError> {
        if options.contour.is_some() && format != ExportFormat::Svg {
            return Err(CaeError::new(
                "export/invalid-contour",
                format!("the contour option only applies to SVG export, not `{}`", format.as_str()),
            ));
        }
        let model = &self.current()?.model;
        let solution = solve(model)?;
        let files = match format {
            ExportFormat::Csv => {
                export_csv_with_stations(model, &solution, target, options.stations.unwrap_or(11))?;
                vec![
                    target.join("nodes.csv").display().to_string(),
                    target.join("elements.csv").display().to_string(),
                    target.join("reactions.csv").display().to_string(),
                    target.join("forces.csv").display().to_string(),
                ]
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
            ExportFormat::Html => {
                let modal = if positive_density(model) { Some(solve_modal(model, 5)?) } else { None };
                let issues = model.validate();
                export_html(model, &solution, modal.as_ref(), &issues, target)?;
                let extension = target.extension().and_then(|extension| extension.to_str());
                let file = if extension == Some("html") || extension == Some("htm") {
                    target.to_path_buf()
                }
                else {
                    target.join("report.html")
                };
                vec![file.display().to_string()]
            }
            ExportFormat::Svg => {
                export_svg_with_contour(
                    model,
                    &solution,
                    options.scale,
                    options.contour,
                    options.stations.unwrap_or(11),
                    target,
                )?;
                let file = if target.extension().and_then(|extension| extension.to_str()) == Some("svg") {
                    target.to_path_buf()
                }
                else {
                    target.join("model.svg")
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
            "{feature} is not implemented; this build supports 2D truss/beam, 3D truss, linear static, modal, self-weight and linear buckling analysis (truss2d/beam2d/truss3d/modal/buckling)"
        ),
    )
}
