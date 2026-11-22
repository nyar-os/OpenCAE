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

use cae_model::{
    CaeError, ElementKind, EndRelease, ExportFormat, Material, OpenCae, OpenCaeOptions, Section, SpectrumCombination,
    SpectrumPoint, Units, angle_section_report, channel_standard_section_report, ellipse_section_report,
    i_standard_section_report, not_implemented, polygon_section_report, section_report, t_section_report,
    tube_standard_section_report,
};

const USAGE: &str = "\
ocae - OpenCAE truss/beam linear-static, modal and buckling solver (2D/3D)

Usage:
  ocae create <file.ocae> [--units mm|m] [--dimension 2|3]
  ocae node add <file> --id <id> --x <x> --y <y> [--z <z>]
  ocae node update <file> --id <id> [--x <x>] [--y <y>] [--z <z>]
  ocae node mass <file> --id <id> --mass <mass>
  ocae node temperature <file> --id <id> [--temperature <t>] [--clear]
  ocae node heat <file> --id <id> --heat <w>
  ocae node remove <file> --id <id>
  ocae node merge <file> --from <id> --into <id>
  ocae node displacement <file> --id <id>
  ocae element add <file> --id <id> --from <node> --to <node> [--E <E>] [--A <A>]
                    [--kind truss|beam] [--I <I>] [--w <w>] [--w2 <w2>] [--dt <dT>]
                    [--convection <h>] [--perimeter <P>]
  ocae element update <file> --id <id> [--from <node>] [--to <node>]
                    [--convection <h>] [--perimeter <P>]
  ocae element split <file> --id <id> --at <fraction>
  ocae element remove <file> --id <id>
  ocae element forces <file> --id <id>
  ocae element release <file> --id <id> [--start] [--end]
  ocae element gradient <file> --id <id> [--value <dT>]
  ocae element span <file> --id <id> [--start <s>] [--end <s>]
  ocae material set <file> --E <E> --A <A> [--density <rho>] [--name <name>] [--alpha <a>] [--conductivity <k>]
  ocae material preset <file> --preset <name> [--section rectangle|circle|i|box|tube|channel]
                        [--width <w>] [--height <h>] [--diameter <d>] [--depth <d>]
                        [--flange-width <b>] [--flange-thickness <t>] [--web-thickness <t>]
                        [--wall <t>] [--outer-diameter <D>] [--inner-diameter <d>]
                        [--area <A>] [--allowable <s>]
  ocae library [--units mm|m]
  ocae section properties --section rectangle|circle|i|box|tube|channel [geometry] [--units mm|m]
  ocae section polygon --points \"x,y;x,y;...\" [--units mm|m]
  ocae section i --height <h> --width <b> --web-thickness <tw> --flange-thickness <tf> [--units mm|m]
  ocae section tube --height <h> --width <b> --thickness <t> [--units mm|m]
  ocae section channel --height <h> --width <b> --web-thickness <tw> --flange-thickness <tf> [--units mm|m]
  ocae section angle --height <h> --width <w> --thickness <t> [--units mm|m]
  ocae section t --height <h> --width <b> --web-thickness <tw> --flange-thickness <tf> [--units mm|m]
  ocae section ellipse --width <w> --height <h> [--units mm|m]
  ocae support add <file> --node <node> [--dx] [--dy] [--dz] [--rz]
                    [--direction <nx,ny>] [--ux <v>] [--uy <v>] [--uz <v>] [--rotation <v>]
                    [--kx <k>] [--ky <k>] [--kz <k>] [--kr <k>]
  ocae support remove <file> --node <node>
  ocae load add <file> --node <node> [--fx <fx>] [--fy <fy>] [--fz <fz>] [--mz <mz>]
  ocae load remove <file> --node <node>
  ocae model gravity <file> [--gx <gx>] [--gy <gy>] [--gz <gz>]
  ocae model ambient <file> [--temperature <Ta>] [--clear]
  ocae self-weight <file> --g <g> [--axis x|y|z]
  ocae combination add <file> --name <name> [--loads <f>] [--gravity <f>]
  ocae combination remove <file> --name <name>
  ocae combinations <file> [--out report.json]
  ocae validate <file>
  ocae solve <file> [--out report.json] [--analysis static|modal|buckling] [--modes N]
  ocae reactions <file> [--out report.json]
  ocae reaction <file> --node <node>
                   [--element truss|beam] [--dimension 2|3]
  ocae modal <file> [--modes N] [--out report.json]
  ocae heat <file> [--out report.json]
  ocae buckling <file> [--modes N] [--out report.json]
  ocae buckling-combinations <file> [--modes N] [--out report.json]
  ocae forces <file> [--stations N] [--out report.json]
  ocae stresses <file> [--stations N] [--out report.json]
  ocae deflections <file> [--stations N] [--out report.json]
  ocae nodal-stresses <file> [--stations N] [--out report.json]
  ocae load-cases <file> [--out report.json]
  ocae matrices <file> [--out report.json]
  ocae spectrum <file> [--modes N] [--axis x|y|z] [--combination srss|cqc] [--damping <z>] --spectrum <T:Sa,...> [--out report.json]
  ocae export <file> --format csv|vtk|md|html --out <dir|file>
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
        "node" => node_command(&args[1..]),
        "element" => element_command(&args[1..]),
        "material" => material_command(&args[1..]),
        "library" => library_command(&args[1..]),
        "section" => section_command(&args[1..]),
        "support" => support_command(&args[1..]),
        "load" => load_command(&args[1..]),
        "model" => model_command(&args[1..]),
        "self-weight" => self_weight(&args[1..]),
        "combination" => combination_command(&args[1..]),
        "combinations" => combinations_command(&args[1..]),
        "validate" => validate(&args[1..]),
        "solve" => solve_command(&args[1..]),
        "reactions" => reactions_command(&args[1..]),
        "reaction" => reaction_command(&args[1..]),
        "modal" => modal_command(&args[1..]),
        "buckling" => buckling_command(&args[1..]),
        "buckling-combinations" => buckling_combinations_command(&args[1..]),
        "forces" => forces_command(&args[1..]),
        "stresses" => stresses_command(&args[1..]),
        "deflections" => deflections_command(&args[1..]),
        "nodal-stresses" => nodal_stresses_command(&args[1..]),
        "load-cases" => load_cases_command(&args[1..]),
        "matrices" => matrices_command(&args[1..]),
        "spectrum" => spectrum_command(&args[1..]),
        "heat" => heat_command(&args[1..]),
        "export" => export(&args[1..]),
        "report" => report(&args[1..]),
        "import" => Err(not_implemented("Abaqus .inp import")),
        "nonlinear" => Err(not_implemented("nonlinear analysis")),
        "beam" | "beam2d" => {
            Err(CaeError::new("cli/unknown-command", "`beam` is not a command; use `--kind beam --I <I>` with `element add`"))
        }
        "frame" | "frame3d" => Err(not_implemented("3D frame elements")),
        "thermal" => heat_command(&args[1..]),
        "contact" => Err(not_implemented("contact analysis")),
        other => Err(CaeError::new("cli/unknown-command", format!("unknown command `{other}`; run --help for usage"))),
    }
}

