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
| `open-cae node temperature <file>` | `--id <id> [--temperature <t>] [--clear]` | Fixed temperature for steady heat conduction; `--clear` removes the Dirichlet boundary. Ignored by structural analysis. |
| `open-cae node heat <file>` | `--id <id> --heat <w>` | Concentrated nodal heat flow in watts, positive into the node; `0` clears it. Ignored by structural analysis. |
| `open-cae node remove <file>` | `--id <id>` | Rejected while an element references the node. |
| `open-cae node merge <file>` | `--from <id> --into <id>` | Re-points element endpoints, adds loads and merges supports; `into` keeps its coordinates. |
| `open-cae node displacement <file>` | `--id <id>` | Prints one node's solved displacement. |

### Elements

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae element add <file>` | `--id <id> --from <n> --to <n> [--E <E>] [--A <A>] [--kind truss\|beam] [--I <I>] [--w <w>] [--w2 <w2>] [--dt <dT>] [--dt-grad <dT>] [--w-from <s>] [--w-to <s>] [--convection <h>] [--perimeter <P>] [--tension-only]` | `beam` is 2D-only and needs positive `--I` or a material section. `--w`/`--w2` are local `+y` intensities at the start/end node. `--w-from`/`--w-to` limit the load to a length fraction (both or neither). `--dt` needs material `alpha`; `--dt-grad` also needs a section depth. `--convection` is the surface coefficient `h` in `W/(m²·K)` and `--perimeter` the exposed perimeter `P`; both feed `heat` only and need a model ambient temperature. `--tension-only` marks a truss as a cable. |
| `open-cae element update <file>` | `--id <id> [--from <n>] [--to <n>] [--w <w>] [--w2 <w2>] [--w-from <s>] [--w-to <s>] [--dt <dT>] [--dt-grad <dT>] [--convection <h>] [--perimeter <P>] [--clear-load] [--clear-temperature]` | Updates only the named fields in place: endpoints, distributed load, temperature and convection; material, inertia, releases, tension-only and unnamed values are preserved. `--w` without `--w2` sets a uniform load, `--w2` alone changes the end intensity, and `--w-from`/`--w-to` follow the `element add` rule (both or neither). `--clear-load` removes the load and its span, `--clear-temperature` removes the uniform change and the gradient. Conflicts and an empty request report `session/argument-invalid`. |
| `open-cae element split <file>` | `--id <id> --at <fraction>` | Splits a member at a length fraction strictly between 0 and 1, reusing or creating the interior node. Prints the interior node and both element ids. Uniform loads and temperatures are copied; partial-span/linear loads and gradients are rejected with `element/split-unsupported-load`. |
| `open-cae element remove <file>` | `--id <id>` | Removes one element. |
| `open-cae element forces <file>` | `--id <id>` | Prints the solved result of one element. |
| `open-cae element release <file>` | `--id <id> [--start] [--end]` | Releases the bending moment at one or both beam ends; at least one flag is required. Trusses are rejected. |
| `open-cae element tension-only <file>` | `--id <id> [--clear]` | Marks a truss as tension-only, or clears it with `--clear`. Beams are rejected. |

### Material and sections

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae material set <file>` | `--E <E> --A <A> [--density <rho>] [--name <name>] [--allowable <s>] [--alpha <a>] [--conductivity <k>] [--section <shape> ...geometry]` | Sets the global material; `--alpha` is the thermal expansion coefficient in `1/K`, `--conductivity` the heat-conduction `k` in `W/(m·K)` (required only by `heat`). |
| `open-cae material preset <file>` | `--preset <name> [--section <shape> ...geometry] [--area <A>] [--allowable <s>]` | Presets: `steel`, `stainless-steel`, `aluminum`, `concrete`, `timber`. |
| `open-cae library` | `[--units mm\|m]` | Prints presets and supported section shapes. |
| `open-cae section properties` | `--section <shape> [...geometry] [--units mm\|m]` | Prints area, inertias, torsion constant, extreme fibre and section modulus. |
| `open-cae section polygon` | `--points "x,y;x,y;..." [--units mm\|m]` | Prints area, centroid, `Ix`/`Iy`/`Ixy` about the centroid, polar moment `J = Ix + Iy` and radii of gyration of a closed polygon. Vertices may be clockwise or counter-clockwise and the closing vertex may be repeated. |
| `open-cae section i` | `--height <h> --width <b> --web-thickness <tw> --flange-thickness <tf> [--units mm\|m]` | I-section: area, centroid, `Ix`/`Iy`/`Ixy`, torsion constant `J` and radii of gyration. Requires `0 < tf < h/2` and `0 < tw < b`; invalid dimensions report `section/invalid-dimensions`. |
| `open-cae section tube` | `--height <h> --width <b> --thickness <t> [--units mm\|m]` | Hollow **rectangular** tube: area, centroid, `Ix`/`Iy`/`Ixy`, torsion constant `J` and radii of gyration. Requires `0 < t < min(b, h)/2`. |
| `open-cae section channel` | `--height <h> --width <b> --web-thickness <tw> --flange-thickness <tf> [--units mm\|m]` | Open channel with the opening facing right: area, centroid, `Ix`/`Iy`/`Ixy`, torsion constant `J` and radii of gyration. The centroid `x` is measured from the web face and is not `b/2`. |
| `open-cae section angle` | `--height <h> --width <w> --thickness <t> [--units mm\|m]` | Equal-thickness L (angle): area, centroid, `Ix`/`Iy`/`Ixy`, torsion constant `J` and radii of gyration. The outer corner is the origin, the `t × t` corner is counted once, and `Ixy` is generally non-zero. Requires `0 < t < min(h, w)`. |
| `open-cae section t` | `--height <h> --width <b> --web-thickness <tw> --flange-thickness <tf> [--units mm\|m]` | T-section with the flange at the top: area, centroid, `Ix`/`Iy`/`Ixy = 0`, torsion constant `J` and radii of gyration. The centroid `y` is measured from the bottom edge. Requires `0 < tf < h` and `0 < tw < b`. |
| `open-cae section ellipse` | `--width <w> --height <h> [--units mm\|m]` | Solid ellipse with semi-axes `a = w/2` and `b = h/2`: area `πab`, centroid `(a, b)`, `Ix = πab³/4`, `Iy = πa³b/4`, `Ixy = 0`, exact torsion constant `J = πa³b³/(a² + b²)` and radii of gyration `b/2`, `a/2`. Requires positive, finite `w` and `h`; invalid dimensions report `section/invalid-dimensions`. |
| `open-cae section triangle` | `--side <a> [--units mm\|m]` | Solid equilateral triangle with side `a`, base along the bottom edge and apex at the top centre: area `√3/4·a²`, centroid `(a/2, a·√3/6)`, `Ix = Iy = √3·a⁴/96`, `Ixy = 0`, exact torsion constant `J = √3·a⁴/80` and radii of gyration `a/√24`. Requires a positive, finite `a`; invalid dimensions report `section/invalid-dimensions`. |
| `open-cae section z` | `--height <h> --width <w> --thickness <t> [--units mm\|m]` | Equal-thickness Z-section: area, centroid `(w/2, h/2)`, `Ix`/`Iy`/`Ixy`, torsion constant `J` and radii of gyration. The origin is the lower-left corner of the bounding rectangle, the web is centred (`w/2 − t/2 … w/2 + t/2`), the top flange reaches the right edge and the bottom flange the left edge, so `Ixy` is generally non-zero. `A = t(w + h − t)` and `J = (w + h − t)·t³/3`. Requires `0 < t < w` and `2t < h`; invalid dimensions report `section/invalid-dimensions`. |
| `open-cae section principal` | `--ix <Ix> --iy <Iy> --ixy <Ixy> [--units mm\|m]` | Principal second moments of a centroidal moment matrix: `I1` (major), `I2` (minor), polar moment `Ix + Iy` and the counter-clockwise major-axis angle from `+x` in degrees, normalised to `(-90, 90]`. Echoes `ix`, `iy` and `ixy`; `--units` labels the unit system and does not scale the values. Requires positive `ix`/`iy` and `ix·iy ≥ ixy²` (within `1e-12`); violations report `section/invalid-second-moments`. |

