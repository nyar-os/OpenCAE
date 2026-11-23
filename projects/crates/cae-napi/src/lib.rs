//! Thin Node-API surface for OpenCAE.
//!
//! Every export performs one orthogonal operation and returns a JSON string, so
//! the TypeScript layer owns argument parsing, help text and exit codes. The
//! document editing, validation, solving and export calls forward to
//! `cae-model`'s [`cae_model::OpenCae`] session, the same session the
//! `open-cae` CLI drives, so the entry points cannot disagree.
//!
//! Unimplemented routes keep the stable `capability/not-implemented` code. Bad
//! arguments report `session/argument-invalid`; every other failure keeps the
//! stable code of [`cae_model::CaeError`].

#![deny(missing_docs)]

use std::path::Path;

use cae_model::{
    CaeError, ConstructionStage, ElementKind, ElementUpdate, EndRelease, ExportFormat, ExportOptions, InfluenceDirection,
    InfluenceQuantity, InfluenceTrainRequest, Material, OpenCae, OpenCaeOptions, Section, SpectrumCombination, SpectrumPoint,
    Units, angle_section_report, channel_standard_section_report, ellipse_section_report, i_standard_section_report,
    library_json, polygon_section_report, principal_section_report, section_report, t_section_report, triangle_section_report,
    tube_standard_section_report, z_section_report,
};
use serde::Serialize;
use serde_json::Value;
use wae::{CallContext, Error, JsValue, Result, function, register_module};

fn argument_error(message: &str) -> Error {
    Error::new(format!("session/argument-invalid: {message}"))
}

fn argument(ctx: &CallContext, index: usize) -> Result<JsValue> {
    ctx.args().get(index).copied().ok_or_else(|| argument_error("a required argument is missing"))
}

fn text(ctx: &CallContext, index: usize) -> Result<String> {
    argument(ctx, index)?.as_string()
}

fn optional_text(ctx: &CallContext, index: usize) -> Result<Option<String>> {
    match ctx.args().get(index) {
        Some(value) => value.as_string().map(Some),
        None => Ok(None),
    }
}

fn object(ctx: &CallContext, index: usize) -> Result<Value> {
    let raw = text(ctx, index)?;
    serde_json::from_str(&raw).map_err(|error| argument_error(&format!("expected a JSON object: {error}")))
}

fn optional_object(ctx: &CallContext, index: usize) -> Result<Option<Value>> {
    match ctx.args().get(index) {
        Some(_) => object(ctx, index).map(Some),
        None => Ok(None),
    }
}

fn number_field(request: &Value, name: &str) -> Result<Option<f64>> {
    match request.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_f64().map(Some).ok_or_else(|| argument_error(&format!("`{name}` must be a number"))),
    }
}

fn required_number_field(request: &Value, name: &str) -> Result<f64> {
    number_field(request, name)?.ok_or_else(|| argument_error(&format!("`{name}` is required")))
}

fn text_field(request: &Value, name: &str) -> Result<String> {
    match request.get(name).and_then(Value::as_str) {
        Some(value) if !value.is_empty() => Ok(value.to_string()),
        _ => Err(argument_error(&format!("`{name}` must be a non-empty string"))),
    }
}

fn boolean_field(request: &Value, name: &str) -> Result<bool> {
    match request.get(name) {
        None | Some(Value::Null) => Ok(false),
        Some(value) => value.as_bool().ok_or_else(|| argument_error(&format!("`{name}` must be a boolean"))),
    }
}

fn options_from(value: Option<&Value>) -> Result<OpenCaeOptions> {
    let Some(value) = value
    else {
        return Ok(OpenCaeOptions::default());
    };
    if !value.is_object() {
        return Err(argument_error("options must be an object"));
    }
    let units = match value.get("units") {
        None | Some(Value::Null) => None,
        Some(Value::String(raw)) => Some(raw.parse::<Units>().map_err(|_| argument_error("`units` must be `mm` or `m`"))?),
        Some(_) => return Err(argument_error("`units` must be a string")),
    };
    let dimension = match value.get("dimension") {
        None | Some(Value::Null) => None,
        Some(raw) => {
            let number = raw.as_u64().ok_or_else(|| argument_error("`dimension` must be an integer"))?;
            Some(u32::try_from(number).map_err(|_| argument_error("`dimension` must be 2 or 3"))?)
        }
    };
    Ok(OpenCaeOptions { units, dimension })
}

fn fail(error: CaeError) -> Error {
    Error::new(format!("{}: {}", error.code, error.message))
}

fn respond<T: Serialize>(ctx: &CallContext, value: &T) -> Result<JsValue> {
    let text = serde_json::to_string(value).map_err(|error| Error::new(format!("document/serialize: {error}")))?;
    ctx.env().string(&text)
}

/// Library version of the native binding, as a JSON string.
#[function]
fn version(ctx: CallContext) -> Result<JsValue> {
    respond(&ctx, &env!("CARGO_PKG_VERSION"))
}

/// Capability matrix of the engine, as a JSON string.
#[function]
fn capabilities(ctx: CallContext) -> Result<JsValue> {
    let matrix = OpenCae::new(OpenCaeOptions::default()).capabilities();
    ctx.env().string(matrix)
}