fn session() -> OpenCae {
    OpenCae::new(OpenCaeOptions::default())
}

fn node_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("add") => node_add(&args[1..]),
        Some("update") => node_update(&args[1..]),
        Some("mass") => node_mass(&args[1..]),
        Some("temperature") => node_temperature(&args[1..]),
        Some("heat") => node_heat(&args[1..]),
        Some("remove") => node_remove(&args[1..]),
        Some("merge") => node_merge(&args[1..]),
        Some("displacement") => node_displacement(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!(
                "subcommand `{other}` of `node` does not exist; only `add`, `update`, `mass`, `temperature`, `heat`, `remove`, `merge` or `displacement` is supported"
            ),
        )),
        None => Err(CaeError::new(
            "cli/missing-argument",
            "`node` is missing a subcommand; only `add`, `update`, `mass`, `temperature`, `heat`, `remove`, `merge` or `displacement` is supported",
        )),
    }
}

fn support_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("add") => support_add(&args[1..]),
        Some("remove") => support_remove(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!("subcommand `{other}` of `support` does not exist; only `add` or `remove` is supported"),
        )),
        None => {
            Err(CaeError::new("cli/missing-argument", "`support` is missing a subcommand; only `add` or `remove` is supported"))
        }
    }
}

fn load_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("add") => load_add(&args[1..]),
        Some("remove") => load_remove(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!("subcommand `{other}` of `load` does not exist; only `add` or `remove` is supported"),
        )),
        None => {
            Err(CaeError::new("cli/missing-argument", "`load` is missing a subcommand; only `add` or `remove` is supported"))
        }
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

fn node_update(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--x", "--y", "--z"], &[])?;
    let file = positional(&parsed, 0, "node update <file> --id <id> [--x <x>] [--y <y>] [--z <z>]")?;
    let id = required(&parsed, "--id")?;
    let x = number(&parsed, "--x")?;
    let y = number(&parsed, "--y")?;
    let z = number(&parsed, "--z")?;
    if x.is_none() && y.is_none() && z.is_none() {
        return Err(CaeError::new(
            "node/invalid-coordinate",
            "specify at least one of `--x`, `--y` or `--z`; an empty move request would silently succeed",
        ));
    }
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.update_node(id, x, y, z)?;
    println!("node `{id}` moved (revision {})", summary.revision);
    Ok(0)
}

fn node_mass(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--mass"], &[])?;
    let file = positional(&parsed, 0, "node mass <file> --id <id> --mass <mass>")?;
    let id = required(&parsed, "--id")?;
    let mass = required_number(&parsed, "--mass")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_node_mass(id, mass)?;
    println!("node `{id}` mass set to {mass} (revision {})", summary.revision);
    Ok(0)
}

fn node_temperature(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--temperature"], &["--clear"])?;
    let file = positional(&parsed, 0, "node temperature <file> --id <id> [--temperature <t>] [--clear]")?;
    let id = required(&parsed, "--id")?;
    let temperature = if parsed.flags.contains("--clear") { None } else { number(&parsed, "--temperature")? };
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_node_temperature(id, temperature)?;
    match temperature {
        Some(value) => println!("node `{id}` temperature set to {value} (revision {})", summary.revision),
        None => println!("node `{id}` temperature cleared (revision {})", summary.revision),
    }
    Ok(0)
}