`section i`, `section tube`, `section channel`, `section angle`, `section t`,
`section ellipse`, `section triangle` and `section z` print the same field names
as `section polygon` (`area`, `centroidX`, `centroidY`, `ix`, `iy`, `ixy`,
`polarMoment`, `radiusOfGyrationX`, `radiusOfGyrationY`), but `polarMoment`
holds the Saint-Venant torsion constant `J`, not `Ix + Iy`. `section principal`
prints `units`, the echoed `ix`, `iy`, `ixy`, the principal moments `i1` and
`i2`, the polar moment `polarMoment = Ix + Iy` and `principalAngle` in degrees.
The coordinates
place the origin at the lower-left corner of the bounding rectangle. Note that
`section tube` is the rectangular tube, whereas `--section tube` in
`section properties` and `material preset` remains the circular hollow section
with `--outer-diameter`/`--inner-diameter`. The L, T, ellipse, triangle and Z
calculators are standalone properties reports: they are not `Section` tags
stored in documents.

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
| `open-cae model ambient <file>` | `[--temperature <Ta>] [--clear]` | Sets the ambient temperature used by surface convection in `heat`; `--clear` removes it. Ignored by structural analysis. |
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
| `open-cae heat <file>` | `[--out <file>]` | Steady-state axial heat conduction. Requires at least one fixed node temperature and a positive material `conductivity`; prints temperatures, element fluxes and fixed-temperature reactions. Optional member `convection`/`perimeter` and `model ambient` add surface convection. `thermal` is an alias. No radiation or transient stepping. |

Use the dedicated `tension-only` command for cables. The `solve` parser also
accepts `--analysis tension-only`, but currently resolves it to a static
solve, and it rejects `--iterations`/`--tolerance` outside the dedicated
command. Heat conduction has its own `heat` command rather than an
`--analysis` value.

