use std::collections::HashSet;

use boost_check::{CheckReport, Issue as CheckIssue, Severity as CheckSeverity};
use serde::Serialize;

use crate::{
    model::{ElementKind, FORMAT, Model, Units, VERSION, rotation_nodes, spatial_beam_nodes},
    solver::check_stability,
};

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The document must not be solved.
    Error,
    /// Suspicious but solvable input.
    Warning,
    /// Informational message, for example an assumed default.
    Notice,
}

/// A single validation finding with a stable code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Stable code such as `element/zero-length` or `model/mechanism`.
    pub code: String,
    /// How strongly the finding affects solving.
    pub severity: Severity,
    /// Human readable explanation.
    pub message: String,
}

impl From<CheckIssue> for Issue {
    fn from(issue: CheckIssue) -> Self {
        Self { code: issue.code, severity: from_check_severity(issue.severity), message: issue.message }
    }
}

/// Maps the shared `boost-check` vocabulary onto the OpenCAE public vocabulary.
///
/// `boost-check` has no `Notice` level, so `Info` carries the assumed-default
/// findings the CLI and JSON contract call `notice`.
fn from_check_severity(severity: CheckSeverity) -> Severity {
    match severity {
        CheckSeverity::Info => Severity::Notice,
        CheckSeverity::Warning => Severity::Warning,
        CheckSeverity::Error => Severity::Error,
    }
}

fn check_error(code: &str, message: impl Into<String>) -> CheckIssue {
    CheckIssue::error(code, message)
}

fn check_notice(code: &str, message: impl Into<String>) -> CheckIssue {
    CheckIssue::new(code, CheckSeverity::Info, message)
}

fn check_warning(code: &str, message: impl Into<String>) -> CheckIssue {
    CheckIssue::new(code, CheckSeverity::Warning, message)
}