fn node_heat(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--heat"], &[])?;
    let file = positional(&parsed, 0, "node heat <file> --id <id> --heat <w>")?;
    let id = required(&parsed, "--id")?;
    let heat = required_number(&parsed, "--heat")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_node_heat(id, heat)?;
    println!("node `{id}` heat set to {heat} (revision {})", summary.revision);
    Ok(0)
}

fn node_remove(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id"], &[])?;
    let file = positional(&parsed, 0, "node remove <file> --id <id>")?;
    let id = required(&parsed, "--id")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.remove_node(id)?;
    println!("node `{id}` removed (revision {})", summary.revision);
    Ok(0)
}

fn node_merge(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--from", "--into"], &[])?;
    let file = positional(&parsed, 0, "node merge <file> --from <id> --into <id>")?;
    let from = required(&parsed, "--from")?;
    let into = required(&parsed, "--into")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.merge_nodes(from, into)?;
    println!("node `{from}` merged into `{into}` (revision {})", summary.revision);
    Ok(0)
}

fn node_displacement(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id"], &[])?;
    let file = positional(&parsed, 0, "node displacement <file> --id <id>")?;
    let id = required(&parsed, "--id")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let displacement = session.node_displacement(id)?;
    println!("{}", serde_json::to_string_pretty(&displacement).unwrap_or_default());
    Ok(0)
}

fn element_add(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(
        args,
        &["--id", "--from", "--to", "--E", "--A", "--kind", "--I", "--w", "--w2", "--dt", "--convection", "--perimeter"],
        &[],
    )?;
    let file = positional(
        &parsed,
        0,
        "element add <file> --id <id> --from <node> --to <node> [--kind truss|beam] [--I <I>] [--w <w>] [--w2 <w2>] [--dt <dT>] [--convection <h>] [--perimeter <P>]",
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
    let distributed_load_end = number(&parsed, "--w2")?;
    let temperature = number(&parsed, "--dt")?;
    let convection = number(&parsed, "--convection")?;
    let perimeter = number(&parsed, "--perimeter")?;
    let mut session = session();
    session.open(Path::new(file))?;
    session.add_element_thermal(
        id,
        from,
        to,
        youngs_modulus,
        area,
        kind,
        inertia,
        distributed_load,
        distributed_load_end,
        temperature,
    )?;
    if convection.is_some() {
        session.set_element_convection(id, convection)?;
    }
    if perimeter.is_some() {
        session.set_element_perimeter(id, perimeter)?;
    }
    let summary = session.summary()?;
    println!("element `{id}` added as {} (revision {})", kind.as_str(), summary.revision);
    Ok(0)
}

fn element_update(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--from", "--to", "--convection", "--perimeter"], &[])?;
    let file = positional(
        &parsed,
        0,
        "element update <file> --id <id> [--from <node>] [--to <node>] [--convection <h>] [--perimeter <P>]",
    )?;
    let id = required(&parsed, "--id")?;
    let from = parsed.options.get("--from").map(String::as_str);
    let to = parsed.options.get("--to").map(String::as_str);
    let convection = number(&parsed, "--convection")?;
    let perimeter = number(&parsed, "--perimeter")?;
    if from.is_none() && to.is_none() && convection.is_none() && perimeter.is_none() {
        return Err(CaeError::new(
            "element/degenerate",
            "specify at least one of `--from`, `--to`, `--convection` or `--perimeter`; an empty request would silently succeed",
        ));
    }
    let mut session = session();
    session.open(Path::new(file))?;
    if from.is_some() || to.is_some() {
        session.update_element(id, from, to)?;
    }
    if convection.is_some() {
        session.set_element_convection(id, convection)?;
    }
    if perimeter.is_some() {
        session.set_element_perimeter(id, perimeter)?;
    }
    let summary = session.summary()?;
    println!("element `{id}` updated (revision {})", summary.revision);
    Ok(0)
}

fn element_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("add") => element_add(&args[1..]),
        Some("update") => element_update(&args[1..]),
        Some("split") => element_split(&args[1..]),
        Some("remove") => element_remove(&args[1..]),
        Some("forces") => element_forces(&args[1..]),
        Some("release") => element_release(&args[1..]),
        Some("gradient") => element_gradient(&args[1..]),
        Some("span") => element_span(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!(
                "subcommand `{other}` of `element` does not exist; only `add`, `update`, `split`, `remove`, `forces`, `release`, `gradient` or `span` is supported"
            ),
        )),
        None => Err(CaeError::new(
            "cli/missing-argument",
            "`element` is missing a subcommand; only `add`, `update`, `split`, `remove`, `forces`, `release`, `gradient` or `span` is supported",
        )),
    }
}

fn element_split(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--at"], &[])?;
    let file = positional(&parsed, 0, "element split <file> --id <id> --at <fraction>")?;
    let id = required(&parsed, "--id")?;
    let at = required_number(&parsed, "--at")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let report = session.split_element(id, at)?;
    println!(
        "element `{id}` split at {at}: node `{}`, elements `{}` and `{}` (revision {})",
        report.split.node, report.split.first, report.split.second, report.summary.revision
    );
    Ok(0)
}

