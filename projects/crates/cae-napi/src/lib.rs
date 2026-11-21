//! Thin Node-API surface for OpenCAE.
//!
//! Every export performs one orthogonal operation and returns a JSON string, so
//! the TypeScript layer owns argument parsing, help text and exit codes. The
//! document editing, validation, solving and export calls forward to
//! `cae-model`'s [`cae_model::OpenCae`] session, which the `ocae` binary also
//! calls, so the two entry points cannot disagree.
//!
//! Unimplemented routes keep the stable `capability/not-implemented` code. Bad
//! arguments report `session/argument-invalid`; every other failure keeps the
//! stable code of [`cae_model::CaeError`].

#![deny(missing_docs)]

use std::path::Path;

use cae_model::{
    CaeError, ElementKind, EndRelease, ExportFormat, Material, OpenCae, OpenCaeOptions, Section, Units, library_json,
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

/// Adds a truss or beam element to a document and saves it.
///
/// The request accepts `kind` (`"truss"` or `"beam"`, default `"truss"`),
/// `inertia` (required for beams), `distributedLoad` (beam transverse load per
/// unit length in local `+y` at the start node), `distributedLoadEnd` (the
/// value at the end node, forming a linearly varying load), `temperature`
/// (uniform temperature change in kelvin) and `temperatureGradient` (linear
/// difference between the local `+y` and `-y` extreme fibres).
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
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let summary = session
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
    respond(&ctx, &summary)
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

fn section_field(request: &Value) -> Result<Option<Section>> {
    match request.get("section") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|error| argument_error(&format!("`section` is invalid: {error}"))),
    }
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
    let prescribed = ux.is_some() || uy.is_some() || uz.is_some() || rotation.is_some();
    let spring = kx.is_some() || ky.is_some() || kz.is_some() || kr.is_some();
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
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
/// `analysis` is `"static"` (default), `"modal"` or `"buckling"`; modal and
/// buckling requests honour `modes` (default 5).
#[function]
fn solve(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let request = optional_object(&ctx, 1)?;
    let out = request.as_ref().and_then(|request| request.get("out")).and_then(Value::as_str);
    let analysis = request.as_ref().and_then(|request| request.get("analysis")).and_then(Value::as_str);
    let modes = request.as_ref().and_then(|request| request.get("modes")).and_then(Value::as_u64);
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
        other => Err(argument_error(&format!("unsupported analysis `{other}`; use `static`, `modal` or `buckling`"))),
    }
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

/// Exports a solved document as `csv`, `vtk` or `md`.
#[function]
fn export(ctx: CallContext) -> Result<JsValue> {
    let path = text(&ctx, 0)?;
    let format = text(&ctx, 1)?.parse::<ExportFormat>().map_err(fail)?;
    let target = text(&ctx, 2)?;
    let mut session = OpenCae::new(OpenCaeOptions::default());
    session.open(Path::new(&path)).map_err(fail)?;
    let report = session.export(format, Path::new(&target)).map_err(fail)?;
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
    element_add,
    element_release,
    material_set,
    material_preset,
    library,
    support_add,
    load_add,
    gravity_set,
    combination_add,
    combination_remove,
    solve_combinations,
    solve_buckling_combinations,
    validate,
    solve,
    reactions,
    internal_forces,
    stresses,
    deflections,
    nodal_stresses,
    load_cases,
    export,
    report
];