/// Creates an empty document and returns its summary.
#[function]
fn create(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let options = options_from(optional_object(&ctx, 1)?.as_ref())?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    let summary = session.create(Path::new(&path), options).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Opens a document and returns its summary.
#[function]
fn open(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    let summary = session.open(Path::new(&path)).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Writes a document to `target` (or in place) and returns the new summary.
#[function]
fn save(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let target = optional_text(&ctx, 1)?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.save(target.as_deref().map(Path::new)).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Adds a node to a document and saves it.
#[function]
fn node_add(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let x = required_number_field(&request, "x")?;
    let y = required_number_field(&request, "y")?;
    let z = number_field(&request, "z")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.add_node(&id, x, y, z).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Moves an existing node of a document and saves it.
///
/// The request accepts `id` and any of `x`, `y` or `z`; omitted coordinates
/// keep their current value.
#[function]
fn node_update(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let x = number_field(&request, "x")?;
    let y = number_field(&request, "y")?;
    let z = number_field(&request, "z")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.update_node(&id, x, y, z).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets or clears the concentrated mass of a node and saves the document.
///
/// The request accepts `id` and a non-negative `mass`; `0` clears it.
#[function]
fn node_mass(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let mass = required_number_field(&request, "mass")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_node_mass(&id, mass).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets or clears the fixed temperature of a node and saves the document.
///
/// The request accepts `id` and `temperature`; `null` removes the heat boundary.
#[function]
fn node_temperature(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let temperature = number_field(&request, "temperature")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_node_temperature(&id, temperature).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets the concentrated nodal heat flow and saves the document.
///
/// The request accepts `id` and a finite `heat` in watts; `0` removes the load.
#[function]
fn node_heat(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let heat = required_number_field(&request, "heat")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_node_heat(&id, heat).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Removes a node from a document and saves it.
///
/// A node referenced by an element, a support or a load is rejected with
/// `node/referenced-by-*`.
#[function]
fn node_remove(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.remove_node(&id).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Adds a truss or beam element to a document and saves it.
///
/// The request accepts `kind` (`"truss"` or `"beam"`, default `"truss"`),
/// `inertia` (required for beams), `distributedLoad` (beam transverse load per
/// unit length in local `+y` at the start node), `distributedLoadEnd` (the
/// value at the end node, forming a linearly varying load), `temperature`
/// (uniform temperature change in kelvin), `temperatureGradient` (linear
/// difference between the local `+y` and `-y` extreme fibres), `convection`
/// (surface convection `h`) and `perimeter` (exposed perimeter).
#[function]
fn element_add(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let from = text_field(&request, "from")?;
    let to = text_field(&request, "to")?;
    let youngs_modulus = number_field(&request, "youngsModulus")?;
    let area = number_field(&request, "area")?;
    let kind = element_kind_field(&request)?;
    let inertia = number_field(&request, "inertia")?;
    let distributed_load = number_field(&request, "distributedLoad")?;
    let distributed_load_end = number_field(&request, "distributedLoadEnd")?;
    let temperature = number_field(&request, "temperature")?;
    let temperature_gradient = number_field(&request, "temperatureGradient")?;
    let distributed_load_from = number_field(&request, "distributedLoadFrom")?;
    let distributed_load_to = number_field(&request, "distributedLoadTo")?;
    let convection = number_field(&request, "convection")?;
    let perimeter = number_field(&request, "perimeter")?;
    let tension_only = request.get("tensionOnly").and_then(Value::as_bool).unwrap_or(false);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let mut summary = session
        .add_element_thermal(
            &id,
            &from,
            &to,
            youngs_modulus,
            area,
            kind,
            inertia,
            distributed_load,
            distributed_load_end,
            temperature,
        )
        .map_err(fail)?;
    if temperature_gradient.is_some() {
        session.set_element_temperature_gradient(&id, temperature_gradient).map_err(fail)?;
    }
    if distributed_load_from.is_some() || distributed_load_to.is_some() {
        let span = match (distributed_load_from, distributed_load_to) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => {
                return Err(fail(CaeError::new(
                    "element/invalid-load-span",
                    "`distributedLoadFrom` and `distributedLoadTo` must be supplied together",
                )));
            }
        };
        session.set_element_load_span(&id, span).map_err(fail)?;
    }
    if convection.is_some() {
        summary = session.set_element_convection(&id, convection).map_err(fail)?;
    }
    if perimeter.is_some() {
        summary = session.set_element_perimeter(&id, perimeter).map_err(fail)?;
    }
    if tension_only {
        summary = session.set_element_tension_only(&id, true).map_err(fail)?;
    }
    respond(&ctx, &summary)
}

/// Updates an existing element of a document in place and saves it.
///
/// The request accepts `id` and any of `from`, `to`, `distributedLoad`,
/// `distributedLoadEnd`, `distributedLoadFrom`, `distributedLoadTo`,
/// `temperature`, `temperatureGradient`, `convection`, `perimeter`,
/// `clearLoad` or `clearTemperature`. Omitted fields keep their current value,
/// and the call causes exactly one revision. Conflicting clear flags report
/// `session/argument-invalid`.
#[function]
fn element_update(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let update = ElementUpdate {
        from: request.get("from").and_then(Value::as_str).map(str::to_string),
        to: request.get("to").and_then(Value::as_str).map(str::to_string),
        distributed_load: number_field(&request, "distributedLoad")?,
        distributed_load_end: number_field(&request, "distributedLoadEnd")?,
        distributed_load_from: number_field(&request, "distributedLoadFrom")?,
        distributed_load_to: number_field(&request, "distributedLoadTo")?,
        temperature: number_field(&request, "temperature")?,
        temperature_gradient: number_field(&request, "temperatureGradient")?,
        convection: number_field(&request, "convection")?,
        perimeter: number_field(&request, "perimeter")?,
        clear_load: boolean_field(&request, "clearLoad")?,
        clear_temperature: boolean_field(&request, "clearTemperature")?,
    };
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.update_element_with(&id, &update).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Splits one element at a fraction of its length and saves the document.
///
/// The request accepts `id` and `at` (a fraction strictly between `0` and `1`).
/// The reply adds `node`, `first`, `second` and `reusedNode` to the document
/// summary. A partial-span load, a linearly varying distributed load or a
/// temperature gradient is rejected with `element/split-unsupported-load`.
#[function]
fn element_split(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let at = required_number_field(&request, "at")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let report = session.split_element(&id, at).map_err(fail)?;
    respond(&ctx, &report)
}

/// Sets or clears the bending-moment releases of a beam element.
///
/// The request accepts `id`, `start` and `end`. When both flags are `false` the
/// release is cleared and the member becomes fully continuous again.
#[function]
fn element_release(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let start = boolean_field(&request, "start")?;
    let end = boolean_field(&request, "end")?;
    let release = if start || end { Some(EndRelease { start, end }) } else { None };
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_element_release(&id, release).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets or clears the tension-only (cable) flag of a truss element.
///
/// The request accepts a required `id` and `tensionOnly` boolean.
#[function]
fn element_tension_only(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let tension_only = boolean_field(&request, "tensionOnly")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_element_tension_only(&id, tension_only).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets or clears the linear temperature gradient of a beam element.
///
/// The request accepts a required `id` and an optional `gradient` number;
/// `null` or an omitted field removes the gradient and keeps the uniform
/// temperature.
#[function]
fn element_gradient(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let gradient = number_field(&request, "gradient")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_element_temperature_gradient(&id, gradient).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets or clears the loaded segment of a beam element's distributed load.
///
/// The request accepts a required `id` and the fractions `spanStart` and
/// `spanEnd`; both must be supplied together, and omitting both (or sending
/// `null`) restores a load over the whole member.
#[function]
fn element_span(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let start = number_field(&request, "spanStart")?;
    let end = number_field(&request, "spanEnd")?;
    let span = match (start, end) {
        (Some(start), Some(end)) => Some((start, end)),
        (None, None) => None,
        _ => return Err(argument_error("specify both `spanStart` and `spanEnd`, or neither to restore a full-span load")),
    };
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_element_load_span(&id, span).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Removes an element from a document and saves it.
#[function]
fn element_remove(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.remove_element(&id).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Merges one node into another and saves the document.
#[function]
fn node_merge(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let from = text_field(&request, "from")?;
    let into = text_field(&request, "into")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.merge_nodes(&from, &into).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Replaces the global material of a document and saves it.
///
/// The request accepts `youngsModulus`, `area`, optional `density`, `name`,
/// `allowableStress` and a parametric `section` object. Values that are omitted
/// keep their current document value.
#[function]
fn material_set(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let youngs_modulus = required_number_field(&request, "youngsModulus")?;
    let area = required_number_field(&request, "area")?;
    let density = number_field(&request, "density")?;
    let allowable_stress = number_field(&request, "allowableStress")?;
    let alpha = number_field(&request, "alpha")?;
    let conductivity = number_field(&request, "conductivity")?;
    let section = section_field(&request)?;
    let name = request.get("name").and_then(Value::as_str).map(str::to_string);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    let summary = session.open(Path::new(&path)).map_err(fail)?;
    let material = Material {
        name: name.unwrap_or(summary.material.name.clone()),
        youngs_modulus,
        area,
        density: density.unwrap_or(summary.material.density),
        allowable_stress: allowable_stress.or(summary.material.allowable_stress),
        alpha: alpha.or(summary.material.alpha),
        conductivity: conductivity.or(summary.material.conductivity),
        section: section.or(summary.material.section),
    };
    let summary = session.set_material_data(material).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Applies a built-in material preset to a document and saves it.
///
/// The request accepts `preset` (required), optional `area`, `allowableStress`
/// and a parametric `section`. A supplied section provides the area when
/// `area` is omitted.
#[function]
fn material_preset(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let preset = text_field(&request, "preset")?;
    let area = number_field(&request, "area")?;
    let allowable_stress = number_field(&request, "allowableStress")?;
    let section = section_field(&request)?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.apply_material_preset(&preset, area, section, allowable_stress).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Returns the built-in material and section library as JSON.
///
/// The request accepts an optional `units` value (`mm` or `m`).
#[function]
fn library(ctx: CallContext) -> Result<JsValue> {
    let request = optional_object(&ctx, 0)?;
    let units = match request.as_ref().and_then(|request| request.get("units")).and_then(Value::as_str) {
        Some(raw) => raw.parse::<Units>().map_err(fail)?,
        None => Units::Millimeter,
    };
    let text = library_json(units).map_err(fail)?;
    ctx.env().string(&text)
}

/// Reports derived properties of a parametric section.
///
/// The request accepts a required `section` object and an optional `units`
/// (`mm` or `m`, default `mm`). The reply includes the area, strong/weak
/// inertias, torsion constant, extreme fibre, section modulus and both radii of
/// gyration.
#[function]
fn section_properties(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let section = section_field(&request)?.ok_or_else(|| argument_error("`section` is required"))?;
    let units = match request.get("units").and_then(Value::as_str) {
        Some(raw) => raw.parse::<Units>().map_err(fail)?,
        None => Units::Millimeter,
    };
    let report = section_report(section, units).map_err(fail)?;
    respond(&ctx, &report)
}

fn section_field(request: &Value) -> Result<Option<Section>> {
    match request.get("section") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| argument_error(&format!("`section` is invalid: {error}"))),
    }
}

/// Reports area, centroid, second moments and radii of gyration of a closed
/// polygon section.
///
/// The request accepts a required `points` array of `[x, y]` pairs (one entry
/// per vertex, in either winding order, with or without the closing vertex
/// repeated) and an optional `units` value (`mm` or `m`, default `mm`). The
/// second moments are taken about centroidal axes and the vertex list is
/// normalised to a counter-clockwise winding so `ixy` has one fixed sign.
#[function]
fn section_polygon(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let vertices = polygon_points_field(&request)?;
    let units = match request.get("units").and_then(Value::as_str) {
        Some(raw) => raw.parse::<Units>().map_err(fail)?,
        None => Units::Millimeter,
    };
    let report = polygon_section_report(&vertices, units).map_err(fail)?;
    respond(&ctx, &report)
}

fn request_units(request: &Value) -> Result<Units> {
    match request.get("units").and_then(Value::as_str) {
        Some(raw) => raw.parse::<Units>().map_err(fail),
        None => Ok(Units::Millimeter),
    }
}

/// Reports area, centroid, second moments and torsion constant of an
/// I-section.
///
/// The request accepts `height` (`h`), `width` (`b`), `webThickness` (`tw`),
/// `flangeThickness` (`tf`) and an optional `units` value (`mm` or `m`, default
/// `mm`). Dimensions must satisfy `0 < tf < h/2` and `0 < tw < b`.
#[function]
fn section_i(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = i_standard_section_report(
        required_number_field(&request, "height")?,
        required_number_field(&request, "width")?,
        required_number_field(&request, "webThickness")?,
        required_number_field(&request, "flangeThickness")?,
        units,
    )
    .map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports area, centroid, second moments and torsion constant of a hollow
/// rectangular tube.
///
/// The request accepts `height` (`h`), `width` (`b`), `thickness` (`t`) and an
/// optional `units` value (`mm` or `m`, default `mm`). The wall must satisfy
/// `0 < t < min(b, h)/2`.
#[function]
fn section_tube(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = tube_standard_section_report(
        required_number_field(&request, "height")?,
        required_number_field(&request, "width")?,
        required_number_field(&request, "thickness")?,
        units,
    )
    .map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports area, centroid, second moments and torsion constant of an open
/// channel whose opening faces right.
///
/// The request accepts `height` (`h`), `width` (`b`), `webThickness` (`tw`),
/// `flangeThickness` (`tf`) and an optional `units` value (`mm` or `m`, default
/// `mm`). Dimensions must satisfy `0 < tf < h/2` and `0 < tw < b`.
#[function]
fn section_channel(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = channel_standard_section_report(
        required_number_field(&request, "height")?,
        required_number_field(&request, "width")?,
        required_number_field(&request, "webThickness")?,
        required_number_field(&request, "flangeThickness")?,
        units,
    )
    .map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports area, centroid, second moments and torsion constant of an
/// equal-thickness L (angle) section.
///
/// The request accepts `height` (`h`), `width` (`w`), `thickness` (`t`) and an
/// optional `units` value (`mm` or `m`, default `mm`). Dimensions must satisfy
/// `0 < t < min(h, w)`.
#[function]
fn section_angle(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = angle_section_report(
        required_number_field(&request, "height")?,
        required_number_field(&request, "width")?,
        required_number_field(&request, "thickness")?,
        units,
    )
    .map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports area, centroid, second moments and torsion constant of a T-section
/// whose flange is at the top.
///
/// The request accepts `height` (`h`), `width` (`b`), `webThickness` (`tw`),
/// `flangeThickness` (`tf`) and an optional `units` value (`mm` or `m`, default
/// `mm`). Dimensions must satisfy `0 < tf < h` and `0 < tw < b`.
#[function]
fn section_t(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = t_section_report(
        required_number_field(&request, "height")?,
        required_number_field(&request, "width")?,
        required_number_field(&request, "webThickness")?,
        required_number_field(&request, "flangeThickness")?,
        units,
    )
    .map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports area, centroid, second moments and torsion constant of a solid
/// ellipse.
///
/// The request accepts `width` (`w`), `height` (`h`) and an optional `units`
/// value (`mm` or `m`, default `mm`). The semi-axes are `w/2` and `h/2`, so
/// `A = πwh/4`, `Ix = πwh³/64`, `Iy = πw³h/64` and the centroid is
/// `(w/2, h/2)`. `polarMoment` holds the Saint-Venant torsion constant
/// `πa³b³/(a² + b²)`, not `Ix + Iy`. Both dimensions must be positive and
/// finite.
#[function]
fn section_ellipse(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report =
        ellipse_section_report(required_number_field(&request, "width")?, required_number_field(&request, "height")?, units)
            .map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports area, centroid, second moments and torsion constant of a solid
/// equilateral triangle.
///
/// The request accepts `side` (`a`) and an optional `units` value (`mm` or `m`,
/// default `mm`). The base lies along the bottom edge of the bounding box and
/// the apex is at the top centre, so the centroid is `(a/2, a·√3/6)`.
/// `polarMoment` holds the exact Saint-Venant torsion constant `√3·a⁴/80`, not
/// `Ix + Iy`. The side must be positive and finite.
#[function]
fn section_triangle(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = triangle_section_report(required_number_field(&request, "side")?, units).map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports area, centroid, second moments and torsion constant of an
/// equal-thickness Z-section.
///
/// The request accepts `height` (`h`), `width` (`w`), `thickness` (`t`) and an
/// optional `units` value (`mm` or `m`, default `mm`). The origin is the
/// lower-left corner of the bounding rectangle, the web is centred and the top
/// and bottom flanges extend to opposite edges. Dimensions must satisfy
/// `0 < t < w` and `2t < h`.
#[function]
fn section_z(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = z_section_report(
        required_number_field(&request, "height")?,
        required_number_field(&request, "width")?,
        required_number_field(&request, "thickness")?,
        units,
    )
    .map_err(fail)?;
    respond(&ctx, &report)
}

/// Reports principal second moments and the major-axis angle of a section.
///
/// The request accepts `ix`, `iy`, `ixy` and an optional `units` value (`mm` or
/// `m`, default `mm`). The moments must be positive and define a positive
/// semidefinite matrix. `i1` is the major principal moment, `i2` the minor one,
/// `polarMoment` is `Ix + Iy` and `principalAngle` is the counter-clockwise
/// angle of the major axis from `+x` in degrees within `(-90, 90]`.
#[function]
fn section_principal(ctx: CallContext) -> Result<JsValue> {
    let request = object(&ctx, 0)?;
    let units = request_units(&request)?;
    let report = principal_section_report(
        required_number_field(&request, "ix")?,
        required_number_field(&request, "iy")?,
        required_number_field(&request, "ixy")?,
        units,
    )
    .map_err(fail)?;
    respond(&ctx, &report)
}

fn polygon_points_field(request: &Value) -> Result<Vec<(f64, f64)>> {
    let values = match request.get("points").and_then(Value::as_array) {
        Some(values) => values,
        None => return Err(argument_error("`points` must be an array of `[x, y]` pairs")),
    };
    values
        .iter()
        .map(|value| match value.as_array() {
            Some(pair) if pair.len() == 2 => match (pair[0].as_f64(), pair[1].as_f64()) {
                (Some(x), Some(y)) => Ok((x, y)),
                _ => Err(argument_error("each `points` entry must contain two numbers")),
            },
            _ => Err(argument_error("each `points` entry must be an `[x, y]` array")),
        })
        .collect()
}

/// Adds or merges a support entry of a document and saves it.
///
/// `ux`, `uy`, `uz` and `rotation` enforce non-zero displacement values and
/// imply the matching constraint; when they are present the entry is written
/// through the prescribed-displacement path. `kx`, `ky`, `kz` and `kr` add
/// linear elastic spring stiffness without constraining the matching degree of
/// freedom.
#[function]
fn support_add(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let node = text_field(&request, "node")?;
    let dx = boolean_field(&request, "dx")?;
    let dy = boolean_field(&request, "dy")?;
    let dz = boolean_field(&request, "dz")?;
    let rz = boolean_field(&request, "rz")?;
    let ux = number_field(&request, "ux")?;
    let uy = number_field(&request, "uy")?;
    let uz = number_field(&request, "uz")?;
    let rotation = number_field(&request, "rotation")?;
    let kx = number_field(&request, "kx")?;
    let ky = number_field(&request, "ky")?;
    let kz = number_field(&request, "kz")?;
    let kr = number_field(&request, "kr")?;
    let direction = match request.get("direction") {
        None | Some(Value::Null) => None,
        Some(Value::Array(values)) if values.len() == 2 => match (values[0].as_f64(), values[1].as_f64()) {
            (Some(nx), Some(ny)) => Some((nx, ny)),
            _ => return Err(argument_error("`direction` must be an array of two numbers `[nx, ny]`")),
        },
        Some(_) => return Err(argument_error("`direction` must be an array of two numbers `[nx, ny]`")),
    };
    let prescribed = ux.is_some() || uy.is_some() || uz.is_some() || rotation.is_some();
    let spring = kx.is_some() || ky.is_some() || kz.is_some() || kr.is_some();
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    if let Some((nx, ny)) = direction {
        if dx || dy || dz || rz || prescribed || spring {
            return Err(argument_error(
                "`direction` is an independent inclined-support constraint and cannot be combined with `dx`, `dy`, `dz`, `rz`, a prescribed displacement or a spring",
            ));
        }
        let summary = session.add_support_direction(&node, nx, ny).map_err(fail)?;
        return respond(&ctx, &summary);
    }
    if dx || dy || dz || rz {
        session.add_support_full(&node, dx, dy, dz, rz).map_err(fail)?;
    }
    if prescribed {
        session.prescribe_displacement(&node, ux, uy, uz, rotation).map_err(fail)?;
    }
    let summary =
        if spring { session.set_spring(&node, kx, ky, kz, kr).map_err(fail)? } else { session.summary().map_err(fail)? };
    respond(&ctx, &summary)
}

/// Removes the support entry of a node and saves the document.
#[function]
fn support_remove(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let node = text_field(&request, "node")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.remove_support(&node).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Adds or updates a concentrated load of a document and saves it.
#[function]
fn load_add(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let node = text_field(&request, "node")?;
    let fx = number_field(&request, "fx")?;
    let fy = number_field(&request, "fy")?;
    let fz = number_field(&request, "fz")?;
    let mz = number_field(&request, "mz")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.add_load_full(&node, fx, fy, fz, mz).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Removes the concentrated load entry of a node and saves the document.
#[function]
fn load_remove(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let node = text_field(&request, "node")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.remove_load(&node).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets the gravity acceleration vector of a document and saves it.
///
/// The zero vector clears gravity.
#[function]
fn gravity_set(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let gx = number_field(&request, "gx")?.unwrap_or(0.0);
    let gy = number_field(&request, "gy")?.unwrap_or(0.0);
    let gz = number_field(&request, "gz")?.unwrap_or(0.0);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_gravity(gx, gy, gz).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Sets or clears the ambient temperature used by surface convection and saves
/// the document.
///
/// The request accepts an optional `temperature`; a missing value or `null`
/// clears it.
#[function]
fn ambient_temperature_set(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let temperature = number_field(&request, "temperature")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.set_ambient_temperature(temperature).map_err(fail)?;
    respond(&ctx, &summary)
}

fn element_kind_field(request: &Value) -> Result<ElementKind> {
    match request.get("kind") {
        None | Some(Value::Null) => Ok(ElementKind::Truss),
        Some(Value::String(raw)) => raw.parse::<ElementKind>().map_err(fail),
        Some(_) => Err(argument_error("`kind` must be `truss` or `beam`")),
    }
}

/// Adds or replaces a linear load combination and saves the document.
///
/// The request accepts `name`, `loads` (default 1) and `gravity` (default 0).
#[function]
fn combination_add(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let name = text_field(&request, "name")?;
    let loads = number_field(&request, "loads")?.unwrap_or(1.0);
    let gravity = number_field(&request, "gravity")?.unwrap_or(0.0);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session.add_combination(&name, loads, gravity).map_err(fail)?;
    respond(&ctx, &summary)
}

/// Removes a load combination by name and saves the document.
#[function]
fn combination_remove(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let name = text_field(&request, "name")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let removed = session.remove_combination(&name).map_err(fail)?;
    if !removed {
        return Err(Error::new(format!("combination/unknown-name: no load combination named `{name}` exists")));
    }
    let summary = session.summary().map_err(fail)?;
    respond(&ctx, &summary)
}

/// Solves every load combination and returns the solutions plus their
/// envelope. The request accepts an optional `out` path.
#[function]
fn solve_combinations(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.solve_combinations(out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Validates a document and returns `{ ok, issues }`.
#[function]
fn validate(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let report = session.validate().map_err(fail)?;
    respond(&ctx, &report)
}

/// Solves a document, optionally writing the JSON report to `out`.
///
/// `analysis` is `"static"` (default), `"modal"`, `"buckling"` or
/// `"tensionOnly"`; modal and buckling requests honour `modes` (default 5) and
/// tension-only requests honour `iterations` (default 20) and `tolerance`
/// (default `1e-9`).
#[function]
fn solve(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let analysis = request.as_ref().and_then(|request| request.get("analysis")).and_then(Value::as_str);
    let modes = request.as_ref().and_then(|request| request.get("modes")).and_then(Value::as_u64);
    let iterations = request.as_ref().and_then(|request| request.get("iterations")).and_then(Value::as_u64);
    let tolerance = request.as_ref().and_then(|request| request.get("tolerance")).and_then(Value::as_f64);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    match analysis.unwrap_or("static") {
        "static" => {
            let solution = session.solve(out.map(Path::new)).map_err(fail)?;
            respond(&ctx, &solution)
        }
        "modal" => {
            let modes =
                usize::try_from(modes.unwrap_or(5)).map_err(|_| argument_error("`modes` must be a positive integer"))?;
            let modal = session.solve_modal(modes, out.map(Path::new)).map_err(fail)?;
            respond(&ctx, &modal)
        }
        "buckling" => {
            let modes =
                usize::try_from(modes.unwrap_or(5)).map_err(|_| argument_error("`modes` must be a positive integer"))?;
            let buckling = session.solve_buckling(modes, out.map(Path::new)).map_err(fail)?;
            respond(&ctx, &buckling)
        }
        "tensionOnly" | "tension-only" => {
            let iterations = usize::try_from(iterations.unwrap_or(20))
                .map_err(|_| argument_error("`iterations` must be a positive integer"))?;
            let result = session.tension_only(iterations, tolerance.unwrap_or(1e-9), out.map(Path::new)).map_err(fail)?;
            respond(&ctx, &result)
        }
        other => {
            Err(argument_error(&format!("unsupported analysis `{other}`; use `static`, `modal`, `buckling` or `tensionOnly`")))
        }
    }
}

/// Solves steady-state heat conduction and returns the temperature report.
///
/// The request accepts an optional `out` path for the JSON report.
#[function]
fn heat(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.solve_heat(out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Solves the linear buckling eigenproblem for every stored load combination.
///
/// The request accepts `modes` (default 5) and an optional `out` path for the
/// JSON report.
#[function]
fn solve_buckling_combinations(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let modes = request.as_ref().and_then(|request| request.get("modes")).and_then(Value::as_u64);
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let modes = usize::try_from(modes.unwrap_or(5)).map_err(|_| argument_error("`modes` must be a positive integer"))?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.solve_buckling_combinations(modes, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Samples internal axial force, shear and bending moment and returns the
/// diagram report.
///
/// The request accepts `stations` (default 11, at least 2) and an optional
/// `out` path for the JSON report.
#[function]
fn internal_forces(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let stations = request.as_ref().and_then(|request| request.get("stations")).and_then(Value::as_u64);
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let stations =
        usize::try_from(stations.unwrap_or(11)).map_err(|_| argument_error("`stations` must be a positive integer"))?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let forces = session.internal_forces(stations, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &forces)
}

/// Returns support reactions and the global force equilibrium check.
///
/// The request accepts an optional `out` path for the JSON report.
#[function]
fn reactions(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.reactions(out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Returns the solved displacement of one node.
///
/// The request accepts a required `id`.
#[function]
fn node_displacement(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.node_displacement(&id).map_err(fail)?;
    respond(&ctx, &result)
}

/// Returns the solved member result of one element.
///
/// The request accepts a required `id` and the reply includes the local end
/// forces.
#[function]
fn element_result(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let id = text_field(&request, "id")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.element_result(&id).map_err(fail)?;
    respond(&ctx, &result)
}

/// Returns the support reaction of one node.
///
/// The request accepts a required `node`.
#[function]
fn node_reaction(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let node = text_field(&request, "node")?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.node_reaction(&node).map_err(fail)?;
    respond(&ctx, &result)
}

/// Recovers extreme-fibre beam stresses from the sampled internal forces.
///
/// The request accepts `stations` (default 11, at least 2) and an optional
/// `out` path for the JSON report.
#[function]
fn stresses(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let stations = request.as_ref().and_then(|request| request.get("stations")).and_then(Value::as_u64);
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let stations =
        usize::try_from(stations.unwrap_or(11)).map_err(|_| argument_error("`stations` must be a positive integer"))?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.stresses(stations, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Samples the deflected centreline of every element.
///
/// The request accepts `stations` (default 11, at least 2) and an optional
/// `out` path for the JSON report.
#[function]
fn deflections(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let stations = request.as_ref().and_then(|request| request.get("stations")).and_then(Value::as_u64);
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let stations =
        usize::try_from(stations.unwrap_or(11)).map_err(|_| argument_error("`stations` must be a positive integer"))?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.deflections(stations, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Averages element-end stresses at shared nodes and reports the extremes.
///
/// The request accepts `stations` (default 11, at least 2) and an optional
/// `out` path for the JSON report.
#[function]
fn nodal_stresses(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let stations = request.as_ref().and_then(|request| request.get("stations")).and_then(Value::as_u64);
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let stations =
        usize::try_from(stations.unwrap_or(11)).map_err(|_| argument_error("`stations` must be a positive integer"))?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.nodal_stresses(stations, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Summarises the unfactored static case and every stored load combination.
///
/// The request accepts an optional `out` path for the JSON report.
#[function]
fn load_cases(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.load_cases(out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Solves a response-spectrum analysis by modal superposition.
///
/// The request accepts `spectrum` (array of `{ period, acceleration }` points),
/// optional `modes` (default 5), `axis` (`"x"`, `"y"` or `"z"`, default `"x"`),
/// `combination` (`"srss"` or `"cqc"`, default `"srss"`), `damping` (required
/// and in `(0, 1]` for CQC) and an optional `out` path.
#[function]
fn response_spectrum(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let spectrum: Vec<SpectrumPoint> = match request.get("spectrum") {
        Some(value) => {
            serde_json::from_value(value.clone()).map_err(|error| argument_error(&format!("`spectrum` is invalid: {error}")))?
        }
        None => return Err(argument_error("`spectrum` is required")),
    };
    let modes = usize::try_from(request.get("modes").and_then(Value::as_u64).unwrap_or(5))
        .map_err(|_| argument_error("`modes` must be a positive integer"))?;
    let axis = match request.get("axis").and_then(Value::as_str).unwrap_or("x") {
        "x" => 0,
        "y" => 1,
        "z" => 2,
        other => return Err(argument_error(&format!("`axis` must be `x`, `y` or `z`, not `{other}`"))),
    };
    let combination =
        SpectrumCombination::parse(request.get("combination").and_then(Value::as_str).unwrap_or("srss")).map_err(fail)?;
    let damping = number_field(&request, "damping")?;
    let out = request.get("out").and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.response_spectrum(modes, axis, &spectrum, combination, damping, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Exports the assembled global stiffness and lumped mass matrices and returns
/// the report.
///
/// The request accepts an optional `out` path for the JSON report.
#[function]
fn matrices(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.matrices(out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Solves the steady-state harmonic response over a frequency sweep.
///
/// The request accepts required `damping`, `startHz` and `endHz`, optional
/// `modes` (default 5), `axis` (`"x"`, `"y"` or `"z"`, default `"x"`),
/// `points` (default 101) and an optional `out` path. The static load vector of
/// the document is treated as the harmonic excitation.
#[function]
fn frequency_response(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let modes = usize::try_from(request.get("modes").and_then(Value::as_u64).unwrap_or(5))
        .map_err(|_| argument_error("`modes` must be a positive integer"))?;
    let axis = match request.get("axis").and_then(Value::as_str).unwrap_or("x") {
        "x" => 0,
        "y" => 1,
        "z" => 2,
        other => return Err(argument_error(&format!("`axis` must be `x`, `y` or `z`, not `{other}`"))),
    };
    let damping = required_number_field(&request, "damping")?;
    let start = required_number_field(&request, "startHz")?;
    let end = required_number_field(&request, "endHz")?;
    let points = usize::try_from(request.get("points").and_then(Value::as_u64).unwrap_or(101))
        .map_err(|_| argument_error("`points` must be a positive integer"))?;
    let out = request.get("out").and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.frequency_response(modes, axis, damping, start, end, points, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Solves construction stages by activating and deactivating elements.
///
/// The request accepts a required `stages` array of
/// `{ name, elements?, loads?, gravity? }` objects and an optional `out` path.
#[function]
fn construction_stages(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let stages: Vec<ConstructionStage> = match request.get("stages") {
        Some(value) => {
            serde_json::from_value(value.clone()).map_err(|error| argument_error(&format!("`stages` is invalid: {error}")))?
        }
        None => return Err(argument_error("`stages` is required")),
    };
    let out = request.get("out").and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.construction_stages(&stages, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Builds an influence line as a unit load moves along a track beam element.
///
/// The request requires `track` and `quantity` and accepts optional
/// `direction` (`x` or `y`, default `y`), `points` (default 11) and `out`.
#[function]
fn influence_line(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let track = request.get("track").and_then(Value::as_str).ok_or_else(|| argument_error("`track` is required"))?;
    let quantity: InfluenceQuantity = match request.get("quantity") {
        Some(value) => {
            serde_json::from_value(value.clone()).map_err(|error| argument_error(&format!("`quantity` is invalid: {error}")))?
        }
        None => return Err(argument_error("`quantity` is required")),
    };
    let direction = match request.get("direction").and_then(Value::as_str).unwrap_or("y") {
        "x" => InfluenceDirection::X,
        "y" => InfluenceDirection::Y,
        other => return Err(argument_error(&format!("`direction` must be `x` or `y`, not `{other}`"))),
    };
    let points = usize::try_from(request.get("points").and_then(Value::as_u64).unwrap_or(11))
        .map_err(|_| argument_error("`points` must be a positive integer"))?;
    let out = request.get("out").and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.influence_line(track, &quantity, direction, points, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Builds the moving-load-train envelope of an influence line.
///
/// The request requires `track`, `quantity`, `axles` and `spacings` and
/// accepts optional `direction` (`x` or `y`, default `y`), `points` (default
/// 11), `step` (default 0.01, a fraction of the track length) and `out`.
#[function]
fn influence_line_train(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = object(&ctx, 1)?;
    let out = request.get("out").and_then(Value::as_str).map(str::to_string);
    let train: InfluenceTrainRequest =
        serde_json::from_value(request).map_err(|error| argument_error(&format!("`request` is invalid: {error}")))?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.influence_line_train(&train, out.as_deref().map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Solves a geometric-nonlinear truss model with a large-displacement Newton
/// iteration.
///
/// The request accepts optional `increments` (default 10), `iterations`
/// (default 30), `tolerance` (default `1e-10`) and an optional `out` path.
#[function]
fn geometric_nonlinear(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let number = |name: &str, default: f64| -> Result<f64> {
        match request.as_ref().and_then(|request| request.get(name)) {
            None | Some(Value::Null) => Ok(default),
            Some(value) => value.as_f64().ok_or_else(|| argument_error(&format!("`{name}` must be a number"))),
        }
    };
    let increments = usize::try_from(number("increments", 10.0)? as u64)
        .map_err(|_| argument_error("`increments` must be a positive integer"))?;
    let iterations = usize::try_from(number("iterations", 30.0)? as u64)
        .map_err(|_| argument_error("`iterations` must be a positive integer"))?;
    let tolerance = number("tolerance", 1e-10)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.nonlinear(increments, iterations, tolerance, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Solves a model with tension-only (cable) truss members.
///
/// The request accepts optional `iterations` (default 20), `tolerance`
/// (default `1e-9`) and an optional `out` path for the JSON report.
#[function]
fn tension_only(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let number = |name: &str, default: f64| -> Result<f64> {
        match request.as_ref().and_then(|request| request.get(name)) {
            None | Some(Value::Null) => Ok(default),
            Some(value) => value.as_f64().ok_or_else(|| argument_error(&format!("`{name}` must be a number"))),
        }
    };
    let iterations = usize::try_from(number("iterations", 20.0)? as u64)
        .map_err(|_| argument_error("`iterations` must be a positive integer"))?;
    let tolerance = number("tolerance", 1e-9)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let result = session.tension_only(iterations, tolerance, out.map(Path::new)).map_err(fail)?;
    respond(&ctx, &result)
}

/// Exports a solved document as `csv`, `vtk`, `md`, `html` or `svg`.
///
/// The optional fourth argument is a JSON object with a `scale` number that
/// multiplies the nodal displacements of the SVG deformed shape (default `1`).
#[function]
fn export(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let format = text(&ctx, 1)?.parse::<ExportFormat>().map_err(fail)?;
    let target = text(&ctx, 2)?;
    let options = optional_object(&ctx, 3)?;
    let scale = match options.as_ref() {
        Some(options) => number_field(options, "scale")?.unwrap_or(1.0),
        None => 1.0,
    };
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let report = session.export_with_options(format, Path::new(&target), ExportOptions { scale }).map_err(fail)?;
    respond(&ctx, &report)
}

/// Solves a document and returns the full report.
#[function]
fn report(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let report = session.report().map_err(fail)?;
    respond(&ctx, &report)
}

register_module![
    version,
    capabilities,
    create,
    open,
    save,
    node_add,
    node_update,
    node_mass,
    node_temperature,
    node_heat,
    node_merge,
    node_remove,
    element_add,
    element_update,
    element_split,
    element_remove,
    element_release,
    element_tension_only,
    element_gradient,
    element_span,
    material_set,
    material_preset,
    library,
    section_properties,
    section_polygon,
    section_i,
    section_tube,
    section_channel,
    section_angle,
    section_t,
    section_ellipse,
    section_triangle,
    section_z,
    section_principal,
    support_add,
    support_remove,
    load_add,
    load_remove,
    gravity_set,
    ambient_temperature_set,
    combination_add,
    combination_remove,
    solve_combinations,
    solve_buckling_combinations,
    validate,
    solve,
    heat,
    reactions,
    node_displacement,
    element_result,
    node_reaction,
    internal_forces,
    influence_line,
    influence_line_train,
    stresses,
    deflections,
    nodal_stresses,
    load_cases,
    matrices,
    response_spectrum,
    frequency_response,
    construction_stages,
    geometric_nonlinear,
    tension_only,
    export,
    report
];