fn element_remove(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id"], &[])?;
    let file = positional(&parsed, 0, "element remove <file> --id <id>")?;
    let id = required(&parsed, "--id")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.remove_element(id)?;
    println!("element `{id}` removed (revision {})", summary.revision);
    Ok(0)
}

fn element_forces(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id"], &[])?;
    let file = positional(&parsed, 0, "element forces <file> --id <id>")?;
    let id = required(&parsed, "--id")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let result = session.element_result(id)?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
    Ok(0)
}

fn element_release(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id"], &["--start", "--end"])?;
    let file = positional(&parsed, 0, "element release <file> --id <id> [--start] [--end]")?;
    let id = required(&parsed, "--id")?;
    let start = parsed.flags.contains("--start");
    let end = parsed.flags.contains("--end");
    if !start && !end {
        return Err(CaeError::new(
            "cli/missing-option",
            "specify `--start` and/or `--end`; a release must free at least one end",
        ));
    }
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_element_release(id, Some(EndRelease { start, end }))?;
    println!("element `{id}` released (start={start}, end={end}, revision {})", summary.revision);
    Ok(0)
}

fn element_gradient(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--value"], &[])?;
    let file = positional(&parsed, 0, "element gradient <file> --id <id> [--value <dT>]")?;
    let id = required(&parsed, "--id")?;
    let value = number(&parsed, "--value")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_element_temperature_gradient(id, value)?;
    match value {
        Some(value) => println!("element `{id}` gradient set to {value} (revision {})", summary.revision),
        None => println!("element `{id}` gradient cleared (revision {})", summary.revision),
    }
    Ok(0)
}

fn element_span(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--id", "--start", "--end"], &[])?;
    let file = positional(&parsed, 0, "element span <file> --id <id> [--start <s>] [--end <s>]")?;
    let id = required(&parsed, "--id")?;
    let start = number(&parsed, "--start")?;
    let end = number(&parsed, "--end")?;
    let span = match (start, end) {
        (Some(start), Some(end)) => Some((start, end)),
        (None, None) => None,
        _ => {
            return Err(CaeError::new(
                "cli/missing-option",
                "specify both `--start` and `--end`, or neither to restore a full-span load",
            ));
        }
    };
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_element_load_span(id, span)?;
    match span {
        Some((start, end)) => {
            println!("element `{id}` load span set to {start}..{end} (revision {})", summary.revision)
        }
        None => println!("element `{id}` load span cleared (revision {})", summary.revision),
    }
    Ok(0)
}

fn material_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("set") => material_set(&args[1..]),
        Some("preset") => material_preset(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!("subcommand `{other}` of `material` does not exist; only `set` or `preset` is supported"),
        )),
        None => Err(CaeError::new(
            "cli/missing-argument",
            "`material` is missing a subcommand; only `set` or `preset` is supported",
        )),
    }
}

fn material_preset(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(
        args,
        &[
            "--preset",
            "--area",
            "--allowable",
            "--section",
            "--width",
            "--height",
            "--diameter",
            "--depth",
            "--flange-width",
            "--flange-thickness",
            "--web-thickness",
            "--wall",
            "--outer-diameter",
            "--inner-diameter",
        ],
        &[],
    )?;
    let file = positional(
        &parsed,
        0,
        "material preset <file> --preset <name> [--section rectangle|circle|i|box|tube|channel] [--area A] [--allowable S]",
    )?;
    let preset = required(&parsed, "--preset")?;
    let area = number(&parsed, "--area")?;
    let allowable = number(&parsed, "--allowable")?;
    let section = optional_section(&parsed)?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.apply_material_preset(preset, area, section, allowable)?;
    println!(
        "material preset `{preset}` applied: E={} A={} density={} allowable={:?} section={} (revision {})",
        summary.material.youngs_modulus,
        summary.material.area,
        summary.material.density,
        summary.material.allowable_stress,
        summary.material.section.map(Section::shape_name).unwrap_or("none"),
        summary.revision
    );
    Ok(0)
}

fn optional_section(parsed: &Parsed) -> Result<Option<Section>, CaeError> {
    let Some(shape) = parsed.options.get("--section")
    else {
        let geometry = [
            "--width",
            "--height",
            "--diameter",
            "--depth",
            "--flange-width",
            "--flange-thickness",
            "--web-thickness",
            "--wall",
            "--outer-diameter",
            "--inner-diameter",
        ];
        if geometry.iter().any(|option| parsed.options.contains_key(*option)) {
            return Err(CaeError::new(
                "cli/missing-option",
                "section dimensions require `--section rectangle`, `--section circle`, `--section i`, `--section box`, `--section tube` or `--section channel`",
            ));
        }
        return Ok(None);
    };
    let section = match shape.as_str() {
        "rectangle" => Section::rectangle(required_number(parsed, "--width")?, required_number(parsed, "--height")?)?,
        "circle" => Section::circle(required_number(parsed, "--diameter")?)?,
        "i" | "i-section" => Section::i_section(
            required_number(parsed, "--depth")?,
            required_number(parsed, "--flange-width")?,
            required_number(parsed, "--flange-thickness")?,
            required_number(parsed, "--web-thickness")?,
        )?,
        "box" => Section::box_section(
            required_number(parsed, "--width")?,
            required_number(parsed, "--height")?,
            required_number(parsed, "--wall")?,
        )?,
        "tube" => Section::tube(required_number(parsed, "--outer-diameter")?, required_number(parsed, "--inner-diameter")?)?,
        "channel" => Section::channel(
            required_number(parsed, "--depth")?,
            required_number(parsed, "--flange-width")?,
            required_number(parsed, "--flange-thickness")?,
            required_number(parsed, "--web-thickness")?,
        )?,
        other => {
            return Err(CaeError::new(
                "cli/invalid-option",
                format!(
                    "unknown section shape `{other}`; valid values are `rectangle`, `circle`, `i`, `box`, `tube` or `channel`"
                ),
            ));
        }
    };
    Ok(Some(section))
}

