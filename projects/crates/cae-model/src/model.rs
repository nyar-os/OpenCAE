use std::{path::Path, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{CaeError, export};

/// Identifier of the document format stored in the `format` field.
pub const FORMAT: &str = "open-cae-truss";

/// Document version understood by this build.
pub const VERSION: u32 = 1;

/// Unit system declared by a document.
///
/// The solver does not convert units. All values must be mutually consistent:
/// with [`Units::Millimeter`] use `N`, `mm`, `mm²` and `MPa`; with
/// [`Units::Meter`] use `N`, `m`, `m²` and `Pa`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Units {
    /// Millimetre / newton / megapascal system.
    #[serde(rename = "mm")]
    #[default]
    Millimeter,
    /// Metre / newton / pascal system.
    #[serde(rename = "m")]
    Meter,
}

impl Units {
    /// Returns the short identifier stored in documents (`"mm"` or `"m"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Millimeter => "mm",
            Self::Meter => "m",
        }
    }
}

impl FromStr for Units {
    type Err = CaeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mm" => Ok(Self::Millimeter),
            "m" => Ok(Self::Meter),
            other => {
                Err(CaeError::new("cli/invalid-units", format!("unsupported units `{other}`; valid values are `mm` or `m`")))
            }
        }
    }
}

/// Parametric cross-section geometry stored with the global material.
///
/// The section is optional so existing documents load unchanged. When present
/// it supplies the second moment of area used by beams that omit their own
/// `inertia` and the extreme-fibre distance used by stress recovery. All
/// dimensions use the document units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape")]
pub enum Section {
    /// Solid rectangle with `width` along the local x axis and `height` along
    /// the local y axis.
    #[serde(rename = "rectangle", rename_all = "camelCase")]
    Rectangle {
        /// Section width.
        width: f64,
        /// Section height (bending depth).
        height: f64,
    },
    /// Solid circular section.
    #[serde(rename = "circle", rename_all = "camelCase")]
    Circle {
        /// Diameter.
        diameter: f64,
    },
    /// Doubly symmetric I-section.
    #[serde(rename = "i", rename_all = "camelCase")]
    ISection {
        /// Total depth.
        depth: f64,
        /// Flange width.
        flange_width: f64,
        /// Flange thickness.
        flange_thickness: f64,
        /// Web thickness.
        web_thickness: f64,
    },
}

impl Section {
    /// Returns the short shape identifier (`rectangle`, `circle` or `i`).
    pub fn shape_name(self) -> &'static str {
        match self {
            Self::Rectangle { .. } => "rectangle",
            Self::Circle { .. } => "circle",
            Self::ISection { .. } => "i",
        }
    }
}

/// Global isotropic linear elastic material and section data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// Human readable material name.
    #[serde(default = "default_material_name")]
    pub name: String,
    /// Young's modulus `E` (MPa for millimetres, Pa for metres).
    pub youngs_modulus: f64,
    /// Cross-sectional area `A` (mm² for millimetres, m² for metres).
    pub area: f64,
    /// Mass density used by modal analysis and self-weight.
    ///
    /// The value must be consistent with the declared units, for example
    /// `7.85e-9 t/mm³` in the millimetre system. It defaults to `0`, which is
    /// valid for static analysis but rejected when modal analysis or gravity is
    /// requested.
    #[serde(default)]
    pub density: f64,
    /// Permissible stress used by stress-recovery utilisation checks.
    ///
    /// The value uses the document stress unit (MPa for millimetres, Pa for
    /// metres). It is optional and never required by static, modal or buckling
    /// analysis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowable_stress: Option<f64>,
    /// Coefficient of thermal expansion `alpha` in `1/K`.
    ///
    /// It is required only when an element declares a uniform temperature
    /// change, and is never used by analyses without thermal loads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    /// Parametric section geometry.
    ///
    /// Supplying a section lets beams omit their own `inertia` and enables
    /// beam stress recovery through the extreme-fibre distance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<Section>,
}

fn default_material_name() -> String {
    "steel".to_string()
}

/// Element formulation.
///
/// [`ElementKind::Truss`] carries axial force only. [`ElementKind::Beam`] is the
/// two-dimensional Euler-Bernoulli beam with three degrees of freedom per
/// node (`ux`, `uy`, `rz`); three-dimensional beams are not implemented and are
/// rejected with `capability/not-implemented`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ElementKind {
    /// Two-force truss member (axial stiffness only).
    #[serde(rename = "truss")]
    #[default]
    Truss,
    /// Two-dimensional Euler-Bernoulli beam.
    #[serde(rename = "beam")]
    Beam,
}

impl ElementKind {
    /// Returns the short identifier stored in documents (`"truss"` or `"beam"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Truss => "truss",
            Self::Beam => "beam",
        }
    }
}

impl FromStr for ElementKind {
    type Err = CaeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "truss" => Ok(Self::Truss),
            "beam" => Ok(Self::Beam),
            other => Err(CaeError::new(
                "cli/invalid-option",
                format!("unknown element type `{other}`; valid values are `truss` or `beam`"),
            )),
        }
    }
}

fn default_dimension() -> u32 {
    2
}

/// A truss node with a unique identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Unique node identifier.
    pub id: String,
    /// X coordinate (mm or m according to [`Model::units`]).
    pub x: f64,
    /// Y coordinate (mm or m according to [`Model::units`]).
    pub y: f64,
    /// Z coordinate (mm or m according to [`Model::units`]). Defaults to 0
    /// so two-dimensional documents omit it.
    #[serde(default)]
    pub z: f64,
}

