use std::collections::HashSet;

use boost_check::{CheckReport, Issue as CheckIssue, Severity as CheckSeverity};
use serde::Serialize;

use crate::{
    model::{ElementKind, FORMAT, Model, VERSION, beam_nodes},
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

        let mut node_ids = HashSet::new();
        for node in &self.nodes {
            if !node_ids.insert(node.id.as_str()) {
                report.push(check_error("node/duplicate-id", format!("node id `{}` is duplicated", node.id)));
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
                if !element.inertia.is_some_and(positive_finite) {
                    report.push(check_error(
                        "element/invalid-inertia",
                        format!("beam element `{}` requires a positive finite inertia", element.id),
                    ));
                }
            }
            if element.distributed_load.is_some() || element.distributed_load_end.is_some() {
                if element.kind != ElementKind::Beam {
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
        }

        let beam_nodes = beam_nodes(self);
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
            if support.rz {
                if self.dimension == 3 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!("node `{}` in a three-dimensional document cannot constrain rz rotation; beam elements are two-dimensional only", support.node),
                    ));
                }
                else if !beam_nodes.contains(support.node.as_str()) {
                    report.push(check_error(
                        "support/inactive-dof",
                        format!("node `{}` is not connected to any beam element, so it has no rz rotational degree of freedom to constrain", support.node),
                    ));
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
                if self.dimension == 3 {
                    report.push(check_error(
                        "document/dimension-mismatch",
                        format!("node `{}` in a three-dimensional document cannot carry an mz moment; beam elements are two-dimensional only", load.node),
                    ));
                }
                else if !beam_nodes.contains(load.node.as_str()) {
                    report.push(check_error(
                        "load/inactive-dof",
                        format!("node `{}` is not connected to any beam element, so it has no rotational degree of freedom for the mz moment", load.node),
                    ));
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