fn library_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    println!("{}", cae_model::library_json(units)?);
    Ok(0)
}

fn section_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("properties") => section_properties(&args[1..]),
        Some("polygon") => section_polygon(&args[1..]),
        Some("i") => section_i(&args[1..]),
        Some("tube") => section_tube(&args[1..]),
        Some("channel") => section_channel(&args[1..]),
        Some("angle") => section_angle(&args[1..]),
        Some("t") => section_t(&args[1..]),
        Some("ellipse") => section_ellipse(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!(
                "subcommand `{other}` of `section` does not exist; only `properties`, `polygon`, `i`, `tube`, `channel`, `angle`, `t` or `ellipse` is supported"
            ),
        )),
        None => Err(CaeError::new(
            "cli/missing-argument",
            "`section` is missing a subcommand; only `properties`, `polygon`, `i`, `tube`, `channel`, `angle`, `t` or `ellipse` is supported",
        )),
    }
}

fn section_properties(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(
        args,
        &[
            "--units",
            "--section",
            "--width",
            "--height",
            "--diameter",
            "--depth",
            "--flange-width",
            "--flange-thickness",
            "--web-thickness",
            "--wall",
            "--outer-diameter",
            "--inner-diameter",
        ],
        &[],
    )?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let section = optional_section(&parsed)?.ok_or_else(|| {
        CaeError::new(
            "cli/missing-option",
            "`--section` is required; use `--section rectangle`, `--section circle`, `--section i`, `--section box`, `--section tube` or `--section channel`",
        )
    })?;
    let report = section_report(section, units)?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn section_polygon(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--points", "--units"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let points = parse_points(required(&parsed, "--points")?)?;
    let report = polygon_section_report(&points, units)?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn section_i(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units", "--height", "--width", "--web-thickness", "--flange-thickness"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let report = i_standard_section_report(
        required_number(&parsed, "--height")?,
        required_number(&parsed, "--width")?,
        required_number(&parsed, "--web-thickness")?,
        required_number(&parsed, "--flange-thickness")?,
        units,
    )?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn section_tube(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units", "--height", "--width", "--thickness"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let report = tube_standard_section_report(
        required_number(&parsed, "--height")?,
        required_number(&parsed, "--width")?,
        required_number(&parsed, "--thickness")?,
        units,
    )?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn section_channel(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units", "--height", "--width", "--web-thickness", "--flange-thickness"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let report = channel_standard_section_report(
        required_number(&parsed, "--height")?,
        required_number(&parsed, "--width")?,
        required_number(&parsed, "--web-thickness")?,
        required_number(&parsed, "--flange-thickness")?,
        units,
    )?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn section_angle(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units", "--height", "--width", "--thickness"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let report = angle_section_report(
        required_number(&parsed, "--height")?,
        required_number(&parsed, "--width")?,
        required_number(&parsed, "--thickness")?,
        units,
    )?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn section_t(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units", "--height", "--width", "--web-thickness", "--flange-thickness"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let report = t_section_report(
        required_number(&parsed, "--height")?,
        required_number(&parsed, "--width")?,
        required_number(&parsed, "--web-thickness")?,
        required_number(&parsed, "--flange-thickness")?,
        units,
    )?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn section_ellipse(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--units", "--width", "--height"], &[])?;
    let units = match parsed.options.get("--units") {
        Some(value) => value.parse::<Units>()?,
        None => Units::Millimeter,
    };
    let report = ellipse_section_report(required_number(&parsed, "--width")?, required_number(&parsed, "--height")?, units)?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    Ok(0)
}

fn parse_points(value: &str) -> Result<Vec<(f64, f64)>, CaeError> {
    let mut points = Vec::new();
    for item in value.split(';').map(str::trim).filter(|item| !item.is_empty()) {
        let Some((x, y)) = item.split_once(',')
        else {
            return Err(CaeError::new("cli/invalid-points", format!("polygon point `{item}` must use the form `x,y`")));
        };
        let coordinate = |axis: &str, raw: &str| -> Result<f64, CaeError> {
            raw.trim().parse::<f64>().map_err(|_| {
                CaeError::new(
                    "cli/invalid-points",
                    format!("{axis} coordinate `{}` of polygon point `{item}` is not a number", raw.trim()),
                )
            })
        };
        points.push((coordinate("x", x)?, coordinate("y", y)?));
    }
    Ok(points)
}

fn material_set(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(
        args,
        &[
            "--E",
            "--A",
            "--density",
            "--name",
            "--allowable",
            "--alpha",
            "--conductivity",
            "--section",
            "--width",
            "--height",
            "--diameter",
            "--depth",
            "--flange-width",
            "--flange-thickness",
            "--web-thickness",
            "--wall",
            "--outer-diameter",
            "--inner-diameter",
        ],
        &[],
    )?;
    let file = positional(&parsed, 0, "material set <file> --E <E> --A <A> [--density <rho>] [--name <name>]")?;
    let youngs_modulus = required_number(&parsed, "--E")?;
    let area = required_number(&parsed, "--A")?;
    let density = number(&parsed, "--density")?;
    let allowable = number(&parsed, "--allowable")?;
    let alpha = number(&parsed, "--alpha")?;
    let conductivity = number(&parsed, "--conductivity")?;
    let section = optional_section(&parsed)?;
    let name = parsed.options.get("--name").cloned();
    let mut session = session();
    let current = session.open(Path::new(file))?;
    let material = Material {
        name: name.unwrap_or(current.material.name.clone()),
        youngs_modulus,
        area,
        density: density.unwrap_or(current.material.density),
        allowable_stress: allowable.or(current.material.allowable_stress),
        alpha: alpha.or(current.material.alpha),
        conductivity: conductivity.or(current.material.conductivity),
        section: section.or(current.material.section),
    };
    let summary = session.set_material_data(material)?;
    println!(
        "material set: E={} A={} density={} allowable={:?} section={} (revision {})",
        summary.material.youngs_modulus,
        summary.material.area,
        summary.material.density,
        summary.material.allowable_stress,
        summary.material.section.map(Section::shape_name).unwrap_or("none"),
        summary.revision
    );
    Ok(0)
}

fn parse_direction(value: &str) -> Result<(f64, f64), CaeError> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    if parts.len() != 2 {
        return Err(CaeError::new(
            "cli/invalid-option",
            format!("`--direction` must be two comma-separated numbers `nx,ny`, not `{value}`"),
        ));
    }
    let nx = parts[0].parse::<f64>().map_err(|_| {
        CaeError::new("cli/invalid-option", format!("`--direction` X component `{}` is not a number", parts[0]))
    })?;
    let ny = parts[1].parse::<f64>().map_err(|_| {
        CaeError::new("cli/invalid-option", format!("`--direction` Y component `{}` is not a number", parts[1]))
    })?;
    Ok((nx, ny))
}

fn support_add(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(
        args,
        &["--node", "--ux", "--uy", "--uz", "--rotation", "--kx", "--ky", "--kz", "--kr", "--direction"],
        &["--dx", "--dy", "--dz", "--rz"],
    )?;
    let file = positional(
        &parsed,
        0,
        "support add <file> --node <node> [--dx] [--dy] [--dz] [--rz] [--direction <nx,ny>] [--ux <v>] [--uy <v>] [--uz <v>] [--rotation <v>] [--kx <k>] [--ky <k>] [--kz <k>] [--kr <k>]",
    )?;
    let node = required(&parsed, "--node")?;
    let dx = parsed.flags.contains("--dx");
    let dy = parsed.flags.contains("--dy");
    let dz = parsed.flags.contains("--dz");
    let rz = parsed.flags.contains("--rz");
    let ux = number(&parsed, "--ux")?;
    let uy = number(&parsed, "--uy")?;
    let uz = number(&parsed, "--uz")?;
    let rotation = number(&parsed, "--rotation")?;
    let kx = number(&parsed, "--kx")?;
    let ky = number(&parsed, "--ky")?;
    let kz = number(&parsed, "--kz")?;
    let kr = number(&parsed, "--kr")?;
    let direction = parsed.options.get("--direction").map(|value| parse_direction(value)).transpose()?;
    let prescribed = ux.is_some() || uy.is_some() || uz.is_some() || rotation.is_some();
    let spring = kx.is_some() || ky.is_some() || kz.is_some() || kr.is_some();
    if direction.is_some() && (dx || dy || dz || rz || prescribed || spring) {
        return Err(CaeError::new(
            "cli/direction-conflict",
            "`--direction` is an independent inclined-support constraint; do not combine it with `--dx`, `--dy`, `--dz`, `--rz`, a prescribed displacement or a spring",
        ));
    }
    if direction.is_none() && !dx && !dy && !dz && !rz && !prescribed && !spring {
        return Err(CaeError::new(
            "cli/missing-option",
            "specify at least one of `--dx`, `--dy`, `--dz`, `--rz`, `--direction`, `--ux`, `--uy`, `--uz`, `--rotation`, `--kx`, `--ky`, `--kz` or `--kr`; otherwise the support has no effect",
        ));
    }
    let mut session = session();
    session.open(Path::new(file))?;
    if let Some((nx, ny)) = direction {
        let summary = session.add_support_direction(node, nx, ny)?;
        println!("support `{node}` added (direction=({nx}, {ny}), revision {})", summary.revision);
        return Ok(0);
    }
    if dx || dy || dz || rz {
        session.add_support_full(node, dx, dy, dz, rz)?;
    }
    if prescribed {
        session.prescribe_displacement(node, ux, uy, uz, rotation)?;
    }
    let summary = if spring { session.set_spring(node, kx, ky, kz, kr)? } else { session.summary()? };
    println!(
        "support `{node}` added (dx={dx}, dy={dy}, dz={dz}, rz={rz}, ux={ux:?}, uy={uy:?}, uz={uz:?}, rotation={rotation:?}, kx={kx:?}, ky={ky:?}, kz={kz:?}, kr={kr:?}, revision {})",
        summary.revision
    );
    Ok(0)
}

fn support_remove(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--node"], &[])?;
    let file = positional(&parsed, 0, "support remove <file> --node <node>")?;
    let node = required(&parsed, "--node")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.remove_support(node)?;
    println!("support `{node}` removed (revision {})", summary.revision);
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

fn load_remove(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--node"], &[])?;
    let file = positional(&parsed, 0, "load remove <file> --node <node>")?;
    let node = required(&parsed, "--node")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.remove_load(node)?;
    println!("load `{node}` removed (revision {})", summary.revision);
    Ok(0)
}

fn model_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("gravity") => gravity(&args[1..]),
        Some("ambient") => ambient(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!("subcommand `{other}` of `model` does not exist; only `gravity` or `ambient` is supported"),
        )),
        None => Err(CaeError::new(
            "cli/missing-argument",
            "`model` is missing a subcommand; only `gravity` or `ambient` is supported",
        )),
    }
}