impl Model {
    /// Validates the document and returns every finding it can determine.
    ///
    /// Validation covers document metadata, identifiers, element geometry,
    /// material data, references from supports and loads, and the suppression
    /// of rigid-body modes. The latter is checked by assembling the stiffness
    /// matrix on the free degrees of freedom and looking for a numerical rank
    /// deficiency (a near-zero pivot). The error code for an unstable or
    /// under-constrained structure is always `model/mechanism`.
    ///
    /// Unreferenced nodes are reported as the `node/unconnected` warning and
    /// two nodes at identical coordinates as `node/duplicate-coordinates`.
    /// Warnings never block solving, so a load path that ignores an isolated
    /// node still solves while the document stays visibly suspicious.
    ///
    /// A density or gravity value that belongs to the other unit system is
    /// reported as `units/density-scale` or `units/gravity-scale`, because the
    /// solver never converts units. A load pattern no combination references is
    /// `pattern/unused` and a combination whose factors scale no active action
    /// is `combination/empty`; both are notices, not failures.
    ///
    /// A two-dimensional document (`dimension == 2`) must not contain a
    /// non-zero node `z`, a `dz` support or a non-zero `fz` load; such values
    /// are reported as `document/dimension-mismatch` errors instead of being
    /// silently ignored.
    pub fn validate(&self) -> Vec<Issue> {
        let mut report = CheckReport::new();

        if self.format != FORMAT {
            report
                .push(check_error("document/format", format!("document format must be `{FORMAT}`, but is `{}`", self.format)));
        }
        if self.version != VERSION {
            report.push(check_error(
                "document/version",
                format!("only document version {VERSION} is supported, but found {}", self.version),
            ));
        }
        if self.dimension != 2 && self.dimension != 3 {
            report.push(check_error(
                "document/dimension",
                format!("document dimension must be 2 or 3, but is {}; other dimensions are not supported", self.dimension),
            ));
        }
        if self.units.is_none() {
            report.push(check_notice(
                "document/assumed-units",
                "the document does not declare units; `mm` (N, mm, mm², MPa) is assumed",
            ));
        }
        if self.nodes.is_empty() {
            report.push(check_error("model/no-nodes", "the model requires at least 1 node"));
        }
        if self.elements.is_empty() {
            report.push(check_error("model/no-elements", "the model requires at least 1 element"));
        }
        if !positive_finite(self.material.youngs_modulus) || !positive_finite(self.material.area) {
            report.push(check_error(
                "material/invalid",
                format!(
                    "E and A of global material `{}` must be positive (currently E={}, A={})",
                    self.material.name, self.material.youngs_modulus, self.material.area
                ),
            ));
        }
        if self.material.allowable_stress.is_some_and(|value| !positive_finite(value)) {
            report.push(check_error(
                "material/invalid-allowable-stress",
                format!(
                    "allowable stress of global material `{}` must be positive (currently {:?})",
                    self.material.name, self.material.allowable_stress
                ),
            ));
        }
        if self.material.alpha.is_some_and(|value| !value.is_finite()) {
            report.push(check_error(
                "material/invalid-alpha",
                format!(
                    "coefficient of thermal expansion alpha of material `{}` must be finite (currently {:?})",
                    self.material.name, self.material.alpha
                ),
            ));
        }
        if self.material.conductivity.is_some_and(|value| !positive_finite(value)) {
            report.push(check_error(
                "material/invalid-conductivity",
                format!(
                    "thermal conductivity of material `{}` must be positive and finite (currently {:?})",
                    self.material.name, self.material.conductivity
                ),
            ));
        }
        if self.material.shear_modulus.is_some_and(|value| !positive_finite(value)) {
            report.push(check_error(
                "material/invalid-shear",
                format!(
                    "shear modulus of material `{}` must be positive and finite (currently {:?})",
                    self.material.name, self.material.shear_modulus
                ),
            ));
        }
        if self.material.poissons_ratio.is_some_and(|value| !value.is_finite() || value <= -1.0 || value >= 0.5) {
            report.push(check_error(
                "material/invalid-poisson",
                format!(
                    "Poisson's ratio of material `{}` must be finite and inside (-1, 0.5) (currently {:?})",
                    self.material.name, self.material.poissons_ratio
                ),
            ));
        }
        if self.ambient_temperature.is_some_and(|value| !value.is_finite()) {
            report.push(check_error(
                "heat/invalid-ambient",
                format!("ambient temperature must be finite (currently {:?})", self.ambient_temperature),
            ));
        }
        let section_properties = match self.material.section {
            Some(section) => match section.properties() {
                Ok(properties) => Some(properties),
                Err(error) => {
                    report.push(check_error(&error.code, error.message));
                    None
                }
            },
            None => None,
        };
        let section_inertia = section_properties.map(|properties| properties.inertia_strong);
        let section_torsion = section_properties.map(|properties| properties.torsion_constant);

        let mut node_ids = HashSet::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if !node_ids.insert(node.id.as_str()) {
                report.push(check_error("node/duplicate-id", format!("node id `{}` is duplicated", node.id)));
            }
            if !node.mass.is_finite() || node.mass < 0.0 {
                report.push(check_error(
                    "node/invalid-mass",
                    format!("mass of node `{}` must be finite and non-negative (currently {})", node.id, node.mass),
                ));
            }
            if node.temperature.is_some_and(|value| !value.is_finite()) {
                report.push(check_error(
                    "node/invalid-temperature",
                    format!("fixed temperature of node `{}` must be finite", node.id),
                ));
            }
            if !node.heat.is_finite() {
                report.push(check_error(
                    "load/invalid-heat",
                    format!("heat flow of node `{}` must be finite (currently {})", node.id, node.heat),
                ));
            }
            if self.dimension == 2 && node.z != 0.0 {
                report.push(check_error(
                    "document/dimension-mismatch",
                    format!(
                        "node `{}` in a two-dimensional document must have z = 0 (currently z={}); set dimension to 3 for 3D models",
                        node.id, node.z
                    ),
                ));
            }
            if let Some(other) =
                self.nodes[..index].iter().find(|other| other.x == node.x && other.y == node.y && other.z == node.z)
            {
                report.push(check_warning(
                    "node/duplicate-coordinates",
                    format!(
                        "node `{}` shares its coordinates ({}, {}, {}) with node `{}`; two distinct nodes at the same location have no connecting stiffness",
                        node.id, node.x, node.y, node.z, other.id
                    ),
                ));
            }
        }

        let connected: HashSet<&str> =
            self.elements.iter().flat_map(|element| [element.from.as_str(), element.to.as_str()]).collect();
        for node in &self.nodes {
            if !connected.contains(node.id.as_str()) {
                report.push(check_warning(
                    "node/unconnected",
                    format!(
                        "node `{}` is not referenced by any element and contributes no stiffness; remove it or connect it",
                        node.id
                    ),
                ));
            }
        }

        let mut element_ids = HashSet::new();
        for element in &self.elements {
            if !element_ids.insert(element.id.as_str()) {
                report.push(check_error("element/duplicate-id", format!("element id `{}` is duplicated", element.id)));
            }
        }