/// Optional per-element material override.
///
/// Missing fields fall back to the global [`Model::material`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementMaterial {
    /// Element-specific Young's modulus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub youngs_modulus: Option<f64>,
    /// Element-specific cross-sectional area.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<f64>,
}

/// Bending-moment release (hinge) at the ends of a two-dimensional beam.
///
/// A released end carries no bending moment; the matching rotational degree of
/// freedom is condensed out of the element before assembly, so the element
/// behaves like a member with a hinge at that end. Releasing both ends of a
/// member therefore models a simply supported (moment-free) beam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndRelease {
    /// Release the bending moment at the `from` node.
    #[serde(default)]
    pub start: bool,
    /// Release the bending moment at the `to` node.
    #[serde(default)]
    pub end: bool,
}

impl EndRelease {
    /// Returns `true` when no end is released.
    pub fn is_empty(self) -> bool {
        !self.start && !self.end
    }
}

/// A structural member: a truss bar or a two-dimensional Euler-Bernoulli beam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    /// Unique element identifier.
    pub id: String,
    /// Start node identifier.
    #[serde(rename = "from")]
    pub from: String,
    /// End node identifier.
    #[serde(rename = "to")]
    pub to: String,
    /// Optional element-specific `E`/`A` overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<ElementMaterial>,
    /// Element formulation; defaults to [`ElementKind::Truss`].
    #[serde(default)]
    pub kind: ElementKind,
    /// Second moment of area `I` about the out-of-plane axis; required and
    /// strictly positive for beam elements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inertia: Option<f64>,
    /// Distributed load per unit length at the `from` end in the element's
    /// local `+y` direction; only valid on beam elements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_load: Option<f64>,
    /// Distributed load per unit length at the `to` end in the element's local
    /// `+y` direction.
    ///
    /// Together with [`Element::distributed_load`] this defines a linearly
    /// varying (trapezoidal) transverse load. When the start value is omitted
    /// it is `0`, and when this end value is omitted the load is uniform.
    /// Only valid on beam elements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_load_end: Option<f64>,
    /// Start of the loaded segment as a fraction of the element length.
    ///
    /// When both this and [`Element::distributed_load_to`] are present the
    /// distributed load acts only between these fractions, and
    /// [`Element::distributed_load`] / [`Element::distributed_load_end`] are the
    /// intensities at the segment ends instead of at the member ends. Both
    /// fractions must satisfy `0 <= from < to <= 1`. Only valid on beam
    /// elements that declare a distributed load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_load_from: Option<f64>,
    /// End of the loaded segment as a fraction of the element length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_load_to: Option<f64>,
    /// Uniform temperature change `ΔT` of the member (kelvin or degrees
    /// Celsius difference).
    ///
    /// The solver converts it into a thermal axial strain `alpha·ΔT` and an
    /// equivalent nodal load. A uniform change never produces bending. The
    /// material must declare [`Material::alpha`] when this value is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Linear temperature difference between the local `+y` and `-y` extreme
    /// fibres of the section.
    ///
    /// A positive value heats the `+y` face. The solver converts it into the
    /// free thermal curvature `alpha·ΔT/h`, where `h` is the section depth
    /// supplied by [`Material::section`], and adds the matching equivalent end
    /// moments. A uniform [`Element::temperature`] and a gradient may be
    /// combined. Only valid on beam elements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_gradient: Option<f64>,
    /// Optional bending-moment releases at the beam ends.
    ///
    /// Only valid on beam elements; a member with both ends released carries no
    /// end moments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<EndRelease>,
}

/// Enforced (prescribed) values of the constrained degrees of freedom of a
/// support.
///
/// Every component applies only when the matching [`Support`] flag (`dx`, `dy`,
/// `dz`, `rz`) is `true`; components left out of the document default to `0`,
/// which is the ordinary homogeneous constraint. Values use the document units
/// and the rotation is in radians. A non-zero value therefore models a support
/// settlement or an explicitly driven displacement.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrescribedDisplacement {
    /// Enforced translation along X (requires `dx`).
    #[serde(default)]
    pub ux: f64,
    /// Enforced translation along Y (requires `dy`).
    #[serde(default)]
    pub uy: f64,
    /// Enforced translation along Z (requires `dz`; three dimensions only).
    #[serde(default)]
    pub uz: f64,
    /// Enforced rotation about Z in radians (requires `rz`; beam nodes in two
    /// dimensions only).
    #[serde(default)]
    pub rz: f64,
}

/// Linear elastic spring stiffness attached to a support node.
///
/// A spring is a flexible support: the matching degree of freedom stays free
/// and the assembled stiffness receives an additional diagonal term. All
/// components default to zero, and a component is only active when it is
/// strictly positive. Units follow the document: force per length for `kx`,
/// `ky` and `kz`, and moment per radian for `kr`.
///
/// A spring and a rigid constraint on the same degree of freedom are mutually
/// exclusive and rejected by validation (`support/spring-constrained`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpringStiffness {
    /// Translational stiffness along X.
    #[serde(default)]
    pub kx: f64,
    /// Translational stiffness along Y.
    #[serde(default)]
    pub ky: f64,
    /// Translational stiffness along Z; three dimensions only.
    #[serde(default)]
    pub kz: f64,
    /// Rotational stiffness about Z; two-dimensional beam nodes only.
    #[serde(default)]
    pub kr: f64,
}