fn ambient(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--temperature"], &["--clear"])?;
    let file = positional(&parsed, 0, "model ambient <file> [--temperature <Ta>] [--clear]")?;
    let temperature = if parsed.flags.contains("--clear") { None } else { number(&parsed, "--temperature")? };
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.set_ambient_temperature(temperature)?;
    match temperature {
        Some(value) => println!("ambient temperature set to {value} (revision {})", summary.revision),
        None => println!("ambient temperature cleared (revision {})", summary.revision),
    }
    Ok(0)
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

fn combination_command(args: &[String]) -> Result<i32, CaeError> {
    match args.first().map(String::as_str) {
        Some("add") => combination_add(&args[1..]),
        Some("remove") => combination_remove(&args[1..]),
        Some(other) => Err(CaeError::new(
            "cli/unknown-command",
            format!("subcommand `{other}` of `combination` does not exist; only `add` or `remove` is supported"),
        )),
        None => Err(CaeError::new(
            "cli/missing-argument",
            "`combination` is missing a subcommand; only `add` or `remove` is supported",
        )),
    }
}

fn combination_add(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--name", "--loads", "--gravity"], &[])?;
    let file = positional(&parsed, 0, "combination add <file> --name <name> [--loads <f>] [--gravity <f>]")?;
    let name = required(&parsed, "--name")?;
    let loads = number(&parsed, "--loads")?.unwrap_or(1.0);
    let gravity = number(&parsed, "--gravity")?.unwrap_or(0.0);
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.add_combination(name, loads, gravity)?;
    println!("combination `{name}` set (loads={loads}, gravity={gravity}, revision {})", summary.revision);
    Ok(0)
}

fn combination_remove(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--name"], &[])?;
    let file = positional(&parsed, 0, "combination remove <file> --name <name>")?;
    let name = required(&parsed, "--name")?;
    let mut session = session();
    session.open(Path::new(file))?;
    if !session.remove_combination(name)? {
        return Err(CaeError::new("combination/unknown-name", format!("no load combination named `{name}` exists")));
    }
    println!("combination `{name}` removed");
    Ok(0)
}

fn combinations_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--out"], &[])?;
    let file = positional(&parsed, 0, "combinations <file> [--out report.json]")?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let result = session.solve_combinations(out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
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

fn reactions_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--out"], &[])?;
    let file = positional(&parsed, 0, "reactions <file> [--out report.json]")?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.reactions(out.as_deref())?;
    println!(
        "reactions: {} support(s), reaction=({}, {}, {}), applied=({}, {}, {}), unbalanced=({}, {}, {}), tolerance={}, balanced={}",
        summary.supports.len(),
        summary.reaction.fx,
        summary.reaction.fy,
        summary.reaction.fz,
        summary.applied.fx,
        summary.applied.fy,
        summary.applied.fz,
        summary.unbalanced.fx,
        summary.unbalanced.fy,
        summary.unbalanced.fz,
        summary.tolerance,
        summary.balanced
    );
    println!("{}", serde_json::to_string_pretty(&summary).unwrap_or_default());
    Ok(0)
}

fn reaction_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--node"], &[])?;
    let file = positional(&parsed, 0, "reaction <file> --node <node>")?;
    let node = required(&parsed, "--node")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let reaction = session.node_reaction(node)?;
    println!("{}", serde_json::to_string_pretty(&reaction).unwrap_or_default());
    Ok(0)
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

fn heat_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--out"], &[])?;
    let file = positional(&parsed, 0, "heat <file> [--out report.json]")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let result = session.solve_heat(out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
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

fn buckling_combinations_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--modes", "--out"], &[])?;
    let file = positional(&parsed, 0, "buckling-combinations <file> [--modes N] [--out report.json]")?;
    let mut session = session();
    session.open(Path::new(file))?;
    let modes = parse_modes(&parsed)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let result = session.solve_buckling_combinations(modes, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
    Ok(0)
}

fn forces_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--stations", "--out"], &[])?;
    let file = positional(&parsed, 0, "forces <file> [--stations N] [--out report.json]")?;
    let stations = parse_stations(&parsed)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let forces = session.internal_forces(stations, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&forces).unwrap_or_default());
    Ok(0)
}

