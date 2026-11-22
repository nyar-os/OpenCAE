# `open-cae` command reference

The `open-cae` CLI from `@wae/open-cae` is a thin wrapper over the same
session API as the SDK. It parses arguments, calls the API, prints JSON and
maps errors to exit codes.

```shell
pnpm add @wae/open-cae
pnpm exec open-cae --help
```

Every command takes a document path. Commands that edit a document write the
file atomically; every command that solves or queries prints a JSON report to
stdout, and `--out <file>` also writes that JSON report to the path.

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | success |
| `1` | input, validation, IO or unknown-command error |
| `2` | capability not implemented (`capability/not-implemented`) |

`--help` (also `-h`, `help`) prints the usage block.

## Commands

### Document

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae create <file.ocae>` | `[--units mm\|m] [--dimension 2\|3]` | Creates a document; defaults are `mm` and 2D. Refuses to replace an existing file. |
| `open-cae validate <file>` | — | Prints validation issues; exit code 1 when errors exist. |
| `open-cae report <file>` | — | Solves and prints the document summary, issues and solution. |
| `open-cae capabilities` | — | Prints the machine-readable capability matrix. |

### Nodes

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae node add <file>` | `--id <id> --x <x> --y <y> [--z <z>]` | `--z` must be 0 in 2D documents. |
| `open-cae node update <file>` | `--id <id> [--x <x>] [--y <y>] [--z <z>]` | Moves a node in place and keeps every reference; at least one coordinate is required. |
| `open-cae node mass <file>` | `--id <id> --mass <mass>` | Concentrated mass for modal analysis; `0` clears it. Tonnes in `mm`, kilograms in `m`, matching `density`. |
| `open-cae node remove <file>` | `--id <id>` | Rejected while an element references the node. |
| `open-cae node merge <file>` | `--from <id> --into <id>` | Re-points element endpoints, adds loads and merges supports; `into` keeps its coordinates. |
| `open-cae node displacement <file>` | `--id <id>` | Prints one node's solved displacement. |

### Elements

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae element add <file>` | `--id <id> --from <n> --to <n> [--E <E>] [--A <A>] [--kind truss\|beam] [--I <I>] [--w <w>] [--w2 <w2>] [--dt <dT>] [--dt-grad <dT>] [--w-from <s>] [--w-to <s>] [--tension-only]` | `beam` is 2D-only and needs positive `--I` or a material section. `--w`/`--w2` are local `+y` intensities at the start/end node. `--w-from`/`--w-to` limit the load to a length fraction (both or neither). `--dt` needs material `alpha`; `--dt-grad` also needs a section depth. `--tension-only` marks a truss as a cable. |
| `open-cae element update <file>` | `--id <id> [--from <n>] [--to <n>]` | Reconnects endpoints and preserves material, loads, temperature and releases. |
| `open-cae element remove <file>` | `--id <id>` | Removes one element. |
| `open-cae element forces <file>` | `--id <id>` | Prints the solved result of one element. |
| `open-cae element release <file>` | `--id <id> [--start] [--end]` | Releases the bending moment at one or both beam ends; at least one flag is required. Trusses are rejected. |
| `open-cae element tension-only <file>` | `--id <id> [--clear]` | Marks a truss as tension-only, or clears it with `--clear`. Beams are rejected. |

### Material and sections

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae material set <file>` | `--E <E> --A <A> [--density <rho>] [--name <name>] [--allowable <s>] [--alpha <a>] [--section <shape> ...geometry]` | Sets the global material; `--alpha` is the thermal expansion coefficient in `1/K`. |
| `open-cae material preset <file>` | `--preset <name> [--section <shape> ...geometry] [--area <A>] [--allowable <s>]` | Presets: `steel`, `stainless-steel`, `aluminum`, `concrete`, `timber`. |
| `open-cae library` | `[--units mm\|m]` | Prints presets and supported section shapes. |
| `open-cae section properties` | `--section <shape> [...geometry] [--units mm\|m]` | Prints area, inertias, torsion constant, extreme fibre and section modulus. |

Section shapes and their geometry flags:

| Shape | Flags |
|-------|-------|
| `rectangle` | `--width --height` |
| `circle` | `--diameter` |
| `i` | `--depth --flange-width --flange-thickness --web-thickness` |
| `box` | `--width --height --wall` |
| `tube` | `--outer-diameter --inner-diameter` |
| `channel` | `--depth --flange-width --flange-thickness --web-thickness` |

### Supports and loads

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae support add <file>` | `--node <n> [--dx] [--dy] [--dz] [--rz] [--direction <nx,ny>] [--ux <v>] [--uy <v>] [--uz <v>] [--rotation <v>] [--kx] [--ky] [--kz] [--kr]` | At least one freedom, displacement or spring is required. `--rz`/`--kr` are 2D beam-node-only; `--dz`/`--kz` are 3D-only. `--direction` is an independent inclined constraint and cannot be combined with the other options. A supplied displacement constrains that degree of freedom; a supplied spring leaves it free. |
| `open-cae support remove <file>` | `--node <n>` | Removes the support entry. |
| `open-cae load add <file>` | `--node <n> [--fx] [--fy] [--fz] [--mz]` | Adds or updates a nodal force/moment; `--mz` is beam-node-only and `--fz` is 3D-only. |
| `open-cae load remove <file>` | `--node <n>` | Removes the nodal load. |
| `open-cae model gravity <file>` | `[--gx] [--gy] [--gz]` | Sets the gravity vector; omitted components are 0. |
| `open-cae self-weight <file>` | `--g <g> [--axis x\|y\|z]` | Shorthand for gravity along one axis; `--axis` defaults to `y`. |

### Combinations

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae combination add <file>` | `--name <name> [--loads <f>] [--gravity <f>]` | Adds or replaces a combination; defaults are `1` and `0`. |
| `open-cae combination remove <file>` | `--name <name>` | Removes a combination. |
| `open-cae combinations <file>` | `[--out <file>]` | Solves every combination and prints solutions plus the envelope. |
| `open-cae buckling-combinations <file>` | `[--modes <N>] [--out <file>]` | Buckling factors per combination and the governing combination. |