impl SpringStiffness {
    /// Returns `true` when every component is zero.
    pub fn is_empty(self) -> bool {
        self.kx == 0.0 && self.ky == 0.0 && self.kz == 0.0 && self.kr == 0.0
    }
}

/// Support conditions of a node. `true` means that degree of freedom is fixed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Support {
    /// Node identifier.
    pub node: String,
    /// `true` constrains translation along X.
    #[serde(default)]
    pub dx: bool,
    /// `true` constrains translation along Y.
    #[serde(default)]
    pub dy: bool,
    /// `true` constrains translation along Z. Only valid in three dimensions.
    #[serde(default)]
    pub dz: bool,
    /// `true` constrains rotation about Z. Only valid in two dimensions and
    /// only at nodes touched by at least one beam element.
    #[serde(default)]
    pub rz: bool,
    /// Enforced non-zero values of the constrained degrees of freedom.
    ///
    /// `None` is the ordinary zero constraint and is omitted from the JSON, so
    /// documents written by older builds load unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prescribed: Option<PrescribedDisplacement>,
    /// Linear elastic spring stiffness of this node.
    ///
    /// Supplying a spring leaves the matching degrees of freedom free and adds
    /// its stiffness to the global matrix; the support reaction then reports
    /// the spring force `−k·u`. `None` is omitted from the JSON so existing
    /// documents load unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spring: Option<SpringStiffness>,
}

/// Concentrated nodal load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Load {
    /// Node identifier.
    pub node: String,
    /// Force along X (positive to the right). Defaults to zero.
    #[serde(default)]
    pub fx: f64,
    /// Force along Y (positive upwards). Defaults to zero.
    #[serde(default)]
    pub fy: f64,
    /// Force along Z. Defaults to zero and is only valid in three dimensions.
    #[serde(default)]
    pub fz: f64,
    /// Concentrated moment about Z (counter-clockwise positive). Defaults to
    /// zero and is only valid at beam nodes in two dimensions.
    #[serde(default)]
    pub mz: f64,
}

/// Linear combination of the applied load case and self-weight.
///
/// `loads` multiplies every nodal force/moment and every element distributed
/// load; `gravity` multiplies the gravity vector. Prescribed displacements are
/// boundary conditions and are never scaled. A combination named `D+L` with
/// `loads = 1` and `gravity = 1` therefore means `1.0·(live loads) +
/// 1.0·(dead weight)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadCombination {
    /// Unique human readable combination name.
    pub name: String,
    /// Factor applied to nodal loads and element distributed loads.
    #[serde(default = "default_load_factor")]
    pub loads: f64,
    /// Factor applied to the gravity vector (self-weight).
    #[serde(default)]
    pub gravity: f64,
}

fn default_load_factor() -> f64 {
    1.0
}

/// In-memory representation of an `.ocae` document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Document format tag; must equal [`FORMAT`].
    pub format: String,
    /// Document version; this build understands [`VERSION`].
    pub version: u32,
    /// Declared unit system. Missing units are assumed to be millimetres and
    /// reported as a validation notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<Units>,
    /// Spatial dimension of the model: `2` or `3`. Defaults to `2` so older
    /// documents without this field remain valid.
    #[serde(default = "default_dimension")]
    pub dimension: u32,
    /// Global material and section data.
    pub material: Material,
    /// Model nodes.
    #[serde(default)]
    pub nodes: Vec<Node>,
    /// Truss elements.
    #[serde(default)]
    pub elements: Vec<Element>,
    /// Nodal supports.
    #[serde(default)]
    pub supports: Vec<Support>,
    /// Concentrated nodal loads.
    #[serde(default)]
    pub loads: Vec<Load>,
    /// Optional gravity acceleration vector `g = (gx, gy, gz)`.
    ///
    /// When set, the solver applies body loads derived from the material
    /// density and section properties. Unit consistency is the caller's
    /// responsibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity: Option<[f64; 3]>,
    /// Optional linear load combinations used by the combination solver.
    ///
    /// An empty list keeps existing documents unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub combinations: Vec<LoadCombination>,
    /// Monotonically increasing edit counter maintained by the CLI helpers.
    #[serde(default)]
    pub revision: u32,
}

impl Model {
    /// Creates an empty document in the requested unit system with a default
    /// steel material and revision `0`.
    ///
    /// The default section is 100 mm². In millimetres this is `E = 210000 MPa`,
    /// `A = 100 mm²`; in metres the same physical section is `E = 2.1e11 Pa`,
    /// `A = 1e-4 m²`.
    pub fn new(units: Units) -> Self {
        Self::new_with_dimension(units, 2)
    }

    /// Creates an empty document in the requested unit system and dimension.
    ///
    /// `dimension` is stored as given; only `2` and `3` are accepted by
    /// validation and by the CLI. The default material and revision follow
    /// [`Model::new`].
    pub fn new_with_dimension(units: Units, dimension: u32) -> Self {
        let (youngs_modulus, area) = match units {
            Units::Millimeter => (210_000.0, 100.0),
            Units::Meter => (210_000_000_000.0, 1e-4),
        };
        Self {
            format: FORMAT.to_string(),
            version: VERSION,
            units: Some(units),
            dimension,
            material: Material {
                name: default_material_name(),
                youngs_modulus,
                area,
                density: 0.0,
                allowable_stress: None,
                alpha: None,
                section: None,
            },
            nodes: Vec::new(),
            elements: Vec::new(),
            supports: Vec::new(),
            loads: Vec::new(),
            gravity: None,
            combinations: Vec::new(),
            revision: 0,
        }
    }