fn stresses_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--stations", "--out"], &[])?;
    let file = positional(&parsed, 0, "stresses <file> [--stations N] [--out report.json]")?;
    let stations = parse_stations(&parsed)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let stresses = session.stresses(stations, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&stresses).unwrap_or_default());
    Ok(0)
}

fn deflections_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--stations", "--out"], &[])?;
    let file = positional(&parsed, 0, "deflections <file> [--stations N] [--out report.json]")?;
    let stations = parse_stations(&parsed)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let deflections = session.deflections(stations, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&deflections).unwrap_or_default());
    Ok(0)
}

fn nodal_stresses_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--stations", "--out"], &[])?;
    let file = positional(&parsed, 0, "nodal-stresses <file> [--stations N] [--out report.json]")?;
    let stations = parse_stations(&parsed)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let stresses = session.nodal_stresses(stations, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&stresses).unwrap_or_default());
    Ok(0)
}

fn load_cases_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--out"], &[])?;
    let file = positional(&parsed, 0, "load-cases <file> [--out report.json]")?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let summary = session.load_cases(out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&summary).unwrap_or_default());
    Ok(0)
}

fn parse_spectrum(value: &str) -> Result<Vec<SpectrumPoint>, CaeError> {
    let mut points = Vec::new();
    for item in value.split(',').map(str::trim).filter(|item| !item.is_empty()) {
        let (period, acceleration) = item.split_once(':').ok_or_else(|| {
            CaeError::new("cli/invalid-spectrum", format!("spectrum point `{item}` must use the form `period:acceleration`"))
        })?;
        let period = period.trim().parse::<f64>().map_err(|_| {
            CaeError::new(
                "cli/invalid-spectrum",
                format!("period `{}` of spectrum point `{item}` is not a number", period.trim()),
            )
        })?;
        let acceleration = acceleration.trim().parse::<f64>().map_err(|_| {
            CaeError::new(
                "cli/invalid-spectrum",
                format!("acceleration `{}` of spectrum point `{item}` is not a number", acceleration.trim()),
            )
        })?;
        points.push(SpectrumPoint { period, acceleration });
    }
    if points.is_empty() {
        return Err(CaeError::new("cli/invalid-spectrum", "provide at least one `period:acceleration` spectrum point"));
    }
    Ok(points)
}