        let mut element_pairs = HashSet::new();
        for element in &self.elements {
            let start = self.node(&element.from);
            let end = self.node(&element.to);
            if start.is_none() || end.is_none() {
                let missing = if start.is_none() { &element.from } else { &element.to };
                report.push(check_error(
                    "element/unknown-node",
                    format!("element `{}` references unknown node `{missing}`", element.id),
                ));
            }
            if element.from == element.to {
                report.push(check_error(
                    "element/degenerate",
                    format!("element `{}` has identical start and end nodes", element.id),
                ));
            }
            else if let (Some(start), Some(end)) = (start, end) {
                let delta_x = end.x - start.x;
                let delta_y = end.y - start.y;
                let delta_z = end.z - start.z;
                let length = (delta_x * delta_x + delta_y * delta_y + delta_z * delta_z).sqrt();
                if length == 0.0 {
                    report.push(check_error("element/zero-length", format!("element `{}` has zero length", element.id)));
                }
                let pair = if element.from <= element.to {
                    (element.from.as_str(), element.to.as_str())
                }
                else {
                    (element.to.as_str(), element.from.as_str())
                };
                if !element_pairs.insert(pair) {
                    report.push(check_error(
                        "element/duplicate",
                        format!(
                            "element `{}` duplicates an existing element (same node pair {} / {})",
                            element.id, element.from, element.to
                        ),
                    ));
                }
            }
            if let Some(material) = &element.material {
                if material.youngs_modulus.is_some_and(|value| !positive_finite(value))
                    || material.area.is_some_and(|value| !positive_finite(value))
                {
                    report.push(check_error(
                        "material/invalid",
                        format!("E/A overrides of element `{}` must be positive", element.id),
                    ));
                }
            }
            if element.kind == ElementKind::Beam {
                if self.dimension != 2 {
                    report.push(check_error(
                        "capability/not-implemented",
                        format!(
                            "3D beam elements (element `{}`) are not implemented; beam supports dimension=2 only",
                            element.id
                        ),
                    ));
                }
                let usable_inertia = match element.inertia {
                    Some(value) => positive_finite(value),
                    None => section_inertia.is_some_and(positive_finite),
                };
                if !usable_inertia {
                    report.push(check_error(
                        "element/invalid-inertia",
                        match element.inertia {
                            Some(value) => format!(
                                "beam element `{}` inertia must be positive and finite (currently {value})",
                                element.id
                            ),
                            None => format!(
                                "beam element `{}` requires an explicit positive inertia or a material section that provides one",
                                element.id
                            ),
                        },
                    ));
                }
            }
            if element.kind == ElementKind::Beam3D {
                if self.dimension != 3 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!("beam3d element `{}` requires a dimension=3 document", element.id),
                    ));
                }
                let usable_inertia = match element.inertia {
                    Some(value) => positive_finite(value),
                    None => section_inertia.is_some_and(positive_finite),
                };
                if !usable_inertia {
                    report.push(check_error(
                        "element/invalid-inertia",
                        match element.inertia {
                            Some(value) => format!(
                                "beam3d element `{}` inertia must be positive and finite (currently {value})",
                                element.id
                            ),
                            None => format!(
                                "beam3d element `{}` requires an explicit positive inertia or a material section that provides one",
                                element.id
                            ),
                        },
                    ));
                }
                if element.inertia_weak.is_some_and(|value| !positive_finite(value)) {
                    report.push(check_error(
                        "element/invalid-inertia",
                        format!("beam3d element `{}` inertiaWeak must be positive and finite", element.id),
                    ));
                }
                let usable_torsion = match element.torsion {
                    Some(value) => positive_finite(value),
                    None => section_torsion.is_some_and(positive_finite),
                };
                if !usable_torsion {
                    report.push(check_error(
                        "element/invalid-torsion",
                        match element.torsion {
                            Some(value) => format!(
                                "beam3d element `{}` torsion constant J must be positive and finite (currently {value})",
                                element.id
                            ),
                            None => format!(
                                "beam3d element `{}` requires an explicit positive torsion constant J or a material section that provides one",
                                element.id
                            ),
                        },
                    ));
                }
                if element.orientation_vector.is_some_and(|vector| !vector.iter().all(|value| value.is_finite()))
                    || element.orientation_angle.is_some_and(|value| !value.is_finite())
                {
                    report.push(check_error(
                        "element/invalid-orientation",
                        format!("orientation data of beam3d element `{}` must be finite", element.id),
                    ));
                }
                if element.release.is_some() {
                    report.push(check_error(
                        "element/release-requires-beam",
                        format!(
                            "end releases are only implemented for two-dimensional beam elements (element `{}`)",
                            element.id
                        ),
                    ));
                }
                if element.temperature_gradient.is_some() {
                    report.push(check_error(
                        "element/gradient-requires-beam",
                        format!(
                            "temperature gradients are only implemented for two-dimensional beam elements (element `{}`)",
                            element.id
                        ),
                    ));
                }
            }
            if element.distributed_load.is_some()
                || element.distributed_load_end.is_some()
                || element.distributed_load_from.is_some()
                || element.distributed_load_to.is_some()
            {
                if element.kind == ElementKind::Truss {
                    report.push(check_error(
                        "element/distributed-load-requires-beam",
                        format!(
                            "element `{}` is a truss element; distributed load w can only be applied to beam elements",
                            element.id
                        ),
                    ));
                }
                if element.distributed_load.is_some_and(|value| !value.is_finite())
                    || element.distributed_load_end.is_some_and(|value| !value.is_finite())
                {
                    report.push(check_error(
                        "element/invalid-distributed-load",
                        format!("distributed load w of element `{}` must be finite", element.id),
                    ));
                }
            }
            if element.distributed_load_z.is_some() || element.distributed_load_z_end.is_some() {
                if element.kind != ElementKind::Beam3D {
                    report.push(check_error(
                        "element/distributed-load-requires-beam",
                        format!(
                            "element `{}` is not a beam3d member; distributed load wz can only be applied to beam3d members",
                            element.id
                        ),
                    ));
                }
                if element.distributed_load_z.is_some_and(|value| !value.is_finite())
                    || element.distributed_load_z_end.is_some_and(|value| !value.is_finite())
                {
                    report.push(check_error(
                        "element/invalid-distributed-load",
                        format!("distributed load wz of element `{}` must be finite", element.id),
                    ));
                }
            }
            match (element.distributed_load_from, element.distributed_load_to) {
                (None, None) => {}
                (Some(start), Some(end)) => {
                    if !start.is_finite() || !end.is_finite() || start < 0.0 || end > 1.0 || start >= end {
                        report.push(check_error(
                            "element/invalid-load-span",
                            format!(
                                "load span of element `{}` must satisfy 0 <= start < end <= 1 (currently {start}..{end})",
                                element.id
                            ),
                        ));
                    }
                    if element.distributed_load.is_none() {
                        report.push(check_error(
                            "element/missing-distributed-load",
                            format!(
                                "element `{}` limits a distributed load span but does not declare the load intensity",
                                element.id
                            ),
                        ));
                    }
                }
                _ => report.push(check_error(
                    "element/invalid-load-span",
                    format!(
                        "element `{}` must declare both the start and the end fraction of a distributed load span",
                        element.id
                    ),
                )),
            }
            if let Some(delta) = element.temperature {
                if !delta.is_finite() {
                    report.push(check_error(
                        "element/invalid-temperature",
                        format!("temperature change of element `{}` must be finite", element.id),
                    ));
                }
                else if self.material.alpha.is_none() {
                    report.push(check_error(
                        "material/missing-alpha",
                        format!(
                            "element `{}` declares a temperature change but the material does not define a coefficient of thermal expansion (alpha)",
                            element.id
                        ),
                    ));
                }
                else if self.material.alpha == Some(0.0) && delta != 0.0 {
                    report.push(check_warning(
                        "element/inactive-temperature",
                        format!(
                            "element `{}` declares a temperature change but alpha is zero, so no thermal strain is produced",
                            element.id
                        ),
                    ));
                }
            }
            if let Some(gradient) = element.temperature_gradient {
                if !gradient.is_finite() {
                    report.push(check_error(
                        "element/invalid-temperature-gradient",
                        format!("temperature gradient of element `{}` must be finite", element.id),
                    ));
                }
                else if element.kind != ElementKind::Beam {
                    report.push(check_error(
                        "element/gradient-requires-beam",
                        format!(
                            "element `{}` is a truss element and cannot carry a temperature gradient; use a beam element",
                            element.id
                        ),
                    ));
                }
                else if self.material.alpha.is_none() {
                    report.push(check_error(
                        "material/missing-alpha",
                        format!(
                            "element `{}` declares a temperature gradient but the material does not define a coefficient of thermal expansion (alpha)",
                            element.id
                        ),
                    ));
                }
                else if self.material.alpha == Some(0.0) && gradient != 0.0 {
                    report.push(check_warning(
                        "element/inactive-temperature",
                        format!(
                            "element `{}` declares a temperature gradient but alpha is zero, so no thermal curvature is produced",
                            element.id
                        ),
                    ));
                }
                else if gradient != 0.0 && section_inertia.is_none() {
                    report.push(check_error(
                        "material/missing-section",
                        format!(
                            "element `{}` declares a temperature gradient but the material does not define a parametric section that provides the depth",
                            element.id
                        ),
                    ));
                }
            }
            if let Some(convection) = element.convection {
                if !positive_finite(convection) {
                    report.push(check_error(
                        "heat/invalid-convection",
                        format!(
                            "convection coefficient of element `{}` must be positive and finite (currently {convection})",
                            element.id
                        ),
                    ));
                }
                match element.perimeter {
                    None => report.push(check_error(
                        "heat/missing-perimeter",
                        format!("element `{}` declares convection but no perimeter", element.id),
                    )),
                    Some(perimeter) if !positive_finite(perimeter) => report.push(check_error(
                        "heat/invalid-perimeter",
                        format!("perimeter of element `{}` must be positive and finite (currently {perimeter})", element.id),
                    )),
                    Some(_) => {}
                }
                if self.ambient_temperature.is_none() {
                    report.push(check_error(
                        "heat/missing-ambient",
                        "surface convection requires a finite model ambient temperature",
                    ));
                }
            }
            else if element.perimeter.is_some_and(|value| !positive_finite(value)) {
                report.push(check_error(
                    "heat/invalid-perimeter",
                    format!(
                        "perimeter of element `{}` must be positive and finite (currently {:?})",
                        element.id, element.perimeter
                    ),
                ));
            }
            if let Some(release) = element.release {
                if element.kind != ElementKind::Beam {
                    report.push(check_error(
                        "element/release-requires-beam",
                        format!(
                            "element `{}` is a truss element and cannot have bending-moment releases; use a beam element",
                            element.id
                        ),
                    ));
                }
                else if release.is_empty() {
                    report.push(check_error(
                        "element/invalid-release",
                        format!("release of element `{}` must free at least one end", element.id),
                    ));
                }
            }
            if element.tension_only && element.kind != ElementKind::Truss {
                report.push(check_error(
                    "element/tension-only-requires-truss",
                    format!(
                        "element `{}` is a beam element and cannot be tension-only; the tension-only flag is limited to truss elements",
                        element.id
                    ),
                ));
            }
        }

        let rotation_nodes = rotation_nodes(self);
        let spatial_nodes = spatial_beam_nodes(self);
        for support in &self.supports {
            if self.node(&support.node).is_none() {
                report.push(check_error("support/unknown-node", format!("support references unknown node `{}`", support.node)));
            }
            if self.dimension == 2 && support.dz {
                report.push(check_error(
                    "document/dimension-mismatch",
                    format!(
                        "node `{}` in a two-dimensional document cannot constrain Z translation (dz); set dimension to 3 for 3D models",
                        support.node
                    ),
                ));
            }
            if let Some(direction) = support.direction {
                if self.dimension != 2 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!(
                            "node `{}` uses an inclined support direction, which is only available in two-dimensional documents",
                            support.node
                        ),
                    ));
                }
                else {
                    let norm = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
                    if !norm.is_finite() || norm <= 0.0 {
                        report.push(check_error(
                            "support/invalid-direction",
                            format!(
                                "inclined support direction of node `{}` must be a finite non-zero vector (currently ({}, {}))",
                                support.node, direction[0], direction[1]
                            ),
                        ));
                    }
                }
                if support.dx || support.dy || support.prescribed.is_some() || support.spring.is_some() {
                    report.push(check_error(
                        "support/invalid-direction",
                        format!(
                            "node `{}` combines an inclined direction with a global constraint, prescribed displacement or spring; use one constraint frame per support entry",
                            support.node
                        ),
                    ));
                }
            }
            if support.rz {
                if self.dimension == 3 && !spatial_nodes.contains(support.node.as_str()) {
                    report.push(check_error(
                        "support/inactive-dof",
                        format!("node `{}` has no rz rotational degree of freedom: it has no beam3d element", support.node),
                    ));
                }
                else if self.dimension == 2 && !rotation_nodes.contains(support.node.as_str()) {
                    report.push(check_error(
                        "support/inactive-dof",
                        format!("node `{}` has no rz rotational degree of freedom: it has no beam element, or every beam end at the node is released", support.node),
                    ));
                }
            }
            for (name, constrained) in [("rx", support.rx), ("ry", support.ry)] {
                if !constrained {
                    continue;
                }
                if self.dimension != 3 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!(
                            "node `{}` in a two-dimensional document cannot constrain the {name} rotation; set dimension to 3",
                            support.node
                        ),
                    ));
                }
                else if !spatial_nodes.contains(support.node.as_str()) {
                    report.push(check_error(
                        "support/inactive-dof",
                        format!("node `{}` has no {name} rotational degree of freedom: it has no beam3d element", support.node),
                    ));
                }
            }
            if let Some(spring) = &support.spring {
                let components = [
                    ("kx", spring.kx, support.dx),
                    ("ky", spring.ky, support.dy),
                    ("kz", spring.kz, support.dz),
                    ("kr", spring.kr, support.rz),
                ];
                for (name, stiffness, constrained) in components {
                    if !stiffness.is_finite() || stiffness < 0.0 {
                        report.push(check_error(
                            "support/invalid-spring",
                            format!(
                                "spring stiffness {name} of node `{}` must be finite and non-negative (currently {stiffness})",
                                support.node
                            ),
                        ));
                    }
                    else if stiffness > 0.0 && constrained {
                        report.push(check_error(
                            "support/spring-constrained",
                            format!(
                                "node `{}` defines a {name} spring and also rigidly constrains that degree of freedom; remove one of them",
                                support.node
                            ),
                        ));
                    }
                }
                if spring.is_empty() {
                    report.push(check_error(
                        "support/invalid-spring",
                        format!(
                            "spring stiffness of node `{}` is zero; an elastic support needs a positive stiffness",
                            support.node
                        ),
                    ));
                }
                if spring.kz > 0.0 && self.dimension != 3 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!(
                            "node `{}` in a two-dimensional document cannot use a kz spring; set dimension to 3 for 3D models",
                            support.node
                        ),
                    ));
                }
                if spring.kr > 0.0 {
                    if self.dimension == 3 {
                        report.push(check_error(
                            "document/dimension-mismatch",
                            format!(
                                "node `{}` in a three-dimensional document cannot use a kr spring; beam elements are two-dimensional only",
                                support.node
                            ),
                        ));
                    }
                    else if !rotation_nodes.contains(support.node.as_str()) {
                        report.push(check_error(
                            "support/inactive-dof",
                            format!(
                                "node `{}` has no rotational degree of freedom for a kr spring: it has no beam element, or every beam end at the node is released",
                                support.node
                            ),
                        ));
                    }
                }
            }
            if let Some(prescribed) = &support.prescribed {
                let components =
                    [("ux", support.dx, prescribed.ux), ("uy", support.dy, prescribed.uy), ("rz", support.rz, prescribed.rz)];
                for (name, constrained, value) in components {
                    if !value.is_finite() {
                        report.push(check_error(
                            "support/invalid-prescribed",
                            format!("prescribed `{name}` of node `{}` must be finite", support.node),
                        ));
                    }
                    else if value != 0.0 && !constrained {
                        report.push(check_error(
                            "support/prescribed-unconstrained",
                            format!(
                                "node `{}` prescribes {name}={value}, but the matching support flag is not set",
                                support.node
                            ),
                        ));
                    }
                }
                if prescribed.uz != 0.0 {
                    if self.dimension != 3 {
                        report.push(check_error(
                            "document/dimension-mismatch",
                            format!("node `{}` in a two-dimensional document cannot prescribe a Z displacement (uz); set dimension to 3 for 3D models", support.node),
                        ));
                    }
                    else if !support.dz {
                        report.push(check_error(
                            "support/prescribed-unconstrained",
                            format!(
                                "node `{}` prescribes uz={}, but the matching support flag is not set",
                                support.node, prescribed.uz
                            ),
                        ));
                    }
                }
                if self.dimension == 3 && prescribed.rz != 0.0 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!("node `{}` in a three-dimensional document cannot prescribe an rz rotation; beam elements are two-dimensional only", support.node),
                    ));
                }
            }
        }

        for load in &self.loads {
            if self.node(&load.node).is_none() {
                report.push(check_error("load/unknown-node", format!("load references unknown node `{}`", load.node)));
            }
            if self.dimension == 2 && load.fz != 0.0 {
                report.push(check_error(
                    "document/dimension-mismatch",
                    format!(
                        "node `{}` in a two-dimensional document cannot carry a Z load fz (currently fz={}); set dimension to 3 for 3D models",
                        load.node, load.fz
                    ),
                ));
            }
            if load.mz != 0.0 {
                if self.dimension == 3 && !spatial_nodes.contains(load.node.as_str()) {
                    report.push(check_error(
                        "load/inactive-dof",
                        format!(
                            "node `{}` has no rz rotational degree of freedom for the mz moment: it has no beam3d element",
                            load.node
                        ),
                    ));
                }
                else if self.dimension == 2 && !rotation_nodes.contains(load.node.as_str()) {
                    report.push(check_error(
                        "load/inactive-dof",
                        format!("node `{}` has no rotational degree of freedom for the mz moment: it has no beam element, or every beam end at the node is released", load.node),
                    ));
                }
            }
            for (name, moment) in [("mx", load.mx), ("my", load.my)] {
                let Some(value) = moment.filter(|value| *value != 0.0)
                else {
                    continue;
                };
                if self.dimension != 3 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!("node `{}` in a two-dimensional document cannot carry a {name} moment (currently {value}); set dimension to 3", load.node),
                    ));
                }
                else if !spatial_nodes.contains(load.node.as_str()) {
                    report.push(check_error(
                        "load/inactive-dof",
                        format!(
                            "node `{}` has no rotational degree of freedom for the {name} moment: it has no beam3d element",
                            load.node
                        ),
                    ));
                }
            }
        }

        let mut pattern_names = HashSet::new();
        for pattern in &self.patterns {
            if pattern.name.trim().is_empty() {
                report.push(check_error("pattern/invalid-name", "a load pattern requires a non-empty name"));
            }
            else if !pattern_names.insert(pattern.name.as_str()) {
                report
                    .push(check_error("pattern/duplicate-name", format!("load pattern name `{}` is duplicated", pattern.name)));
            }
            for load in &pattern.loads {
                if self.node(&load.node).is_none() {
                    report.push(check_error("load/unknown-node", format!("load references unknown node `{}`", load.node)));
                }
                if self.dimension == 2 && load.fz != 0.0 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!(
                            "node `{}` in a two-dimensional document cannot carry a Z load fz (currently fz={}); set dimension to 3 for 3D models",
                            load.node, load.fz
                        ),
                    ));
                }
                if load.mz != 0.0 {
                    if self.dimension == 3 && !spatial_nodes.contains(load.node.as_str()) {
                        report.push(check_error(
                            "load/inactive-dof",
                            format!(
                                "node `{}` has no rz rotational degree of freedom for the mz moment: it has no beam3d element",
                                load.node
                            ),
                        ));
                    }
                    else if self.dimension == 2 && !rotation_nodes.contains(load.node.as_str()) {
                        report.push(check_error(
                            "load/inactive-dof",
                            format!("node `{}` has no rotational degree of freedom for the mz moment: it has no beam element, or every beam end at the node is released", load.node),
                        ));
                    }
                }
            }
        }

        if let Some(gravity) = self.gravity {
            if gravity.iter().any(|value| !value.is_finite()) {
                report.push(check_error("document/invalid-gravity", "gravity acceleration g components must be finite"));
            }
            if self.dimension != 3 && gravity[2] != 0.0 {
                report.push(check_error(
                    "document/dimension-mismatch",
                    format!("a two-dimensional document cannot set a non-zero gz (currently gz={}); set dimension to 3 for 3D models", gravity[2]),
                ));
            }
            if !positive_finite(self.material.density) {
                report.push(check_error(
                    "material/invalid-density",
                    format!(
                        "gravity requires a positive mass density (currently {}); use t/mm³ in the millimetre system",
                        self.material.density
                    ),
                ));
            }
        }

        let mut combination_names = HashSet::new();
        for combination in &self.combinations {
            if combination.name.trim().is_empty() {
                report.push(check_error("combination/invalid-name", "a load combination requires a non-empty name"));
            }
            else if !combination_names.insert(combination.name.as_str()) {
                report.push(check_error(
                    "combination/duplicate-name",
                    format!("load combination name `{}` is duplicated", combination.name),
                ));
            }
            if !combination.loads.is_finite() || !combination.gravity.is_finite() || !combination.thermal.is_finite() {
                report.push(check_error(
                    "combination/invalid-factor",
                    format!("load combination `{}` factors must be finite", combination.name),
                ));
            }
            else if combination.gravity != 0.0 && self.gravity.is_none() {
                report.push(check_notice(
                    "combination/no-gravity",
                    format!(
                        "load combination `{}` scales gravity by {}, but the document does not define a gravity vector",
                        combination.name, combination.gravity
                    ),
                ));
            }
            let mut pattern_refs = HashSet::new();
            for entry in &combination.patterns {
                if entry.pattern.trim().is_empty() {
                    report.push(check_error(
                        "combination/invalid-name",
                        format!("load combination `{}` references a pattern with an empty name", combination.name),
                    ));
                }
                else if !pattern_refs.insert(entry.pattern.as_str()) {
                    report.push(check_error(
                        "combination/duplicate-pattern",
                        format!(
                            "load combination `{}` references pattern `{}` more than once",
                            combination.name, entry.pattern
                        ),
                    ));
                }
                else if !self.patterns.iter().any(|pattern| pattern.name == entry.pattern) {
                    report.push(check_error(
                        "combination/unknown-pattern",
                        format!("load combination `{}` references unknown load pattern `{}`", combination.name, entry.pattern),
                    ));
                }
                if !entry.factor.is_finite() {
                    report.push(check_error(
                        "combination/invalid-factor",
                        format!(
                            "pattern `{}` of load combination `{}` requires a finite factor",
                            entry.pattern, combination.name
                        ),
                    ));
                }
            }
        }

        // The solver and exporters never convert units, so a density or gravity
        // value written in the other system silently scales every body load.
        // The thresholds below catch the two common mix-ups (kg/m³ in a `mm`
        // document and m/s² in a `mm` document) without touching the numbers.
        if let Some(units) = self.units {
            let density = self.material.density;
            let density_mismatch = match units {
                Units::Millimeter => density > 1.0e-6,
                Units::Meter => density > 0.0 && density < 1.0e-3,
            };
            if density > 0.0 && density.is_finite() && density_mismatch {
                let expected = match units {
                    Units::Millimeter => "t/mm³ (steel is about 7.85e-9)",
                    Units::Meter => "kg/m³ (steel is about 7850)",
                };
                report.push(check_warning(
                    "units/density-scale",
                    format!(
                        "density of material `{}` is {density}, which does not look like the `{}` system, where density is {expected}; units are never converted, so every body load uses this value unchanged",
                        self.material.name,
                        units.as_str()
                    ),
                ));
            }
            if let Some(gravity) = self.gravity {
                let magnitude = (gravity[0] * gravity[0] + gravity[1] * gravity[1] + gravity[2] * gravity[2]).sqrt();
                let gravity_mismatch = match units {
                    Units::Millimeter => (9.5..=10.0).contains(&magnitude),
                    Units::Meter => (9_500.0..=10_000.0).contains(&magnitude),
                };
                if gravity_mismatch {
                    let expected = match units {
                        Units::Millimeter => "mm/s² (standard gravity is 9810)",
                        Units::Meter => "m/s² (standard gravity is 9.81)",
                    };
                    report.push(check_warning(
                        "units/gravity-scale",
                        format!(
                            "gravity magnitude {magnitude} looks like the other unit system; a `{}` document expects acceleration in {expected}, and units are never converted",
                            units.as_str()
                        ),
                    ));
                }
            }
        }

        // Reference integrity: a pattern no combination uses, or a combination
        // that scales nothing, is a silent zero rather than a solve failure.
        // `thermal` defaults to 1, so a zero-thermal check alone would miss the
        // common empty case; the temperature actions decide whether that factor
        // can contribute anything.
        for pattern in &self.patterns {
            let referenced = self
                .combinations
                .iter()
                .any(|combination| combination.patterns.iter().any(|entry| entry.pattern == pattern.name));
            if !referenced {
                report.push(check_notice(
                    "pattern/unused",
                    format!(
                        "load pattern `{}` is defined but no load combination references it, so it never affects a result",
                        pattern.name
                    ),
                ));
            }
        }
        let has_thermal_actions =
            self.elements.iter().any(|element| element.temperature.is_some() || element.temperature_gradient.is_some());
        for combination in &self.combinations {
            let thermal_contributes = combination.thermal != 0.0 && has_thermal_actions;
            if combination.loads == 0.0 && combination.gravity == 0.0 && combination.patterns.is_empty() && !thermal_contributes
            {
                report.push(check_notice(
                    "combination/empty",
                    format!(
                        "load combination `{}` scales no active action (loads, gravity, patterns and temperature contribute nothing), so it produces a zero result",
                        combination.name
                    ),
                ));
            }
        }

        if report.has_errors() {
            return into_issues(report);
        }

        // Stability is decided by the numerical rank of the reduced stiffness
        // matrix, so the same rule covers two and three dimensions and does not
        // rely on a hard-coded minimum number of constrained translations.
        if let Err(nodes) = check_stability(self) {
            report.push(mechanism_issue(nodes));
        }

        into_issues(report)
    }
}

fn into_issues(report: CheckReport) -> Vec<Issue> {
    report.issues.into_iter().map(Issue::from).collect()
}

fn mechanism_issue(unconstrained: Vec<String>) -> CheckIssue {
    let detail = if unconstrained.is_empty() {
        "the stiffness matrix is singular; enough supports exist but some constraints are collinear or degenerate".to_string()
    }
    else {
        format!(
            "the stiffness matrix is singular; these nodes are not constrained by any support: {}",
            unconstrained.join(", ")
        )
    };
    check_error("model/mechanism", format!("the structure contains a mechanism: {detail}"))
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
