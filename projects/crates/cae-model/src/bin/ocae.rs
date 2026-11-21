//! Thin command line over the [`cae_model::OpenCae`] session API.
//!
//! This binary only parses arguments, calls one session operation, prints the
//! result and maps errors to exit codes. Editing, validation, solving and export
//! live in the library so the native addon and the TypeScript SDK share them.

use std::{
    collections::{HashMap, HashSet},
    env,
    path::{Path, PathBuf},
    process,
};

use cae_model::{CaeError, ElementKind, ExportFormat, OpenCae, OpenCaeOptions, Units, not_implemented};

const USAGE: &str = "\
ocae - OpenCAE truss/beam linear-static and modal solver (2D/3D)

Usage:
  ocae create <file.ocae> [--units mm|m] [--dimension 2|3]
  ocae node add <file> --id <id> --x <x> --y <y> [--z <z>]
  ocae element add <file> --id <id> --from <node> --to <node> [--E <E>] [--A <A>]
                    [--kind truss|beam] [--I <I>] [--w <w>]
  ocae material set <file> --E <E> --A <A> [--density <rho>] [--name <name>]
  ocae support add <file> --node <node> [--dx] [--dy] [--dz] [--rz]
  ocae load add <file> --node <node> [--fx <fx>] [--fy <fy>] [--fz <fz>] [--mz <mz>]
  ocae model gravity <file> [--gx <gx>] [--gy <gy>] [--gz <gz>]
  ocae self-weight <file> --g <g> [--axis x|y|z]
  ocae validate <file>
  ocae solve <file> [--out report.json] [--analysis static|modal|buckling] [--modes N]
                   [--element truss|beam] [--dimension 2|3]
  ocae modal <file> [--modes N] [--out report.json]
  ocae buckling <file> [--modes N] [--out report.json]
  ocae export <file> --format csv|vtk|md --out <dir|file>
  ocae report <file>
  ocae capabilities

Exit codes: 0 success, 1 input/validation error, 2 capability not implemented
";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let exit_code = match dispatch(&args) {
        Ok(code) => code,
        Err(error) => {
            let payload = serde_json::to_string_pretty(&error).unwrap_or_else(|_| format!("{{\"code\":\"{}\"}}", error.code));
            eprintln!("{payload}");
            error.exit_code()
        }
    };
    process::exit(exit_code);
}

fn dispatch(args: &[String]) -> Result<i32, CaeError> {
    let Some(command) = args.first().map(String::as_str)
    else {
        print!("{USAGE}");
        return Ok(0);
    };
    match command {
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(0)
        }
        "capabilities" => {
            println!("{}", OpenCae::new(OpenCaeOptions::default()).capabilities());
            Ok(0)
        }
        "create" => create(&args[1..]),
        "node" => subcommand(args, "add", "node"),
        "element" => subcommand(args, "add", "element"),
        "material" => subcommand(args, "set", "material"),
        "support" => subcommand(args, "add", "support"),
        "load" => subcommand(args, "add", "load"),
        "model" => model_command(&args[1..]),
        "self-weight" => self_weight(&args[1..]),
        "validate" => validate(&args[1..]),
        "solve" => solve_command(&args[1..]),
        "modal" => modal_command(&args[1..]),
        "buckling" => buckling_command(&args[1..]),
        "export" => export(&args[1..]),
        "report" => report(&args[1..]),
        "import" => Err(not_implemented("Abaqus .inp import")),
        "nonlinear" => Err(not_implemented("nonlinear analysis")),
        "beam" | "beam2d" => {
            Err(CaeError::new("cli/unknown-command", "`beam` is not a command; use `--kind beam --I <I>` with `element add`"))
        }
        "frame" | "frame3d" => Err(not_implemented("3D frame elements")),
        "thermal" => Err(not_implemented("thermal analysis")),
        "contact" => Err(not_implemented("contact analysis")),
        other => Err(CaeError::new("cli/unknown-command", format!("unknown command `{other}`; run --help for usage"))),
    }
}

fn session() -> OpenCae {
    OpenCae::new(OpenCaeOptions::default())
}

