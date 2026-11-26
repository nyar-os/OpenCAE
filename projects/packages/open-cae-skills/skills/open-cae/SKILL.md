---
name: open-cae
description: Build and solve truss, beam and spatial frame models with OpenCAE. Use when a task involves an `.ocae` document, nodes and elements, supports, loads, static, modal, buckling, tension-only or steady heat analysis, load combinations, influence lines, response spectra, or SVG/PDF result drawings.
---

# open-cae

## Status

The structural and heat core is executable. The CLI and SDK create, edit and
reopen one `.ocae` document, solve linear static, modal, linear buckling,
tension-only cable and steady-state heat conduction problems, and print a JSON
report for every query. Truss elements are two-force members; `beam` is a 2D
bending member and `beam3d` a spatial six-degree-of-freedom frame member.

Every editing command rewrites the document atomically and raises its revision.
Every solver or report command reads the document and writes nothing unless
`--out` is given. Units are stored on the document and **never converted**:
a document created with `--units mm` reports millimetres, square millimetres,
cubic millimetres and tonnes.

`nonlinear`, `contact` and `import --format step` are not implemented and exit
with code `2`. There is no radiation, no transient heat step and no
auto-generated mesh. The section subcommands are standalone property reports,
not geometry stored in a document.

## Executable commands

```bash
open-cae capabilities
open-cae create <file.ocae> [--units mm|m] [--dimension 2|3]
open-cae validate <file>
open-cae report <file>
open-cae import <input.inp> <output.ocae> [--format abaqus|inp] [--units mm|m]

open-cae node add <file> --id <id> --x <x> --y <y> [--z <z>]
open-cae node update <file> --id <id> [--x <x>] [--y <y>] [--z <z>]
open-cae node mass <file> --id <id> --mass <mass>
open-cae node temperature <file> --id <id> [--temperature <t>] [--clear]
open-cae node heat <file> --id <id> --heat <w>
open-cae node merge <file> --from <id> --into <id>
open-cae node remove <file> --id <id>
open-cae node displacement <file> --id <id>

open-cae element add <file> --id <id> --from <n> --to <n> [--E <E>] [--A <A>] [--kind truss|beam|beam3d] [--I <I>] [--torsion <J>] [--w <w>] [--dt <dT>] [--convection <h>] [--perimeter <P>] [--tension-only]
open-cae element update <file> --id <id> [--from <n>] [--to <n>] [--w <w>] [--clear-load] [--clear-temperature]
open-cae element split <file> --id <id> --at <fraction>
open-cae element release <file> --id <id> [--start] [--end]
open-cae element tension-only <file> --id <id> [--clear]
open-cae element forces <file> --id <id>
open-cae element remove <file> --id <id>

open-cae material set <file> --E <E> --A <A> [--density <rho>] [--alpha <a>] [--conductivity <k>] [--shear-modulus <G>] [--poissons-ratio <nu>] [--section <shape> ...geometry]
open-cae material preset <file> --preset <name> [--section <shape> ...geometry]
open-cae library [--units mm|m]
open-cae section properties --section <shape> [...geometry] [--units mm|m]
open-cae section polygon --points <points> [--units mm|m]
open-cae section principal --ix <Ix> --iy <Iy> --ixy <Ixy> [--units mm|m]

open-cae support add <file> --node <n> [--dx] [--dy] [--dz] [--rz] [--rx] [--ry] [--direction <nx,ny>] [--ux <v>] [--uy <v>] [--kx] [--ky] [--kz] [--kr]
open-cae support remove <file> --node <n>
open-cae load add <file> --node <n> [--fx] [--fy] [--fz] [--mz] [--mx] [--my]
open-cae load remove <file> --node <n>
open-cae model gravity <file> [--gx] [--gy] [--gz]
open-cae model ambient <file> [--temperature <Ta>] [--clear]
open-cae model summary <file> [--out <file>]
open-cae self-weight <file> --g <g> [--axis x|y|z]

open-cae solve <file> [--out <file>] [--analysis static|modal|buckling] [--modes <N>]
open-cae modal <file> [--modes <N>] [--out <file>]
open-cae buckling <file> [--modes <N>] [--out <file>]
open-cae tension-only <file> [--iterations <N>] [--tolerance <t>] [--out <file>]
open-cae heat <file> [--out <file>]

open-cae combination add <file> --name <name> [--loads <f>] [--gravity <f>] [--thermal <f>] [--patterns <list>]
open-cae combination remove <file> --name <name>
open-cae combinations <file> [--out <file>]
open-cae envelope <file> [--out <file>] [--csv <file>]
open-cae buckling-combinations <file> [--modes <N>] [--out <file>]

open-cae reactions <file> [--out <file>]
open-cae reaction <file> --node <n>
open-cae forces <file> [--stations <N>] [--out <file>]
open-cae stresses <file> [--stations <N>] [--out <file>]
open-cae deflections <file> [--stations <N>] [--out <file>]
open-cae nodal-stresses <file> [--stations <N>] [--out <file>]
open-cae load-cases <file> [--out <file>]
open-cae matrices <file> [--out <file>]
open-cae influence <file> --track <element> --quantity reaction|section [--node <node>] [--component <c>] [--position <f>] [--direction x|y] [--points <N>] [--out <file>]
open-cae influence-train <file> --track <element> --quantity reaction|section --axles <P,...> --spacings <d,...> [--out <file>]
open-cae spectrum <file> --spectrum <T:Sa,...> [--modes <N>] [--axis x|y|z] [--combination srss|cqc] [--damping <z>] [--out <file>]

open-cae export <file> --format csv|vtk|md|svg|pdf --out <dir|file> [--scale <N>] [--stations <N>] [--contour <quantity>] [--drawing <kind>]
```

