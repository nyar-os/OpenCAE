//! Load-combination envelope report.
//!
//! The envelope reuses [`crate::solve_combinations`] unchanged: every declared
//! combination is solved on its scaled model by the static combination path,
//! and the unfactored document is always solved as the `base` case first, the
//! same way [`crate::summarize_load_cases`] reports `base`. A document without
//! combinations therefore still produces a valid base-only envelope.
//!
//! One row per node reports the minimum and maximum of `ux`, `uy` (and `uz` in
//! three-dimensional documents) with the combination that produced each
//! extreme. One row per element reports the signed axial force, shear and
//! bending moment. Shear and moment use the signed end force with the larger
//! absolute value (ties keep the start end), matching the end forces printed by
//! `element forces`; a truss therefore reports zero shear and moment. One row
//! per support reports the `fx`, `fy` (and `fz` in three dimensions) reaction
//! extremes, exactly the supports listed by `reactions`.
//!
//! Values stay in the document unit system and are not converted. Ties go to
//! the earlier case in `cases` order, so the report is deterministic for an
//! identical document.

use serde::{Deserialize, Serialize};

use crate::{
    CaeError,
    model::{Model, Units},
    solver::{ElementResult, Solution, solve, solve_combinations},
};

/// Version of the JSON report produced by [`solve_envelope`].
pub const ENVELOPE_REPORT_VERSION: u32 = 1;

/// Format tag of the load-combination envelope JSON report.
pub const ENVELOPE_REPORT_FORMAT: &str = "open-cae-envelope";

/// Minimum, maximum and governing combination of one signed value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeExtreme {
    /// Lowest signed value across the solved cases.
    pub min: f64,
    /// Highest signed value across the solved cases.
    pub max: f64,
    /// Combination that produced `min`; ties keep the earlier case.
    pub min_combination: String,
    /// Combination that produced `max`; ties keep the earlier case.
    pub max_combination: String,
}

impl EnvelopeExtreme {
    fn started(value: f64, combination: &str) -> Self {
        Self { min: value, max: value, min_combination: combination.to_string(), max_combination: combination.to_string() }
    }

    fn observe(&mut self, value: f64, combination: &str) {
        if value < self.min {
            self.min = value;
            self.min_combination = combination.to_string();
        }
        if value > self.max {
            self.max = value;
            self.max_combination = combination.to_string();
        }
    }
}

/// Displacement envelope of one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeNode {
    /// Node identifier.
    pub id: String,
    /// Translation along X.
    pub ux: EnvelopeExtreme,
    /// Translation along Y.
    pub uy: EnvelopeExtreme,
    /// Translation along Z; omitted for two-dimensional documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uz: Option<EnvelopeExtreme>,
}

/// Force envelope of one element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeElement {
    /// Element identifier.
    pub id: String,
    /// Signed axial force, positive in tension.
    pub axial: EnvelopeExtreme,
    /// Signed shear of the end with the larger absolute shear.
    pub shear: EnvelopeExtreme,
    /// Signed bending moment of the end with the larger absolute moment.
    pub moment: EnvelopeExtreme,
}

/// Reaction envelope of one supported node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeReaction {
    /// Support node identifier.
    pub node: String,
    /// Reaction force along X.
    pub fx: EnvelopeExtreme,
    /// Reaction force along Y.
    pub fy: EnvelopeExtreme,
    /// Reaction force along Z; omitted for two-dimensional documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fz: Option<EnvelopeExtreme>,
}

/// Result of [`solve_envelope`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeReport {
    /// Report format tag, always `open-cae-envelope`.
    pub format: String,
    /// Report version.
    pub version: u32,
    /// Unit system declared by the model.
    pub units: String,
    /// Number of load combinations declared in the document.
    pub combination_count: usize,
    /// Solved case names in report order; `base` is always first.
    pub cases: Vec<String>,
    /// Per-node displacement extremes.
    pub nodes: Vec<EnvelopeNode>,
    /// Per-element force extremes.
    pub elements: Vec<EnvelopeElement>,
    /// Per-support reaction extremes.
    pub reactions: Vec<EnvelopeReaction>,
    /// `true` when every case solved successfully.
    pub ok: bool,
}