fn subcommand(args: &[String], expected: &str, group: &str) -> Result<i32, CaeError> {
    match args.get(1).map(String::as_str) {
        Some(action) if action == expected => match (group, action) {
            ("node", "add") => node_add(&args[2..]),
            ("element", "add") => element_add(&args[2..]),
            ("material", "set") => material_set(&args[2..]),
            ("support", "add") => support_add(&args[2..]),
            ("load", "add") => load_add(&args[2..]),
            _ => Err(CaeError::new("cli/unknown-command", format!("unknown command `{group} {action}`"))),
        },
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!("subcommand `{other}` of `{group}` does not exist; only `{expected}` is supported"),
        )),
        None => Err(CaeError::new(
            "cli/missing-argument",
            format!("`{group}` is missing a subcommand; only `{expected}` is supported"),
        )),
    }
}

fn create(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units", "--dimension"], &[])?;
    let file = positional(&parsed, 0, "create <file.ocae> [--units mm|m] [--dimension 2|3]")?;
    let path = PathBuf::from(file);
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let dimension = match parsed.options.get("--dimension") {
        Some(value) => parse_dimension(value)?,
        None => 2,
    };
    let mut session = session();
    let summary = session.create(&path, OpenCaeOptions { units: Some(units), dimension: Some(dimension) })?;
    println!("created {} ({}, dimension {})", summary.path, units.as_str(), summary.dimension);
    Ok(0)
}

fn node_add(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--x", "--y", "--z"], &[])?;
    let file = positional(&parsed, 0, "node add <file> --id <id> --x <x> --y <y> [--z <z>]")?;
    let id = required(&parsed, "--id")?;
    let x = required_number(&parsed, "--x")?;
    let y = required_number(&parsed, "--y")?;
    let z = number(&parsed, "--z")?.unwrap_or(0.0);
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.add_node(id, x, y, Some(z))?;
    println!("node `{id}` added (z={z}, revision {})", summary.revision);
    Ok(0)
}

fn element_add(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--from", "--to", "--E", "--A", "--kind", "--I", "--w"], &[])?;
    let file = positional(
        &parsed,
        0,
        "element add <file> --id <id> --from <node> --to <node> [--kind truss|beam] [--I <I>] [--w <w>]",
    )?;
    let id = required(&parsed, "--id")?;
    let from = required(&parsed, "--from")?;
    let to = required(&parsed, "--to")?;
    let youngs_modulus = number(&parsed, "--E")?;
    let area = number(&parsed, "--A")?;
    let kind = match parsed.options.get("--kind") {
        Some(value) => value.parse::<ElementKind>()?,
        None => ElementKind::Truss,
    };
    let inertia = number(&parsed, "--I")?;
    let distributed_load = number(&parsed, "--w")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.add_element_full(id, from, to, youngs_modulus, area, kind, inertia, distributed_load)?;
    println!("element `{id}` added as {} (revision {})", kind.as_str(), summary.revision);
    Ok(0)
}

fn material_set(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--E", "--A", "--density", "--name"], &[])?;
    let file = positional(&parsed, 0, "material set <file> --E <E> --A <A> [--density <rho>] [--name <name>]")?;
    let youngs_modulus = required_number(&parsed, "--E")?;
    let area = required_number(&parsed, "--A")?;
    let density = number(&parsed, "--density")?;
    let name = parsed.options.get("--name").cloned();
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = match density {
        Some(density) => session.set_material_full(youngs_modulus, area, density, name)?,
        None => session.set_material(youngs_modulus, area, name)?,
    };
    println!(
        "material set: E={} A={} density={} (revision {})",
        summary.material.youngs_modulus, summary.material.area, summary.material.density, summary.revision
    );
    Ok(0)
}

fn support_add(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--node"], &["--dx", "--dy", "--dz", "--rz"])?;
    let file = positional(&parsed, 0, "support add <file> --node <node> [--dx] [--dy] [--dz] [--rz]")?;
    let node = required(&parsed, "--node")?;
    let dx = parsed.flags.contains("--dx");
    let dy = parsed.flags.contains("--dy");
    let dz = parsed.flags.contains("--dz");
    let rz = parsed.flags.contains("--rz");
    if !dx && !dy && !dz && !rz {
        return Err(CaeError::new(
            "cli/missing-option",
            "specify at least one of `--dx`, `--dy`, `--dz` or `--rz`; otherwise the support constrains no degree of freedom",
        ));
    }
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.add_support_full(node, dx, dy, dz, rz)?;
    println!("support `{node}` added (dx={dx}, dy={dy}, dz={dz}, rz={rz}, revision {})", summary.revision);
    Ok(0)
}