fn parse_axis(value: &str) -> Result<usize, CaeError> {
    match value {
        "x" => Ok(0),
        "y" => Ok(1),
        "z" => Ok(2),
        other => {
            Err(CaeError::new("cli/invalid-option", format!("unknown spectrum axis `{other}`; only x, y or z is supported")))
        }
    }
}

fn spectrum_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--modes", "--axis", "--spectrum", "--combination", "--damping", "--out"], &[])?;
    let file = positional(
        &parsed,
        0,
        "spectrum <file> [--modes N] [--axis x|y|z] [--combination srss|cqc] [--damping <z>] --spectrum <T:Sa,...> [--out report.json]",
    )?;
    let modes = parse_modes(&parsed)?;
    let axis = match parsed.options.get("--axis") {
        Some(value) => parse_axis(value)?,
        None => 0,
    };
    let combination = SpectrumCombination::parse(parsed.options.get("--combination").map_or("srss", String::as_str))?;
    let damping = match parsed.options.get("--damping") {
        Some(value) => Some(
            value
                .parse::<f64>()
                .map_err(|_| CaeError::new("cli/invalid-number", format!("value `{value}` of `--damping` is not a number")))?,
        ),
        None => None,
    };
    let spectrum_value = required(&parsed, "--spectrum")?;
    let spectrum = parse_spectrum(spectrum_value)?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let result = session.response_spectrum(modes, axis, &spectrum, combination, damping, out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
    Ok(0)
}

fn matrices_command(args: &[String]) -> Result<i32, CaeError> {
    let parsed = parse(args, &["--out"], &[])?;
    let file = positional(&parsed, 0, "matrices <file> [--out report.json]")?;
    let out = parsed.options.get("--out").map(PathBuf::from);
    let mut session = session();
    session.open(Path::new(file))?;
    let matrices = session.matrices(out.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&matrices).unwrap_or_default());
    Ok(0)
}

fn parse_stations(parsed: &Parsed) -> Result<usize, CaeError> {
    match parsed.options.get("--stations") {
        Some(value) => value.parse::<usize>().map_err(|_| {
            CaeError::new("cli/invalid-number", format!("value `{value}` of `--stations` is not a valid positive integer"))
        }),
        None => Ok(11),
    }
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
    let file = positional(&parsed, 0, "export <file> --format csv|vtk|md|html --out <dir|file>")?;
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