### Analysis

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae solve <file>` | `[--out <file>] [--analysis static\|modal\|buckling] [--modes <N>] [--element truss\|beam] [--dimension 2\|3]` | Static by default. `--modes` (default 5) is modal/buckling-only. `--dimension` must match the document. `--element` is validated but does not change the solve. |
| `open-cae modal <file>` | `[--modes <N>] [--out <file>]` | Modal solve. Needs a positive density or at least one positive nodal mass. |
| `open-cae buckling <file>` | `[--modes <N>] [--out <file>]` | Linear buckling solve. |
| `open-cae tension-only <file>` | `[--iterations <N>] [--tolerance <t>] [--out <file>]` | Cable active-set iteration; defaults are 20 and `1e-9`. |

Use the dedicated `tension-only` command for cables. The `solve` parser also
accepts `--analysis tension-only`, but currently resolves it to a static
solve, and it rejects `--iterations`/`--tolerance` outside the dedicated
command.

### Results

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae reactions <file>` | `[--out <file>]` | Per-support reactions and the global equilibrium check. |
| `open-cae reaction <file>` | `--node <n>` | One support reaction. |
| `open-cae forces <file>` | `[--stations <N>] [--out <file>]` | Sampled axial force, shear and bending moment; `--stations` defaults to 11 and must be at least 2. |
| `open-cae stresses <file>` | `[--stations <N>] [--out <file>]` | Extreme-fibre stresses, governing station and utilisation. A beam needs a material section. |
| `open-cae deflections <file>` | `[--stations <N>] [--out <file>]` | Local deflection/rotation and deformed coordinates. |
| `open-cae nodal-stresses <file>` | `[--stations <N>] [--out <file>]` | Averaged nodal stress and extremes. |
| `open-cae load-cases <file>` | `[--out <file>]` | One summary row for `base` and each combination. |
| `open-cae matrices <file>` | `[--out <file>]` | Assembled global stiffness and lumped mass matrices with row labels. |
| `open-cae spectrum <file>` | `[--modes <N>] [--axis x\|y\|z] --spectrum <T:Sa,...> [--out <file>]` | Response spectrum from `period:acceleration` points, combined with SRSS; `--axis` defaults to `x`. |
| `open-cae export <file>` | `--format csv\|vtk\|md\|svg --out <dir\|file> [--scale N]` | CSV writes `nodes.csv`, `elements.csv` and `reactions.csv`; VTK is legacy ASCII. `--out` is a directory for CSV/VTK and a file for Markdown, HTML and SVG. `--scale` (default `1`) multiplies the nodal displacements of the SVG deformed shape. SVG is deterministic, projects onto the global X–Y plane and carries a title and legend. HTML export is available through the SDK/API, not this CLI. |

### Rejected commands

`open-cae import`, `nonlinear`, `frame`, `frame3d`, `thermal` and `contact`
return `capability/not-implemented` with exit code `2`. `beam` and `beam2d`
are unknown commands (exit code `1`).

## Examples

Two-bar truss, 1000 mm span, 500 mm rise, −1000 N apex load:

```shell
open-cae create demo.ocae --units mm
open-cae material set demo.ocae --E 210000 --A 100 --name steel
open-cae node add demo.ocae --id n1 --x -500 --y 0
open-cae node add demo.ocae --id n2 --x 500 --y 0
open-cae node add demo.ocae --id n3 --x 0 --y 500
open-cae element add demo.ocae --id e1 --from n1 --to n3
open-cae element add demo.ocae --id e2 --from n2 --to n3
open-cae support add demo.ocae --node n1 --dx --dy
open-cae support add demo.ocae --node n2 --dx --dy
open-cae load add demo.ocae --node n3 --fy -1000
open-cae solve demo.ocae --out report.json
```

The solution is an axial force of −707.1068 N in each bar and an apex
displacement of −0.0336718 mm.

2D cantilever beam with a tip mass, modal and buckling:

```shell
open-cae create beam.ocae --units mm
open-cae material set beam.ocae --E 210000 --A 100 --density 7.85e-9
open-cae node add beam.ocae --id n1 --x 0 --y 0
open-cae node add beam.ocae --id n2 --x 1000 --y 0
open-cae node mass beam.ocae --id n2 --mass 0.001
open-cae element add beam.ocae --id b1 --from n1 --to n2 --kind beam --I 1000000
open-cae support add beam.ocae --node n1 --dx --dy --rz
open-cae load add beam.ocae --node n2 --fy -1000
open-cae modal beam.ocae --modes 5
open-cae buckling beam.ocae --modes 2
open-cae export beam.ocae --format md --out report.md
```

An elastic support leaves the degree of freedom free and adds stiffness
instead of constraining it:

```shell
open-cae support add beam.ocae --node n2 --ky 5000
```

Tension-only cable: mark the member, then run the active-set solve.

```shell
open-cae element tension-only demo.ocae --id e1
open-cae tension-only demo.ocae --iterations 20 --tolerance 1e-9
```