fn load_add(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--node", "--fx", "--fy", "--fz", "--mz"], &[])?;
    let file = positional(&parsed, 0, "load add <file> --node <node> [--fx <fx>] [--fy <fy>] [--fz <fz>] [--mz <mz>]")?;
    let node = required(&parsed, "--node")?;
    let fx = number(&parsed, "--fx")?;
    let fy = number(&parsed, "--fy")?;
    let fz = number(&parsed, "--fz")?;
    let mz = number(&parsed, "--mz")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.add_load_full(node, fx, fy, fz, mz)?;
    println!("load `{node}` added (revision {})", summary.revision);
    Ok(0)
}

fn model_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("gravity") => gravity(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!("subcommand `{other}` of `model` does not exist; only `gravity` is supported"),
        )),
        None => Err(CaeError::new("cli/missing-argument", "`model` is missing a subcommand; only `gravity` is supported")),
    }
}

fn gravity(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--gx", "--gy", "--gz"], &[])?;
    let file = positional(&parsed, 0, "model gravity <file> [--gx <gx>] [--gy <gy>] [--gz <gz>]")?;
    let gx = number(&parsed, "--gx")?.unwrap_or(0.0);
    let gy = number(&parsed, "--gy")?.unwrap_or(0.0);
    let gz = number(&parsed, "--gz")?.unwrap_or(0.0);
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_gravity(gx, gy, gz)?;
    println!("gravity set to ({gx}, {gy}, {gz}) (revision {})", summary.revision);
    Ok(0)
}

fn self_weight(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--g", "--axis"], &[])?;
    let file = positional(&parsed, 0, "self-weight <file> --g <g> [--axis x|y|z]")?;
    let value = required_number(&parsed, "--g")?;
    let axis = parsed.options.get("--axis").map(String::as_str).unwrap_or("y");
    let gravity = match axis {
        "x" => (value, 0.0, 0.0),
        "y" => (0.0, value, 0.0),
        "z" => (0.0, 0.0, value),
        other => {
            return Err(CaeError::new("cli/invalid-option", format!("unknown axis `{other}`; only x, y or z is supported")));
        }
    };
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_gravity(gravity.0, gravity.1, gravity.2)?;
    println!("self-weight gravity set to ({}, {}, {}) (revision {})", gravity.0, gravity.1, gravity.2, summary.revision);
    Ok(0)
}

fn validate(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &[], &[])?;
    let file = positional(&parsed, 0, "validate <file>")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let report = session.validate()?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(if report.ok { 0 } else { 1 })
}

fn solve_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--out", "--analysis", "--element", "--dimension", "--modes"], &[])?;
    let file = positional(
        &parsed,
        0,
        "solve <file> [--out report.json] [--analysis static|modal|buckling] [--modes N] [--element truss|beam] [--dimension 2|3]",
    )?;
    let mut session = session();
    let summary = session.open(Path::new(file))?;
    let analysis = validate_analysis_options(&parsed, summary.dimension)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    match analysis.as_str() {
        "modal" => {
            let modes = parse_modes(&parsed)?;
            let modal = session.solve_modal(modes, out.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&modal).unwrap_or_default());
        }
        "buckling" => {
            let modes = parse_modes(&parsed)?;
            let buckling = session.solve_buckling(modes, out.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&buckling).unwrap_or_default());
        }
        _ => {
            if parsed.options.contains_key("--modes") {
                return Err(CaeError::new(
                    "cli/invalid-option",
                    "`--modes` can only be used with `--analysis modal` or `--analysis buckling`",
                ));
            }
            let solution = session.solve(out.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&solution).unwrap_or_default());
        }
    }
    Ok(0)
}

fn modal_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--modes", "--out"], &[])?;
    let file = positional(&parsed, 0, "modal <file> [--modes N] [--out report.json]")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let modes = parse_modes(&parsed)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let modal = session.solve_modal(modes, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&modal).unwrap_or_default());
    Ok(0)
}

fn buckling_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--modes", "--out"], &[])?;
    let file = positional(&parsed, 0, "buckling <file> [--modes N] [--out report.json]")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let modes = parse_modes(&parsed)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let buckling = session.solve_buckling(modes, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&buckling).unwrap_or_default());
    Ok(0)
}

fn parse_modes(parsed: &Parsed) -> Result<usize, CaeError> {
    match parsed.options.get("--modes") {
        Some(value) => value.parse::<usize>().map_err(|_| {
            CaeError::new("cli/invalid-number", format!("value `{value}` of `--modes` is not a valid positive integer"))
        }),
        None => Ok(5),
    }
}