Exit codes: `0` success, `1` invalid input or a failed solve, `2` the
capability is not implemented.

## Preflight

1. Run `open-cae capabilities` and confirm the analysis, element kind or export
   target the task needs is reported as supported. A command name or a file
   extension is not a capability.
2. Run `open-cae report <file>` on an existing document, or `open-cae validate <file>`,
   and confirm the node and element ids the task will address actually exist.
3. If the preflight reports a missing capability, stop and report it. Never
   approximate a nonlinear, contact or transient problem with a linear run.

## Routes

| Route | Intent | Minimum evidence |
|-------|--------|------------------|
| model | Build a document from a described structure | `open-cae validate <file>` and `open-cae report <file>` |
| analyse | Solve and report displacements, forces or reactions | the solver JSON: displacement, axial, shear, moment or reaction |
| review | Check a model before trusting a result | validation issues plus the degrees-of-freedom summary |
| deliver | Write a report, table or drawing | the exported file and the solve report of the same revision |

## Modelling rules

- A `beam` element is 2D and needs a positive `--I` (or a material section);
  `beam3d` needs a `--dimension 3` document and a positive `--torsion <J>`.
  `--inertia-weak` defaults to `--I`.
- End releases: `element release --start`/`--end` frees the bending rotation of
  a `beam`, and all three local rotations of a `beam3d`. Trusses are rejected.
- A truss with `--tension-only` is a cable. Solve it with
  `open-cae tension-only <file> --iterations <n> --tolerance <t>`, not with
  `solve --analysis tension-only`; the `solve` parser accepts that value but
  resolves it to a static solve.
- `--dt` needs a material `alpha`; `--dt-grad` additionally needs a section
  depth. `--convection` and `--perimeter` feed the heat solve only.
- Section shapes assigned to a material are the stored `Section` tags
  (`rectangle`, `circle`, `i`, `box`, `tube`, `channel`). The `i`, `t`, `angle`,
  `z`, `ellipse`, `triangle` and `polygon` subcommands are standalone property
  calculators, not stored geometry.
- Removing a node still referenced by an element is rejected; use
  `node merge` to re-point elements first.

## Analysis

- `solve` is static by default. `--modes` (default `5`) applies to `modal` and
  `buckling` only. `--dimension` must match the document.
- Modal analysis needs a positive density or a positive nodal mass; linear
  buckling needs a loaded, supported model.
- Heat conduction has its own command, `open-cae heat <file> --out <file>`. It
  needs at least one fixed node temperature
  (`open-cae node temperature <file> --id <id> --temperature <t>`) and a positive
  material `--conductivity`. Element flux is
  `q = k·A·(T_from − T_to)/L`, positive from the `from` node to the `to` node.
  Units are never converted, so a `mm` document needs a length-consistent
  conductivity.
- Influence lines and influence trains require an **unloaded** model; they move
  a unit or axle train along one track element and report ordinates.

## Loads and combinations

- `--loads`, `--gravity` and `--thermal` scale the applied case inside a
  combination; defaults are `1`, `0` and `1`.
- Named load patterns are stored by the SDK (`addPatternLoad()`) and are never
  solved alone. The CLI can reference them with
  `open-cae combination add <file> --name <name> --patterns <patterns>` but
  cannot create a pattern entry.
- `open-cae envelope <file> --csv <file>` always includes the unfactored `base`
  case before every combination and reports the governing combination for each
  minimum and maximum.

## Results and exports

- `reactions`, `forces`, `stresses`, `deflections`, `nodal-stresses` and
  `matrices` are read-only JSON reports. `--stations` (default `11`, minimum `2`)
  sets the sampled rows.
- A beam stress report needs a material section; a truss reports zero shear and
  moment.
- `export --format csv|vtk|md|svg|pdf --out <path>`: CSV and VTK take a
  directory, Markdown/HTML/SVG/PDF take a file. `--contour <quantity>` colours an
  SVG/PDF member by `axial`, `shear`, `moment` or `displacement`, and
  `--drawing <kind>` writes a dedicated sheet (`forces`, `shear`, `moment`,
  `displacement`, `reactions`, `overlay`, `modal`, `envelope`, `summary`).
- `--scale` multiplies the deformed shape only. Without it the export picks an
  automatic factor. SVG and PDF output are deterministic; PDF drops non-ASCII
  text.

## Hard rules

- Never claim a solve ran when it did not, and never fabricate a displacement,
  force, reaction or stress.
- Never describe `nonlinear`, `contact` or STEP import as supported; they return
  `capability/not-implemented`.
- Keep one model: a report, table or drawing must come from the document
  revision the solve read.
- Do not hand-write `.ocae` JSON; every edit goes through a CLI or SDK call so
  the document stays valid.

## Safety

- Confirm the target path with the user before any command that creates,
  rewrites or deletes a document; `create` refuses to replace an existing file,
  but every editing command rewrites it in place.
- Never edit an input document unless the task asks for an in-place change.
- State exactly which solver ran, its units and its result; never present an
  estimate as a solved value.