/// Solves the unfactored case and every declared combination, then reports the
/// minimum and maximum of each displacement, force and reaction component.
///
/// The unfactored document is solved first and appears as `base` in `cases`.
/// Each combination is then solved by [`solve_combinations`], so scaling,
/// validation and the stable error codes are exactly those of the combination
/// report; a failing case is returned as its usual error instead of a partial
/// envelope. A document without combinations reports the base case only.
pub fn solve_envelope(model: &Model) -> Result<EnvelopeReport, CaeError> {
    let mut cases: Vec<(String, Solution)> = Vec::with_capacity(1 + model.combinations.len());
    cases.push(("base".to_string(), solve(model)?));
    if !model.combinations.is_empty() {
        for case in solve_combinations(model, &model.combinations)?.combinations {
            cases.push((case.name, case.solution));
        }
    }
    Ok(build_envelope(model, &cases))
}

fn build_envelope(model: &Model, cases: &[(String, Solution)]) -> EnvelopeReport {
    let three_dimensional = model.dimension == 3;
    let (base_name, base) = &cases[0];

    let nodes = base
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let mut ux = EnvelopeExtreme::started(node.ux, base_name);
            let mut uy = EnvelopeExtreme::started(node.uy, base_name);
            let mut uz = three_dimensional.then(|| EnvelopeExtreme::started(node.uz, base_name));
            for (name, solution) in &cases[1..] {
                let current = &solution.nodes[index];
                ux.observe(current.ux, name);
                uy.observe(current.uy, name);
                if let Some(uz) = &mut uz {
                    uz.observe(current.uz, name);
                }
            }
            EnvelopeNode { id: node.id.clone(), ux, uy, uz }
        })
        .collect();

    let elements = base
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let mut axial = EnvelopeExtreme::started(element.axial_force, base_name);
            let mut shear = EnvelopeExtreme::started(controlling_shear(element), base_name);
            let mut moment = EnvelopeExtreme::started(controlling_moment(element), base_name);
            for (name, solution) in &cases[1..] {
                let current = &solution.elements[index];
                axial.observe(current.axial_force, name);
                shear.observe(controlling_shear(current), name);
                moment.observe(controlling_moment(current), name);
            }
            EnvelopeElement { id: element.id.clone(), axial, shear, moment }
        })
        .collect();

    let reactions = base
        .reaction_summary
        .supports
        .iter()
        .enumerate()
        .map(|(index, support)| {
            let mut fx = EnvelopeExtreme::started(support.fx, base_name);
            let mut fy = EnvelopeExtreme::started(support.fy, base_name);
            let mut fz = three_dimensional.then(|| EnvelopeExtreme::started(support.fz, base_name));
            for (name, solution) in &cases[1..] {
                let current = &solution.reaction_summary.supports[index];
                fx.observe(current.fx, name);
                fy.observe(current.fy, name);
                if let Some(fz) = &mut fz {
                    fz.observe(current.fz, name);
                }
            }
            EnvelopeReaction { node: support.node.clone(), fx, fy, fz }
        })
        .collect();

    EnvelopeReport {
        format: ENVELOPE_REPORT_FORMAT.to_string(),
        version: ENVELOPE_REPORT_VERSION,
        units: model.units.unwrap_or(Units::Millimeter).as_str().to_string(),
        combination_count: model.combinations.len(),
        cases: cases.iter().map(|(name, _)| name.clone()).collect(),
        nodes,
        elements,
        reactions,
        ok: true,
    }
}

fn controlling_shear(element: &ElementResult) -> f64 {
    controlling_end(element.end_forces.v1, element.end_forces.v2)
}

fn controlling_moment(element: &ElementResult) -> f64 {
    controlling_end(element.end_forces.m1, element.end_forces.m2)
}

fn controlling_end(start: f64, end: f64) -> f64 {
    if end.abs() > start.abs() {
        end
    }
    else {
        start
    }
}