fn report(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &[], &[])?;
    let file = positional(&parsed, 0, "report <file>")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let report = session.report()?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn export(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--format", "--out"], &[])?;
    let file = positional(&parsed, 0, "export <file> --format csv|vtk|md --out <dir|file>")?;
    let format = required(&parsed, "--format")?;
    let out = required(&parsed, "--out")?;
    let format = format.parse::<ExportFormat>()?;
    let mut session = session();
    session.open(Path::new(file))?;
    let report = session.export(format, Path::new(out))?;
    println!("exported {} to {}", report.format, report.path);
    Ok(0)
}

fn validate_analysis_options(parsed: &Parsed, model_dimension: u32) -> Result<String, CaeError> {
    let analysis = parsed.options.get("--analysis").cloned().unwrap_or_else(|| "static".to_string());
    match analysis.as_str() {
        "static" | "modal" | "buckling" => {}
        "nonlinear" => return Err(not_implemented("nonlinear analysis")),
        "thermal" => return Err(not_implemented("thermal analysis")),
        other => {
            return Err(CaeError::new(
                "cli/invalid-option",
                format!("unknown analysis type `{other}`; only `static`, `modal` or `buckling` is supported"),
            ));
        }
    }
    if let Some(element) = parsed.options.get("--element") {
        match element.as_str() {
            "truss" | "beam" => {}
            "frame" => return Err(not_implemented("3D frame elements")),
            other => {
                return Err(CaeError::new(
                    "cli/invalid-option",
                    format!("unknown element type `{other}`; only `truss` or `beam` is supported"),
                ));
            }
        }
    }
    if let Some(dimension) = parsed.options.get("--dimension") {
        let requested = parse_dimension(dimension)?;
        if requested != model_dimension {
            return Err(CaeError::new(
                "cli/dimension-mismatch",
                format!(
                    "`--dimension {requested}` does not match the document dimension {model_dimension}; change the document dimension or remove this option"
                ),
            ));
        }
    }
    Ok(analysis)
}

fn parse_dimension(value: &str) -> Result<u32, CaeError> {
    match value {
        "2" => Ok(2),
        "3" => Ok(3),
        other => Err(CaeError::new("cli/invalid-option", format!("unknown dimension `{other}`; only 2 or 3 is supported"))),
    }
}

#[derive(Default)]
struct Parsed {
    positionals: Vec<String>,
    flags: HashSet<String>,
    options: HashMap<String, String>,
}

fn parse(args: &[String], value_options: &[&str], flag_options: &[&str]) -> Result<Parsed, CaeError> {
    let mut parsed = Parsed::default();
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument.starts_with("--") {
            if flag_options.contains(&argument.as_str()) {
                parsed.flags.insert(argument.clone());
                index += 1;
            }
            else if value_options.contains(&argument.as_str()) {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CaeError::new("cli/missing-value", format!("`{argument}` is missing a value")))?;
                if value.starts_with("--") {
                    return Err(CaeError::new("cli/missing-value", format!("`{argument}` is missing a value")));
                }
                parsed.options.insert(argument.clone(), value.clone());
                index += 2;
            }
            else {
                return Err(CaeError::new("cli/unknown-option", format!("unknown option `{argument}`")));
            }
        }
        else {
            parsed.positionals.push(argument.clone());
            index += 1;
        }
    }
    Ok(parsed)
}

fn positional<'a>(parsed: &'a Parsed, index: usize, usage: &str) -> Result<&'a str, CaeError> {
    parsed
        .positionals
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| CaeError::new("cli/missing-argument", format!("missing argument: {usage}")))
}

fn required<'a>(parsed: &'a Parsed, name: &str) -> Result<&'a str, CaeError> {
    parsed
        .options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| CaeError::new("cli/missing-option", format!("missing required option `{name}`")))
}

fn number(parsed: &Parsed, name: &str) -> Result<Option<f64>, CaeError> {
    parsed
        .options
        .get(name)
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CaeError::new("cli/invalid-number", format!("value `{value}` of `{name}` is not a valid number")))
        })
        .transpose()
}

fn required_number(parsed: &Parsed, name: &str) -> Result<f64, CaeError> {
    number(parsed, name)?.ok_or_else(|| CaeError::new("cli/missing-option", format!("missing required option `{name}`")))
}