    /// Parses a document from JSON text.
    pub fn parse_json(text: &str) -> Result<Self, CaeError> {
        serde_json::from_str(text).map_err(|error| CaeError::new("document/parse", format!("cannot parse .ocae JSON: {error}")))
    }

    /// Serialises the document as pretty-printed JSON.
    pub fn to_json_pretty(&self) -> Result<String, CaeError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| CaeError::new("document/serialize", format!("cannot serialize document: {error}")))
    }

    /// Reads and parses an `.ocae` file.
    pub fn load(path: &Path) -> Result<Self, CaeError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| CaeError::new("io/read", format!("cannot read `{}`: {error}", path.display())))?;
        Self::parse_json(&text)
    }

    /// Serialises and atomically writes an `.ocae` file.
    pub fn save_atomic(&self, path: &Path) -> Result<(), CaeError> {
        let text = self.to_json_pretty()?;
        export::write_atomic(path, &text)
    }

    /// Returns a node by identifier.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Appends a node at `z = 0`, rejecting duplicate identifiers.
    pub fn add_node(&mut self, id: impl Into<String>, x: f64, y: f64) -> Result<(), CaeError> {
        self.add_node_3d(id, x, y, 0.0)
    }

    /// Appends a node with an explicit Z coordinate.
    ///
    /// A non-zero `z` is rejected while the document is two-dimensional;
    /// switch [`Model::dimension`] to `3` first.
    pub fn add_node_3d(&mut self, id: impl Into<String>, x: f64, y: f64, z: f64) -> Result<(), CaeError> {
        let id = id.into();
        if self.node(&id).is_some() {
            return Err(CaeError::new("node/duplicate-id", format!("node `{id}` already exists")));
        }
        if self.dimension != 3 && z != 0.0 {
            return Err(dimension_mismatch(format!(
                "a two-dimensional document cannot add node `{id}` with z={z}; set dimension to 3 first"
            )));
        }
        self.nodes.push(Node { id, x, y, z });
        self.bump_revision();
        Ok(())
    }

    /// Appends a truss element after checking identifiers, endpoints and
    /// geometric degeneracy.
    pub fn add_element(
        &mut self,
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        material: Option<ElementMaterial>,
    ) -> Result<(), CaeError> {
        self.add_element_kind(id, from, to, material, ElementKind::Truss, None, None)
    }

    /// Appends a structural element of the requested kind.
    ///
    /// Beam elements require a two-dimensional document and a strictly
    /// positive `inertia`; a distributed load is only valid on beams. Invalid
    /// combinations are rejected before the file is written.
    #[allow(clippy::too_many_arguments)]
    pub fn add_element_kind(
        &mut self,
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        material: Option<ElementMaterial>,
        kind: ElementKind,
        inertia: Option<f64>,
        distributed_load: Option<f64>,
    ) -> Result<(), CaeError> {
        self.add_element_with_loads(id, from, to, material, kind, inertia, distributed_load, None)
    }

    /// Appends a structural element with a linearly varying distributed load.
    ///
    /// `distributed_load` is the load per unit length at the `from` end and
    /// `distributed_load_end` the value at the `to` end, both in the local
    /// `+y` direction. Supplying only the end value defines a triangular load
    /// that starts at zero; two equal values are a uniform load. Beam
    /// requirements and rejection rules match [`Model::add_element_kind`].
    #[allow(clippy::too_many_arguments)]
    pub fn add_element_with_loads(
        &mut self,
        id: impl Into<String>,
        from: impl Into<String>,
        to: impl Into<String>,
        material: Option<ElementMaterial>,
        kind: ElementKind,
        inertia: Option<f64>,
        distributed_load: Option<f64>,
        distributed_load_end: Option<f64>,
    ) -> Result<(), CaeError> {
        let id = id.into();
        let from = from.into();
        let to = to.into();
        if self.elements.iter().any(|element| element.id == id) {
            return Err(CaeError::new("element/duplicate-id", format!("element `{id}` already exists")));
        }
        if from == to {
            return Err(CaeError::new(
                "element/degenerate",
                format!("element `{id}` must not have identical start and end nodes"),
            ));
        }
        let start = self
            .node(&from)
            .ok_or_else(|| CaeError::new("element/unknown-node", format!("element `{id}` references unknown node `{from}`")))?;
        let end = self
            .node(&to)
            .ok_or_else(|| CaeError::new("element/unknown-node", format!("element `{id}` references unknown node `{to}`")))?;
        if start.x == end.x && start.y == end.y && start.z == end.z {
            return Err(CaeError::new("element/zero-length", format!("element `{id}` has zero length")));
        }
        match kind {
            ElementKind::Beam => {
                if self.dimension != 2 {
                    return Err(CaeError::new(
                        "capability/not-implemented",
                        format!(
                            "3D beam elements are not implemented; element `{id}` can only use beam in a dimension=2 document"
                        ),
                    ));
                }
                let usable_inertia = match inertia {
                    Some(value) => positive_finite(value),
                    None => self.section_inertia().is_some_and(positive_finite),
                };
                if !usable_inertia {
                    return Err(CaeError::new(
                        "element/invalid-inertia",
                        match inertia {
                            Some(value) => {
                                format!("beam element `{id}` inertia must be positive and finite (currently {value})")
                            }
                            None => format!(
                                "beam element `{id}` requires an explicit positive inertia or a material section that provides one"
                            ),
                        },
                    ));
                }
            }
            ElementKind::Truss => {
                if distributed_load.is_some() || distributed_load_end.is_some() {
                    return Err(CaeError::new(
                        "element/distributed-load-requires-beam",
                        format!(
                            "element `{id}` is a truss element and cannot carry distributed load w; use a beam element instead"
                        ),
                    ));
                }
            }
        }
        if distributed_load.is_some_and(|value| !value.is_finite())
            || distributed_load_end.is_some_and(|value| !value.is_finite())
        {
            return Err(CaeError::new(
                "element/invalid-distributed-load",
                format!("distributed load w of element `{id}` must be finite"),
            ));
        }
        self.elements.push(Element {
            id,
            from,
            to,
            material,
            kind,
            inertia,
            distributed_load,
            distributed_load_end,
            distributed_load_from: None,
            distributed_load_to: None,
            temperature: None,
            temperature_gradient: None,
            release: None,
        });
        self.bump_revision();
        Ok(())
    }

    /// Replaces the global material data, keeping the current density.
    pub fn set_material(&mut self, name: Option<String>, youngs_modulus: f64, area: f64) {
        if let Some(name) = name {
            self.material.name = name;
        }
        self.material.youngs_modulus = youngs_modulus;
        self.material.area = area;
        self.bump_revision();
    }

    /// Replaces the global material data including the mass density.
    pub fn set_material_full(&mut self, name: Option<String>, youngs_modulus: f64, area: f64, density: f64) {
        if let Some(name) = name {
            self.material.name = name;
        }
        self.material.youngs_modulus = youngs_modulus;
        self.material.area = area;
        self.material.density = density;
        self.bump_revision();
    }

    /// Replaces the global material, section and allowable-stress data.
    ///
    /// Section geometry and allowable stress are validated before the document
    /// is touched, so an invalid section never reaches the file.
    pub fn set_material_data(&mut self, material: Material) -> Result<(), CaeError> {
        if !positive_finite(material.youngs_modulus) || !positive_finite(material.area) {
            return Err(CaeError::new(
                "material/invalid",
                format!(
                    "E and A of material `{}` must be positive (currently E={}, A={})",
                    material.name, material.youngs_modulus, material.area
                ),
            ));
        }
        if !material.density.is_finite() || material.density < 0.0 {
            return Err(CaeError::new(
                "material/invalid-density",
                format!("material density must be finite and non-negative (currently {})", material.density),
            ));
        }
        if material.allowable_stress.is_some_and(|value| !positive_finite(value)) {
            return Err(CaeError::new(
                "material/invalid-allowable-stress",
                format!("allowable stress must be positive and finite (currently {:?})", material.allowable_stress),
            ));
        }
        if material.alpha.is_some_and(|value| !value.is_finite()) {
            return Err(CaeError::new(
                "material/invalid-alpha",
                format!("coefficient of thermal expansion alpha must be finite (currently {:?})", material.alpha),
            ));
        }
        if let Some(section) = material.section {
            section.properties()?;
        }
        self.material = material;
        self.bump_revision();
        Ok(())
    }

    /// Replaces or clears the parametric cross-section of the global material.
    ///
    /// The section geometry is validated before the document is touched.
    pub fn set_section(&mut self, section: Option<Section>) -> Result<(), CaeError> {
        if let Some(section) = section {
            section.properties()?;
        }
        self.material.section = section;
        self.bump_revision();
        Ok(())
    }

    /// Strong-axis second moment of area supplied by the material section.
    fn section_inertia(&self) -> Option<f64> {
        self.material.section.and_then(|section| section.properties().ok()).map(|properties| properties.inertia_strong)
    }

    /// Sets or clears the uniform temperature change of an existing element.
    ///
    /// A non-finite value is rejected with `element/invalid-temperature` and an
    /// unknown element identifier with `element/unknown-id`. `None` removes the
    /// thermal load again, so the element returns to the isothermal state.
    pub fn set_element_temperature(&mut self, id: &str, temperature: Option<f64>) -> Result<(), CaeError> {
        if temperature.is_some_and(|value| !value.is_finite()) {
            return Err(CaeError::new(
                "element/invalid-temperature",
                format!("temperature change of element `{id}` must be finite"),
            ));
        }
        let element = self
            .elements
            .iter_mut()
            .find(|element| element.id == id)
            .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{id}`")))?;
        element.temperature = temperature;
        self.bump_revision();
        Ok(())
    }

    /// Sets or clears the loaded segment of an existing beam's distributed
    /// load.
    ///
    /// `span` holds the start and end fractions of the element length, so
    /// `Some((0.25, 0.75))` applies the load to the middle half of the member.
    /// The element must already declare a distributed load. The fractions must
    /// satisfy `0 <= start < end <= 1` and are rejected with
    /// `element/invalid-load-span` otherwise; a truss is rejected with
    /// `element/distributed-load-requires-beam`. `None` restores a load over
    /// the whole member.
    pub fn set_element_load_span(&mut self, id: &str, span: Option<(f64, f64)>) -> Result<(), CaeError> {
        if let Some((start, end)) = span {
            if !start.is_finite() || !end.is_finite() || start < 0.0 || end > 1.0 || start >= end {
                return Err(CaeError::new(
                    "element/invalid-load-span",
                    format!("load span of element `{id}` must satisfy 0 <= start < end <= 1 (currently {start}..{end})"),
                ));
            }
        }
        let element = self
            .elements
            .iter_mut()
            .find(|element| element.id == id)
            .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{id}`")))?;
        if let Some((_, _)) = span {
            if element.kind != ElementKind::Beam {
                return Err(CaeError::new(
                    "element/distributed-load-requires-beam",
                    format!("element `{id}` is a truss element and cannot carry a distributed load span"),
                ));
            }
            if element.distributed_load.is_none() {
                return Err(CaeError::new(
                    "element/missing-distributed-load",
                    format!("element `{id}` must declare a distributed load before its span can be limited"),
                ));
            }
        }
        match span {
            Some((start, end)) => {
                element.distributed_load_from = Some(start);
                element.distributed_load_to = Some(end);
            }
            None => {
                element.distributed_load_from = None;
                element.distributed_load_to = None;
            }
        }
        self.bump_revision();
        Ok(())
    }

    /// Sets or clears the linear temperature gradient of an existing beam.
    ///
    /// `gradient` is the temperature difference between the local `+y` and `-y`
    /// extreme fibres. A non-finite value is rejected with
    /// `element/invalid-temperature-gradient`, a truss element with
    /// `element/gradient-requires-beam` and an unknown identifier with
    /// `element/unknown-id`. `None` removes the gradient again.
    pub fn set_element_temperature_gradient(&mut self, id: &str, gradient: Option<f64>) -> Result<(), CaeError> {
        if gradient.is_some_and(|value| !value.is_finite()) {
            return Err(CaeError::new(
                "element/invalid-temperature-gradient",
                format!("temperature gradient of element `{id}` must be finite"),
            ));
        }
        let element = self
            .elements
            .iter_mut()
            .find(|element| element.id == id)
            .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{id}`")))?;
        if element.kind != ElementKind::Beam && gradient.is_some() {
            return Err(CaeError::new(
                "element/gradient-requires-beam",
                format!("element `{id}` is a truss element and cannot carry a temperature gradient; use a beam element"),
            ));
        }
        element.temperature_gradient = gradient;
        self.bump_revision();
        Ok(())
    }

    /// Sets or clears the bending-moment releases of an existing beam.
    ///
    /// A release on a truss element is rejected with
    /// `element/release-requires-beam`, an empty release with
    /// `element/invalid-release` and an unknown identifier with
    /// `element/unknown-id`. `None` restores a fully continuous member.
    pub fn set_element_release(&mut self, id: &str, release: Option<EndRelease>) -> Result<(), CaeError> {
        if release.is_some_and(EndRelease::is_empty) {
            return Err(CaeError::new(
                "element/invalid-release",
                format!("release of element `{id}` must free at least one end; use `null` to remove it"),
            ));
        }
        let element = self
            .elements
            .iter_mut()
            .find(|element| element.id == id)
            .ok_or_else(|| CaeError::new("element/unknown-id", format!("unknown element `{id}`")))?;
        if element.kind != ElementKind::Beam && release.is_some() {
            return Err(CaeError::new(
                "element/release-requires-beam",
                format!("element `{id}` is a truss element and cannot have bending-moment releases; use a beam element"),
            ));
        }
        element.release = release;
        self.bump_revision();
        Ok(())
    }

    /// Sets the gravity acceleration vector, replacing any previous value.
    ///
    /// All components must be finite. A non-zero `gz` is rejected while the
    /// document is two-dimensional. Setting the zero vector clears gravity.
    pub fn set_gravity(&mut self, gx: f64, gy: f64, gz: f64) -> Result<(), CaeError> {
        if !gx.is_finite() || !gy.is_finite() || !gz.is_finite() {
            return Err(CaeError::new("document/invalid-gravity", "gravity acceleration g components must be finite"));
        }
        if self.dimension != 3 && gz != 0.0 {
            return Err(dimension_mismatch(format!(
                "a two-dimensional document cannot set a non-zero gz (currently gz={gz}); set dimension to 3 first"
            )));
        }
        self.gravity = if gx == 0.0 && gy == 0.0 && gz == 0.0 { None } else { Some([gx, gy, gz]) };
        self.bump_revision();
        Ok(())
    }

    /// Adds or replaces a linear load combination.
    ///
    /// The name must be non-empty and both factors must be finite. Supplying an
    /// existing name updates its factors in place.
    pub fn add_combination(&mut self, name: impl Into<String>, loads: f64, gravity: f64) -> Result<(), CaeError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(CaeError::new("combination/invalid-name", "a load combination requires a non-empty name"));
        }
        if !loads.is_finite() || !gravity.is_finite() {
            return Err(CaeError::new(
                "combination/invalid-factor",
                format!("load combination `{name}` factors must be finite (loads={loads}, gravity={gravity})"),
            ));
        }
        if let Some(existing) = self.combinations.iter_mut().find(|combination| combination.name == name) {
            existing.loads = loads;
            existing.gravity = gravity;
        }
        else {
            self.combinations.push(LoadCombination { name, loads, gravity });
        }
        self.bump_revision();
        Ok(())
    }

    /// Removes a load combination by name, returning whether it existed.
    pub fn remove_combination(&mut self, name: &str) -> bool {
        let before = self.combinations.len();
        self.combinations.retain(|combination| combination.name != name);
        let removed = self.combinations.len() != before;
        if removed {
            self.bump_revision();
        }
        removed
    }

    /// Adds or merges a support entry for a node without a Z constraint.
    pub fn add_support(&mut self, node: impl Into<String>, dx: bool, dy: bool) -> Result<(), CaeError> {
        self.add_support_3d(node, dx, dy, false)
    }

    /// Adds or merges a support entry for a node, optionally constraining Z.
    ///
    /// `dz` is rejected while the document is two-dimensional.
    pub fn add_support_3d(&mut self, node: impl Into<String>, dx: bool, dy: bool, dz: bool) -> Result<(), CaeError> {
        self.add_support_full(node, dx, dy, dz, false)
    }

    /// Adds or merges a support entry, optionally constraining rotation about Z.
    ///
    /// `rz` only takes effect at nodes touched by a beam element; validation
    /// reports `support/inactive-dof` otherwise.
    pub fn add_support_full(
        &mut self,
        node: impl Into<String>,
        dx: bool,
        dy: bool,
        dz: bool,
        rz: bool,
    ) -> Result<(), CaeError> {
        let node = node.into();
        if self.node(&node).is_none() {
            return Err(CaeError::new("support/unknown-node", format!("support references unknown node `{node}`")));
        }
        if self.dimension != 3 && dz {
            return Err(dimension_mismatch(format!(
                "a two-dimensional document cannot constrain Z translation of node `{node}`; set dimension to 3 first"
            )));
        }
        if self.dimension == 3 && rz {
            return Err(dimension_mismatch(format!(
                "a three-dimensional document cannot constrain rz rotation of node `{node}`; beam elements are two-dimensional only"
            )));
        }
        if let Some(support) = self.supports.iter_mut().find(|support| support.node == node) {
            support.dx |= dx;
            support.dy |= dy;
            support.dz |= dz;
            support.rz |= rz;
        }
        else {
            self.supports.push(Support { node, dx, dy, dz, rz, prescribed: None, spring: None });
        }
        self.bump_revision();
        Ok(())
    }

    /// Sets or merges linear elastic spring stiffness at a support node.
    ///
    /// Supplying a component writes that component; components that are not
    /// supplied keep their current value. At least one component must be
    /// supplied and the resulting stiffness must not be zero. A spring is a
    /// flexible support, so the matching degree of freedom stays free.
    ///
    /// An unknown node is rejected with `support/unknown-node`; a negative or
    /// non-finite stiffness with `support/invalid-spring`; a `kz` spring in a
    /// two-dimensional document or a `kr` spring in a three-dimensional
    /// document with `document/dimension-mismatch`.
    pub fn set_spring(
        &mut self,
        node: impl Into<String>,
        kx: Option<f64>,
        ky: Option<f64>,
        kz: Option<f64>,
        kr: Option<f64>,
    ) -> Result<(), CaeError> {
        let node = node.into();
        if kx.is_none() && ky.is_none() && kz.is_none() && kr.is_none() {
            return Err(CaeError::new(
                "support/no-freedom",
                "specify at least one of kx, ky, kz or kr; otherwise the spring carries no stiffness",
            ));
        }
        if self.node(&node).is_none() {
            return Err(CaeError::new("support/unknown-node", format!("support references unknown node `{node}`")));
        }
        if [kx, ky, kz, kr].into_iter().flatten().any(|value| !value.is_finite() || value < 0.0) {
            return Err(CaeError::new(
                "support/invalid-spring",
                format!("spring stiffness of node `{node}` must be finite and non-negative"),
            ));
        }
        let existing =
            self.supports.iter().find(|support| support.node == node).and_then(|support| support.spring).unwrap_or_default();
        let merged = SpringStiffness {
            kx: kx.unwrap_or(existing.kx),
            ky: ky.unwrap_or(existing.ky),
            kz: kz.unwrap_or(existing.kz),
            kr: kr.unwrap_or(existing.kr),
        };
        if merged.is_empty() {
            return Err(CaeError::new(
                "support/invalid-spring",
                format!("spring stiffness of node `{node}` must be strictly positive"),
            ));
        }
        if self.dimension != 3 && merged.kz != 0.0 {
            return Err(dimension_mismatch(format!(
                "a two-dimensional document cannot apply a kz spring to node `{node}`; set dimension to 3 first"
            )));
        }
        if self.dimension == 3 && merged.kr != 0.0 {
            return Err(dimension_mismatch(format!(
                "a three-dimensional document cannot apply a kr spring to node `{node}`; beam elements are two-dimensional only"
            )));
        }
        let position = match self.supports.iter().position(|support| support.node == node) {
            Some(position) => position,
            None => {
                self.supports.push(Support {
                    node: node.clone(),
                    dx: false,
                    dy: false,
                    dz: false,
                    rz: false,
                    prescribed: None,
                    spring: None,
                });
                self.supports.len() - 1
            }
        };
        self.supports[position].spring = Some(merged);
        self.bump_revision();
        Ok(())
    }

    /// Enforces (prescribes) non-zero displacement values at a support node.
    ///
    /// Supplying a component also constrains the matching degree of freedom, so
    /// `prescribe_displacement("n2", Some(0.5), ..)` implies a `dx` support on
    /// `n2` with `ux = 0.5`. Components not supplied keep their previous value.
    /// An unknown node is rejected with `support/unknown-node`; a non-zero `uz`
    /// in a two-dimensional document or a non-zero rotation in a
    /// three-dimensional document is rejected with
    /// `document/dimension-mismatch`. Callers receive `support/no-freedom` when
    /// no component is supplied.
    pub fn prescribe_displacement(
        &mut self,
        node: impl Into<String>,
        ux: Option<f64>,
        uy: Option<f64>,
        uz: Option<f64>,
        rz: Option<f64>,
    ) -> Result<(), CaeError> {
        let node = node.into();
        if ux.is_none() && uy.is_none() && uz.is_none() && rz.is_none() {
            return Err(CaeError::new(
                "support/no-freedom",
                "specify at least one of ux, uy, uz or rz; otherwise no displacement is prescribed",
            ));
        }
        if self.node(&node).is_none() {
            return Err(CaeError::new("support/unknown-node", format!("support references unknown node `{node}`")));
        }
        if [ux, uy, uz, rz].into_iter().flatten().any(|value| !value.is_finite()) {
            return Err(CaeError::new("support/invalid-prescribed", "prescribed displacements must be finite"));
        }
        if self.dimension != 3 && uz.is_some_and(|value| value != 0.0) {
            return Err(dimension_mismatch(format!(
                "a two-dimensional document cannot prescribe a Z displacement for node `{node}`; set dimension to 3 first"
            )));
        }
        if self.dimension == 3 && rz.is_some_and(|value| value != 0.0) {
            return Err(dimension_mismatch(format!(
                "a three-dimensional document cannot prescribe an rz rotation for node `{node}`; beam elements are two-dimensional only"
            )));
        }
        let position = match self.supports.iter().position(|support| support.node == node) {
            Some(position) => position,
            None => {
                self.supports.push(Support {
                    node: node.clone(),
                    dx: false,
                    dy: false,
                    dz: false,
                    rz: false,
                    prescribed: None,
                    spring: None,
                });
                self.supports.len() - 1
            }
        };
        let support = &mut self.supports[position];
        let prescribed = support.prescribed.get_or_insert_with(PrescribedDisplacement::default);
        if let Some(value) = ux {
            support.dx = true;
            prescribed.ux = value;
        }
        if let Some(value) = uy {
            support.dy = true;
            prescribed.uy = value;
        }
        if let Some(value) = uz {
            support.dz = true;
            prescribed.uz = value;
        }
        if let Some(value) = rz {
            support.rz = true;
            prescribed.rz = value;
        }
        self.bump_revision();
        Ok(())
    }

    /// Adds or updates a concentrated load on a node.
    ///
    /// Only the components supplied as `Some` are written; a new load entry is
    /// created with zero default components when the node had none.
    pub fn add_load(&mut self, node: impl Into<String>, fx: Option<f64>, fy: Option<f64>) -> Result<(), CaeError> {
        self.add_load_3d(node, fx, fy, None)
    }

    /// Adds or updates a concentrated load including an optional Z component.
    ///
    /// A non-zero `fz` is rejected while the document is two-dimensional.
    pub fn add_load_3d(
        &mut self,
        node: impl Into<String>,
        fx: Option<f64>,
        fy: Option<f64>,
        fz: Option<f64>,
    ) -> Result<(), CaeError> {
        self.add_load_full(node, fx, fy, fz, None)
    }

    /// Adds or updates a concentrated load including an optional moment `mz`.
    ///
    /// A non-zero `mz` only takes effect at beam nodes in two dimensions;
    /// validation reports `load/inactive-dof` otherwise.
    pub fn add_load_full(
        &mut self,
        node: impl Into<String>,
        fx: Option<f64>,
        fy: Option<f64>,
        fz: Option<f64>,
        mz: Option<f64>,
    ) -> Result<(), CaeError> {
        let node = node.into();
        if self.node(&node).is_none() {
            return Err(CaeError::new("load/unknown-node", format!("load references unknown node `{node}`")));
        }
        if self.dimension != 3 && fz.is_some_and(|value| value != 0.0) {
            return Err(dimension_mismatch(format!(
                "a two-dimensional document cannot apply a Z load (node `{node}`); set dimension to 3 first"
            )));
        }
        if self.dimension == 3 && mz.is_some_and(|value| value != 0.0) {
            return Err(dimension_mismatch(format!(
                "a three-dimensional document cannot apply an mz moment (node `{node}`); beam elements are two-dimensional only"
            )));
        }
        if let Some(load) = self.loads.iter_mut().find(|load| load.node == node) {
            if let Some(fx) = fx {
                load.fx = fx;
            }
            if let Some(fy) = fy {
                load.fy = fy;
            }
            if let Some(fz) = fz {
                load.fz = fz;
            }
            if let Some(mz) = mz {
                load.mz = mz;
            }
        }
        else {
            self.loads.push(Load {
                node,
                fx: fx.unwrap_or(0.0),
                fy: fy.unwrap_or(0.0),
                fz: fz.unwrap_or(0.0),
                mz: mz.unwrap_or(0.0),
            });
        }
        self.bump_revision();
        Ok(())
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new(Units::Millimeter)
    }
}

fn dimension_mismatch(message: impl Into<String>) -> CaeError {
    CaeError::new("document/dimension-mismatch", message)
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// Identifiers of nodes whose rotational degree of freedom is connected to an
/// element.
///
/// A beam end that is released (hinged) does not transfer moment to its node,
/// so that node only owns an `rz` degree of freedom when at least one beam end
/// at the node is continuous. Rotational degrees of freedom are activated only
/// for these nodes, so a pure truss model keeps two degrees of freedom per node
/// and a released beam end cannot create a zero-stiffness rotation row.
pub(crate) fn rotation_nodes(model: &Model) -> std::collections::HashSet<&str> {
    model
        .elements
        .iter()
        .filter(|element| element.kind == ElementKind::Beam)
        .flat_map(|element| {
            let release = element.release.unwrap_or_default();
            let mut nodes = Vec::with_capacity(2);
            if !release.start {
                nodes.push(element.from.as_str());
            }
            if !release.end {
                nodes.push(element.to.as_str());
            }
            nodes
        })
        .collect()
}