A heat document has one scalar temperature per node. Fixed temperatures are
Dirichlet boundaries, other nodes are free; a missing fixed temperature fails
with `heat/no-fixed-temperature` and a missing or non-positive conductivity
with `material/invalid-conductivity`. Element flux is
`q = k·A·(T_from − T_to)/L`, positive from the `from` node to the `to` node.
Units are never converted, so a `mm` document must supply a length-consistent
conductivity.

An element that declares `convection` `h` and `perimeter` `P` exchanges heat
with the model `ambientTemperature` `Ta`: its matrix gains
`h·P·L/6·[[2, 1], [1, 2]]` and its load vector `h·P·L·Ta/2·[1, 1]`. The element
report gains `convection: {coefficient, perimeter, ambientTemperature, heat}`
with `heat = h·P·L·(T_avg − Ta)`, positive out of the member and omitted when no
convection is declared. Fixed reactions balance the convective loss:
`Σreaction + Σapplied heat = Σconvective heat`. Missing `--perimeter` or a
missing ambient reports `heat/missing-perimeter` or `heat/missing-ambient`; a
negative convection coefficient or a non-positive perimeter reports
`heat/invalid-convection` or `heat/invalid-perimeter`.

### Results

| Command | Flags | Notes |
|---------|-------|-------|
| `open-cae reactions <file>` | `[--out <file>]` | Per-support reactions and the global equilibrium check. |
| `open-cae reaction <file>` | `--node <n>` | One support reaction. |
| `open-cae forces <file>` | `[--stations <N>] [--out <file>]` | Sampled axial force, shear and bending moment; `--stations` defaults to 11 and must be at least 2. |
| `open-cae influence <file>` | `--track <element> --quantity reaction\|section [--node <node>] [--component <c>] [--element <id>] [--position <f>] [--direction x\|y] [--points <N>] [--out <file>]` | Influence-line ordinates as a unit load moves along the track beam; the model must be unloaded. `--quantity reaction` needs `--node` and `--component fx\|fy\|mz`; `--quantity section` needs `--position` and `--component axial\|shear\|moment` (only `axial` for truss section elements); `--direction` defaults to `y` and `--points` to 11. |
| `open-cae stresses <file>` | `[--stations <N>] [--out <file>]` | Extreme-fibre stresses, governing station and utilisation. A beam needs a material section. |
| `open-cae deflections <file>` | `[--stations <N>] [--out <file>]` | Local deflection/rotation and deformed coordinates. |
| `open-cae nodal-stresses <file>` | `[--stations <N>] [--out <file>]` | Averaged nodal stress and extremes. |
| `open-cae load-cases <file>` | `[--out <file>]` | One summary row for `base` and each combination. |
| `open-cae matrices <file>` | `[--out <file>]` | Assembled global stiffness and lumped mass matrices with row labels. |
| `open-cae spectrum <file>` | `[--modes <N>] [--axis x\|y\|z] [--combination srss\|cqc] [--damping <z>] --spectrum <T:Sa,...> [--out <file>]` | Response spectrum from `period:acceleration` points; `--axis` defaults to `x` and `--combination` to `srss`. `--damping` is the single modal damping ratio in `(0, 1]`, required by `cqc` and ignored by `srss`. |
| `open-cae export <file>` | `--format csv\|vtk\|md\|svg --out <dir\|file> [--scale N]` | CSV writes `nodes.csv`, `elements.csv` and `reactions.csv`; VTK is legacy ASCII. `--out` is a directory for CSV/VTK and a file for Markdown, HTML and SVG. `--scale` (default `1`) multiplies the nodal displacements of the SVG deformed shape. SVG is deterministic, projects onto the global X–Y plane and carries a title and legend. HTML export is available through the SDK/API, not this CLI. |

### Rejected commands

`open-cae import`, `nonlinear`, `frame`, `frame3d` and `contact`
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

Steady heat conduction through one bar in the metre system:

```shell
open-cae create heat.ocae --units m
open-cae material set heat.ocae --E 210e9 --A 1e-4 --conductivity 50
open-cae node add heat.ocae --id n1 --x 0 --y 0
open-cae node add heat.ocae --id n2 --x 1 --y 0
open-cae element add heat.ocae --id e1 --from n1 --to n2
open-cae node temperature heat.ocae --id n1 --temperature 100
open-cae node temperature heat.ocae --id n2 --temperature 0
open-cae heat heat.ocae --out temperature.json
```

The bar carries `q = kAΔT/L = 50 × 1e-4 × 100 = 0.5 W`; the fixed nodes
report `+0.5 W` and `−0.5 W`. Clear a boundary with
`open-cae node temperature heat.ocae --id n2 --clear`.

Surface convection on a bar with one fixed end and one free end:

```shell
open-cae create fin.ocae --units m
open-cae material set fin.ocae --E 210e9 --A 1e-2 --conductivity 50
open-cae node add fin.ocae --id n1 --x 0 --y 0
open-cae node add fin.ocae --id n2 --x 1 --y 0
open-cae element add fin.ocae --id e1 --from n1 --to n2 --convection 2 --perimeter 0.1
open-cae model ambient fin.ocae --temperature 20
open-cae node temperature fin.ocae --id n1 --temperature 100
open-cae heat fin.ocae --out convection.json
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
