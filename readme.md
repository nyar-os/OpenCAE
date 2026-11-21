# OpenCAE

OpenCAE is a **narrowly scoped and verifiable** linear-elastic analysis tool for 2D/3D trusses and 2D Euler-Bernoulli beams, supporting linear static, self-weight, prescribed displacements, linearly varying distributed loads, internal force diagrams, beam stress recovery, support reaction summaries with a force-equilibrium check, linear elastic spring supports, load combinations, modal (with effective mass) and linear buckling analysis (single load case and per load combination), plus a built-in material and parametric-section library. It targets:

- trusses, space frames and bracing systems that carry axial force only;
- bending problems such as planar beams, cantilever beams and simply supported beams;
- teaching, algorithm validation and small regression models in CI;
- workflows that store models as plain-text JSON and reproduce results from the command line.

It is **not** a general-purpose commercial finite element package: 3D beams/frames, shell/solid elements, nonlinear analysis, steady-state/transient heat transfer, temperature gradients and contact are not implemented. Every unimplemented capability fails explicitly (`capability/not-implemented`, exit code `2`) instead of degrading silently. Modal analysis uses a lumped mass matrix, so frequencies and effective masses are finite element approximations; the readme and tests state the error bounds against analytical solutions.

## Architecture: the API is the core and the CLI only parses arguments

The `OpenCae` session API in `projects/crates/cae-model` is the only implementation: document I/O, editing, validation, solving and CSV/VTK/Markdown/HTML export all live there. The three entry points share the same logic:

| Entry point | Location | Responsibility |
|------|------|------|
| Rust session API | `projects/crates/cae-model/src/session.rs` | all model logic; `OpenCaeOptions`, `OpenCae` |
| Node-API orthogonal functions | `projects/crates/cae-napi` | each export does one thing and returns a JSON string |
| TypeScript SDK | `projects/packages/open-cae` | `new OpenCAE()` session, `CaeDocument` documents, thin `open-cae` CLI |

The CLI only parses arguments, calls the API, prints results and maps exit codes; it duplicates no model logic. Validation, solution and export results are therefore always consistent across the SDK, CLI and native functions.

## TypeScript SDK

The SDK is the recommended programmatic entry point. `new OpenCAE()` opens no file and performs no network access when constructed:

```ts
import { OpenCAE } from '@wae/open-cae';

const cae = new OpenCAE({ locale: 'en-US', units: 'mm', dimension: 2 });
const document = await cae.create('demo.ocae');

document.addNode({ id: 'n1', x: -500, y: 0 });
document.addNode({ id: 'n2', x: 500, y: 0 });
document.addNode({ id: 'n3', x: 0, y: 500 });
document.addElement({ id: 'e1', from: 'n1', to: 'n3' });
document.addElement({ id: 'e2', from: 'n2', to: 'n3' });
document.setMaterial({ youngsModulus: 210000, area: 100, name: 'steel' });
document.addSupport({ node: 'n1', dx: true, dy: true });
document.addSupport({ node: 'n2', dx: true, dy: true });
document.addLoad({ node: 'n3', fy: -1000 });

const validation = document.validate(); // { ok, issues }
const solution = document.solve();       // displacements, reactions, element axial force/stress/strain
document.export('csv', 'out/csv');       // out/csv/nodes.csv, elements.csv, reactions.csv
const reactions = document.reactions();  // support reactions + equilibrium check
const report = document.report();        // document summary + issues + solution
document.save();                         // atomically writes back to the original file
await cae.dispose();
```

2D beam + modal example (beam along the X axis, fixed support constraining `rz`):

```ts
document.setMaterial({ youngsModulus: 210000, area: 100, density: 7.85e-9, name: 'steel' });
document.addElement({ id: 'b1', from: 'n1', to: 'n2', kind: 'beam', inertia: 1e6 });
document.addSupport({ node: 'n1', dx: true, dy: true, rz: true });
document.setGravity({ gy: -9810 });                 // optional: self-weight
const modal = document.solve({ analysis: 'modal', modes: 5 });
document.export('md', 'out/report.md');              // deterministic Markdown report
document.export('html', 'out/report.html');          // self-contained HTML report

document.addCombination({ name: 'D+L', loads: 1, gravity: 1 });
const combinations = document.solveCombinations();   // solutions + envelope
const buckling = document.solveBucklingCombinations(); // factors per combination + governing case
const forces = document.internalForces({ stations: 11 }); // N/V/M diagrams
const stresses = document.stresses({ stations: 11 });     // sigma = N/A +/- M/S, utilization
const deflections = document.deflections({ stations: 11 }); // local deflection/rotation + deformed coordinates
```

- Sessions: `capabilities()`, `library(units?)`, `create(path, options?)`, `open(path)`, `dispose()`.
- Documents: `id/units/dimension/summary`, plus `addNode/addElement/setMaterial/applyMaterialPreset/addSupport/addLoad/setGravity/addCombination/removeCombination/validate/solve/solveCombinations/solveBucklingCombinations/reactions/internalForces/stresses/deflections/export/report/save/close`; every edit operation writes atomically to the file.
- `setMaterial()` and `applyMaterialPreset()` accept an optional parametric `section` (`rectangle`, `circle` or `i`), so beams may omit `inertia` and stress recovery can use the extreme-fibre distance. `allowableStress` enables utilisation checks.
- `addSupport()` also accepts `ux`, `uy`, `uz` and `rotation` to enforce a non-zero displacement (support settlement or driven displacement); supplying one constrains the matching degree of freedom.
- `addSupport()` also accepts `kx`, `ky`, `kz` and `kr` for linear elastic spring stiffness. A spring leaves the matching degree of freedom free and adds its stiffness to the global matrix, so supports can be rigid, elastic or both on different components of the same node; the reaction summary reports the spring force `−k·u`. Spring and rigid constraint on the same component are rejected by validation (`support/spring-constrained`).
- `solve()` defaults to `analysis: 'static'`; `solve({ analysis: 'modal', modes })` returns a `ModalSolution` and `solve({ analysis: 'buckling', modes })` returns a `BucklingSolution` whose factors multiply the static load case.
- Errors carry stable `code`s (such as `model/mechanism`, `session/no-document`, `capability/not-implemented`) and can be detected with `isOpenCAEError`; the exit-code convention is handled by the CLI.

Build the SDK and native addon:

```shell
pnpm install
pnpm run build          # wae build: native → ts → staging, producing dist/*.js and platform packages
pnpm run build:ts       # tsc only, producing @wae/open-cae/dist
pnpm run typecheck
```

The native addon is provided by the `@wae/open-cae-<platform>-<arch>` platform packages: `wae native build` stages the `open-cae-napi` cdylib as `open-cae.<platform>-<arch>.node`, and the SDK resolves it automatically from `process.platform`/`process.arch`, with no global tooling or path configuration. When the matching platform package is not installed, SDK calls throw `capability/not-implemented` rather than fabricating results; only when debugging a self-built binary do you need to point `OPEN_CAE_NATIVE` at a `.node` file.

## Quick start (CLI)

The commands below are thin wrappers around the SDK, convenient for reproducing results in CI and scripts. Run them from the repository root (offline dependencies are already cached):

```shell
cargo run -p cae-model --bin ocae --offline -- create demo.ocae --units mm
cargo run -p cae-model --bin ocae --offline -- material set demo.ocae --E 210000 --A 100 --name steel
cargo run -p cae-model --bin ocae --offline -- material preset demo.ocae --preset steel --section rectangle --width 10 --height 20
cargo run -p cae-model --bin ocae --offline -- library --units mm

cargo run -p cae-model --bin ocae --offline -- node add demo.ocae --id n1 --x -500 --y 0
cargo run -p cae-model --bin ocae --offline -- node add demo.ocae --id n2 --x  500 --y 0
cargo run -p cae-model --bin ocae --offline -- node add demo.ocae --id n3 --x    0 --y 500

cargo run -p cae-model --bin ocae --offline -- element add demo.ocae --id e1 --from n1 --to n3
cargo run -p cae-model --bin ocae --offline -- element add demo.ocae --id e2 --from n2 --to n3

cargo run -p cae-model --bin ocae --offline -- support add demo.ocae --node n1 --dx --dy
cargo run -p cae-model --bin ocae --offline -- support add demo.ocae --node n2 --dx --dy
cargo run -p cae-model --bin ocae --offline -- load add demo.ocae --node n3 --fy -1000

cargo run -p cae-model --bin ocae --offline -- validate demo.ocae
cargo run -p cae-model --bin ocae --offline -- solve demo.ocae --out report.json
cargo run -p cae-model --bin ocae --offline -- export demo.ocae --format csv --out out/csv
cargo run -p cae-model --bin ocae --offline -- export demo.ocae --format vtk --out out/result.vtk
```

The analytical solution for this case (span 1000 mm, height 500 mm, vertical apex load −1000 N) is: axial force −707.1068 N (compression) in each bar, and vertical apex displacement −0.0336718 mm.

A 3D model only needs `--dimension 3` on `create`, with `--z` added to nodes:

```shell
cargo run -p cae-model --bin ocae --offline -- create vertical.ocae --units mm --dimension 3
cargo run -p cae-model --bin ocae --offline -- node add vertical.ocae --id n1 --x 0 --y 0 --z 0
cargo run -p cae-model --bin ocae --offline -- node add vertical.ocae --id n2 --x 0 --y 0 --z 1000
cargo run -p cae-model --bin ocae --offline -- element add vertical.ocae --id e1 --from n1 --to n2
cargo run -p cae-model --bin ocae --offline -- support add vertical.ocae --node n1 --dx --dy --dz
cargo run -p cae-model --bin ocae --offline -- support add vertical.ocae --node n2 --dx --dy
cargo run -p cae-model --bin ocae --offline -- load add vertical.ocae --node n2 --fz -1000
cargo run -p cae-model --bin ocae --offline -- solve vertical.ocae --dimension 3
```

The analytical solution for this 3D single bar is: axial displacement `u_z = F·L/(E·A) = −0.0476190 mm` and stress `σ = F/A = −10 MPa`.

2D cantilever beam, self-weight and modal analysis:

```shell
cargo run -p cae-model --bin ocae --offline -- create beam.ocae --units mm
cargo run -p cae-model --bin ocae --offline -- material set beam.ocae --E 210000 --A 100 --density 7.85e-9
cargo run -p cae-model --bin ocae --offline -- node add beam.ocae --id n1 --x 0 --y 0
cargo run -p cae-model --bin ocae --offline -- node add beam.ocae --id n2 --x 1000 --y 0
cargo run -p cae-model --bin ocae --offline -- element add beam.ocae --id b1 --from n1 --to n2 --kind beam --I 1000000
cargo run -p cae-model --bin ocae --offline -- support add beam.ocae --node n1 --dx --dy --rz
cargo run -p cae-model --bin ocae --offline -- load add beam.ocae --node n2 --fy -1000
cargo run -p cae-model --bin ocae --offline -- model gravity beam.ocae --gy -9810
cargo run -p cae-model --bin ocae --offline -- solve beam.ocae --analysis modal --modes 5
cargo run -p cae-model --bin ocae --offline -- buckling beam.ocae --modes 2
cargo run -p cae-model --bin ocae --offline -- combination add beam.ocae --name P --loads 1
cargo run -p cae-model --bin ocae --offline -- buckling-combinations beam.ocae --modes 2
cargo run -p cae-model --bin ocae --offline -- export beam.ocae --format md --out out/report.md
```

`--kind beam` requires `--I`; `--w` is the distributed transverse load in the element-local `+y` direction at the start node (force/length, positive upwards) and `--w2` optionally sets the intensity at the end node. A uniform load (`--w` only, or `--w2` equal to it) is converted to the consistent nodal vector `[0, wL/2, wL²/12, 0, wL/2, −wL²/12]`; `--w2` closes a linearly varying (trapezoidal) load instead. The nodal moment `--mz` is positive counter-clockwise. `self-weight --g -9810 --axis y` is equivalent to `model gravity --gy -9810`.

Prescribed displacements (support settlements or driven degrees of freedom) use the same command; supplying `--ux/--uy/--uz/--rotation` also constrains that degree of freedom:

```shell
cargo run -p cae-model --bin ocae --offline -- support add beam.ocae --node n2 --uy -2
cargo run -p cae-model --bin ocae --offline -- solve beam.ocae   # roller settles 2 mm
```

For a propped cantilever with a 2 mm roller settlement the roller reaction is `3EIδ/L³` downwards, the fixed-end reaction is the opposite force and the fixed-end moment is `3EIδ/L²`; the value is asserted to relative error `1e-12` in the tests.

Elastic supports use the same command without a constraint flag:

```shell
cargo run -p cae-model --bin ocae --offline -- support add beam.ocae --node n2 --ky 5000
```

For a cantilever with a vertical tip spring `k` the tip deflection is `P/(3EI/L³ + k)` and the fixed-end reaction is `P − kδ`; the tests assert the axial bar `P/(EA/L + k)`, the tip-spring cantilever and a root rotational spring to relative error `1e-12`.

## CLI commands

| Command | Description |
|------|------|
| `ocae create <file.ocae> [--units mm\|m] [--dimension 2\|3]` | Create a document; the default dimension is 2 and consistent steel defaults are written (millimetre system E=210000 MPa, A=100 mm²; metre system E=2.1e11 Pa, A=1e-4 m²) |
| `ocae node add <file> --id <id> --x <x> --y <y> [--z <z>]` | Add a node; duplicate ids are rejected; `--z` defaults to 0 and may be non-zero only in 3D documents |
| `ocae element add <file> --id <id> --from <n> --to <n> [--E <E>] [--A <A>] [--kind truss\|beam] [--I <I>] [--w <w>] [--w2 <w2>] [--dt <dT>]` | Add a truss/beam element; `beam` is 2D-only and requires a positive `--I`; `--w`/`--w2` are the local `+y` transverse intensities at the start/end node (uniform when `--w2` is omitted); `--dt` is a uniform temperature change and requires a material `alpha` |
| `ocae element release <file> --id <id> [--start] [--end]` | Release the bending moment at the start and/or end of a beam (hinge); truss elements are rejected |
| `ocae material set <file> --E <E> --A <A> [--density <rho>] [--name <name>] [--allowable <s>] [--alpha <a>] [--section ...]` | Set the global material, section and density (modal/self-weight analysis requires `--density > 0`); `--alpha` is the coefficient of thermal expansion in `1/K` |
| `ocae material preset <file> --preset <name> [--section rectangle\|circle\|i] [geometry] [--area <A>] [--allowable <s>]` | Apply a built-in material preset (`steel`, `stainless-steel`, `aluminum`, `concrete`, `timber`); a section supplies the area and beam inertia when omitted |
| `ocae library [--units mm\|m]` | Print the built-in material presets and supported section shapes as JSON |
| `ocae support add <file> --node <n> [--dx] [--dy] [--dz] [--rz] [--ux <v>] [--uy <v>] [--uz <v>] [--rotation <v>] [--kx <k>] [--ky <k>] [--kz <k>] [--kr <k>]` | Add/merge support constraints or spring stiffness; `--rz`/`--kr` are available only on beam nodes and `--dz`/`--kz` only in 3D documents. A supplied displacement clamps that degree of freedom to the given value and implies its flag; a supplied spring leaves it free |
| `ocae load add <file> --node <n> [--fx <fx>] [--fy <fy>] [--fz <fz>] [--mz <mz>]` | Add/update a nodal concentrated force/moment; `--fz` is 3D-only and `--mz` is beam-node-only |
| `ocae model gravity <file> [--gx <gx>] [--gy <gy>] [--gz <gz>]` | Set the gravity acceleration; the zero vector clears it |
| `ocae self-weight <file> --g <g> [--axis x\|y\|z]` | Set the gravity acceleration along the given axis |
| `ocae combination add <file> --name <name> [--loads <f>] [--gravity <f>]` | Add or replace a linear load combination; `--loads` (default 1) scales nodal/element loads and `--gravity` (default 0) scales self-weight |
| `ocae combination remove <file> --name <name>` | Remove a load combination |
| `ocae combinations <file> [--out report.json]` | Solve every stored combination and print the individual solutions plus the envelope |
| `ocae validate <file>` | Print JSON validation results; exit code 1 when errors exist |
| `ocae solve <file> [--out report.json] [--analysis static\|modal\|buckling] [--modes N] [--dimension 2\|3]` | Run a static, modal or linear buckling solve and print JSON; `--modes` is modal/buckling-only and defaults to 5 |
| `ocae modal <file> [--modes N] [--out report.json]` | Shortcut for a modal solve |
| `ocae buckling <file> [--modes N] [--out report.json]` | Shortcut for a linear buckling solve |
| `ocae buckling-combinations <file> [--modes N] [--out report.json]` | Solve the linear buckling eigenproblem for every stored load combination and print the factors plus the governing combination |
| `ocae forces <file> [--stations N] [--out report.json]` | Print sampled axial force, shear and bending moment per element; `--stations` counts both ends and defaults to 11 |
| `ocae stresses <file> [--stations N] [--out report.json]` | Recover extreme-fibre stresses, governing station and utilisation; a beam model needs a material section |
| `ocae deflections <file> [--stations N] [--out report.json]` | Sample the deflected centreline (`axial`, `transverse`, `rotation`, deformed `x/y/z`) per element; the Hermite interpolation is completed by the exact particular solution of the member load |
| `ocae export <file> --format csv --out <dir>` | Write `<dir>/nodes.csv`, `<dir>/elements.csv` and `<dir>/reactions.csv`; node columns include `x,y,z` and `ux,uy,uz` |
| `ocae reactions <file> [--out report.json]` | Print per-support reactions and the global `reaction + applied` equilibrium check |
| `ocae export <file> --format vtk --out <dir\|file.vtk>` | Write legacy ASCII VTK; points are 3D coordinates and displacements are 3D vectors |
| `ocae export <file> --format md --out <dir\|file.md>` | Write a deterministic Markdown report; appends the first 5 modes when density is positive |
| `ocae export <file> --format html --out <dir\|file.html>` | Write a deterministic, self-contained HTML report with embedded CSS and the same sections as the Markdown report |
| `ocae report <file>` | Solve and print a JSON report (document summary, validation issues, solution and optional modal) |
| `ocae capabilities` | Print the capability matrix as JSON |

Exit codes: `0` success, `1` input/validation/IO error, `2` capability not implemented. `pnpm exec open-cae ...` matches `ocae ...` in commands, arguments and exit codes.

`solve` accepts `--analysis static|modal|buckling`, `--modes N`, `--element truss|beam` and `--dimension 2|3`. `--dimension` does not override the document setting: when it differs from the document `dimension`, `cli/dimension-mismatch` is returned (exit code 1). `--analysis nonlinear/thermal`, `--element frame`, and running `ocae import`, `ocae nonlinear`, `ocae frame`, `ocae thermal` or `ocae contact` still return `capability/not-implemented` with exit code `2`. Linear buckling supports two-dimensional truss and beam models and three-dimensional truss models.

## `.ocae` document format

The file is UTF-8 JSON with camelCase fields:

```json
{
  "format": "open-cae-truss",
  "version": 1,
  "units": "mm",
  "dimension": 2,
  "material": {
    "name": "steel",
    "youngsModulus": 210000.0,
    "area": 200.0,
    "density": 7.85e-9,
    "allowableStress": 235.0,
    "alpha": 1.2e-5,
    "section": { "shape": "rectangle", "width": 10.0, "height": 20.0 }
  },
  "nodes": [
    { "id": "n1", "x": 0.0, "y": 0.0, "z": 0.0 },
    { "id": "n2", "x": 1000.0, "y": 0.0, "z": 0.0 }
  ],
  "elements": [
    { "id": "e1", "from": "n1", "to": "n2", "kind": "truss" },
    { "id": "b1", "from": "n1", "to": "n2", "kind": "beam", "inertia": 1000000.0, "distributedLoad": -2.0, "temperature": 50.0, "release": { "start": false, "end": true } }
  ],
  "supports": [
    { "node": "n1", "dx": true, "dy": true, "dz": false, "rz": true },
    { "node": "n2", "spring": { "kx": 0.0, "ky": 5000.0, "kz": 0.0, "kr": 0.0 } }
  ],
  "loads": [
    { "node": "n2", "fx": 0.0, "fy": -1000.0, "fz": 0.0, "mz": 0.0 }
  ],
  "gravity": [0.0, -9810.0, 0.0],
  "combinations": [
    { "name": "D+L", "loads": 1.0, "gravity": 1.0 }
  ],
  "revision": 0
}
```

| Field | Description |
|------|------|
| `format` / `version` | Fixed to `open-cae-truss` / `1`; a mismatch fails validation |
| `units` | `"mm"` or `"m"`. When missing, `mm` is assumed and a `document/assumed-units` notice is produced |
| `dimension` | Spatial dimension, `2` or `3`. When missing, `2` is assumed so old documents load without changes; other values raise `document/dimension` |
| `material` | Global material: `youngsModulus` (E), `area` (A), `density` (default 0), optional `name`, `allowableStress` (for utilisation), `alpha` (coefficient of thermal expansion in `1/K`, required by temperature loads) and `section` (parametric geometry). A section is `{ "shape": "rectangle", "width", "height" }`, `{ "shape": "circle", "diameter" }` or `{ "shape": "i", "depth", "flangeWidth", "flangeThickness", "webThickness" }`; when present it supplies the strong-axis inertia for beams that omit `inertia` |
| `nodes` | Node ids must be unique; coordinates are `x`, `y`, `z`, with `z` defaulting to 0 |
| `elements` | `kind` is `truss` (default) or `beam`; `from`/`to` reference nodes; optional `material: { "youngsModulus": ..., "area": ... }` overrides E/A; `beam` requires a positive `inertia` or a material `section` and accepts optional `distributedLoad`/`distributedLoadEnd` intensities in the local `+y` direction (equal or omitted end value means uniform, otherwise linearly varying), an optional uniform `temperature` change and an optional moment `release: { "start": bool, "end": bool }` (beams only) |
| `supports` | `dx`/`dy`/`dz` constrain X/Y/Z translation when `true`; `rz` constrains rotation about Z and applies only at beam nodes; all default to `false`. Optional `prescribed: { ux, uy, uz, rz }` enforces non-zero values of the constrained degrees of freedom (omitted components are `0`), so old documents load unchanged. Optional `spring: { kx, ky, kz, kr }` adds linear elastic stiffness (force/length, moment/radian) and leaves the matching degree of freedom free; `kz` is 3D-only and `kr` is 2D beam-node-only |
| `loads` | Nodal concentrated forces `fx`/`fy`/`fz` and moment `mz`, defaulting to 0; `mz` applies only at beam nodes |
| `gravity` | Optional `[gx, gy, gz]`; when set, self-weight is applied from the density |
| `combinations` | Optional list of `{ name, loads, gravity }` linear combinations; `loads` multiplies nodal and element applied loads (default 1), `gravity` multiplies self-weight (default 0). Names must be unique and factors finite |
| `revision` | Edit counter incremented by CLI modifications; may be 0 |

**Dimension consistency**: if a `dimension: 2` document contains a non-zero `z`, `dz: true`, a non-zero `fz` or a non-zero `gz`, validation returns a `document/dimension-mismatch` error (edit commands also reject it before writing) instead of silently ignoring those values. 3D documents do not allow `beam` elements, `rz: true` or a non-zero `mz` (returning `capability/not-implemented` or `document/dimension-mismatch`). `rz`/`mz` may only be used at nodes with at least one continuous (non-released) beam end; otherwise `support/inactive-dof` / `load/inactive-dof` is reported. A `prescribed` value must be finite and its matching flag must be `true`, otherwise `support/invalid-prescribed` or `support/prescribed-unconstrained` is reported; a non-zero `prescribed.uz` in 2D or `prescribed.rz` in 3D is a `document/dimension-mismatch`. A `spring` component must be finite and non-negative, a zero spring is `support/invalid-spring`, and a spring plus a rigid flag on the same component is `support/spring-constrained`; `kz` in 2D, `kr` in 3D and `kr` at a truss-only node are rejected like their rigid counterparts.

**Degree-of-freedom activation**: a 2D document that contains any beam element uses 3 degrees of freedom per node (`ux, uy, rz`); however, `rz` is active only at beam ends that are **not released**, so purely truss parts and hinged ends never produce zero-stiffness rotation rows. Pure truss documents still use 2 degrees of freedom per node. 3D trusses keep 3 translational degrees of freedom per node.

**Unit convention**: the solver performs no unit conversion and only requires numerically consistent input. With `units: "mm"`, use N, mm, mm² and MPa, with density in t/mm³ (steel is about `7.85e-9`); with `units: "m"`, use N, m, m² and Pa, with density in kg/m³. Mixing units is not an error, but the results have no physical meaning. Modal frequencies are in Hz.

## Capability matrix

| Capability | Status |
|------|------|
| 2D truss linear static (`truss2d`) | ✅ supported |
| 3D truss linear static (`truss3d`) | ✅ supported |
| 2D Euler-Bernoulli beam (`beam2d`, including axial force/shear/bending moment, distributed loads, end moments, end releases) | ✅ supported |
| beam end moment releases (`endRelease`, static condensation of hinges) | ✅ supported |
| self-weight (`selfWeight`, lumped truss distribution + consistent beam loads) | ✅ supported |
| uniform member temperature change (`thermalExpansion`, `α·ΔT` thermal strain + equivalent nodal loads) | ✅ supported |
| prescribed displacements (`prescribedDisplacement`, support settlements) | ✅ supported |
| modal analysis (`modal`, lumped mass + Jacobi, effective modal mass) | ✅ supported |
| linear buckling (`buckling`, 2D truss/beam and 3D truss geometric stiffness + Jacobi) | ✅ supported |
| buckling under load combinations (`bucklingCombinations`, scaled cases + governing envelope) | ✅ supported |
| internal force diagrams (`internalForces`, sampled N/V/M per element) | ✅ supported |
| beam stress recovery and utilisation (`stresses`) | ✅ supported |
| deflected-shape sampling (`deflections`, local deflection/rotation and deformed coordinates) | ✅ supported |
| support reaction summary and equilibrium check (`reactions`) | ✅ supported |
| linear elastic spring supports (`elasticSupports`, translational and rotational stiffness) | ✅ supported |
| load combinations (`loadCombinations`, linear factors + envelope) | ✅ supported |
| material presets and section library (`materialLibrary`, rectangle/circle/I-section properties) | ✅ supported |
| `.ocae` JSON I/O | ✅ supported |
| CSV / legacy VTK / Markdown / HTML report export | ✅ supported |
| 3D beam (`beam3d`) | ❌ not implemented |
| 3D frame (`frame3d`) | ❌ not implemented |
| nonlinear analysis (`nonlinear`) | ❌ not implemented |
| heat transfer, temperature gradients (`thermal`) | ❌ not implemented |
| Abaqus `.inp` import, STEP export | ❌ not implemented |
| contact, prestress, dynamic time history | ❌ not implemented |

The machine-readable version is printed by `ocae capabilities` and matches actual behaviour: `beam3d`/`frame3d`/`thermal`/`nonlinear` are rejected at every entry point, while `buckling`, `prescribedDisplacement`, `endRelease`, `thermalExpansion`, `elasticSupports` and `deflections` are available through the Rust API, the native addon and the TypeScript SDK/CLI for the supported model families. `thermalExpansion` covers uniform member temperature changes only; it does not enable the `thermal` heat-transfer analysis, which remains unimplemented.

## Numerical methods

1. **Direct stiffness method**: truss elements are assembled from the 3D length `L`, direction cosines `c` and axial stiffness `k = EA/L`. 2D beam elements use the 6×6 Euler-Bernoulli stiffness in local `[u1x, u1y, θ1, u2x, u2y, θ2]` coordinates (`EA/L`, `12EI/L³`, `6EI/L²`, `4EI/L`, `2EI/L`) and are assembled as `TᵀKT` with the rotation transformation `c = Δx/L, s = Δy/L`.
2. **Degree-of-freedom activation**: a 2D document containing beams uses `ux, uy, rz` globally, but `rz` is active only at continuous beam ends (a released end contributes no rotational degree of freedom); 2D documents without beams use only `ux, uy`; 3D trusses use `ux, uy, uz`. This prevents zero-stiffness rotation rows in purely truss parts and at hinges.
3. **Constraint handling**: only active, unconstrained degrees of freedom are assembled and the reduced system is solved (row/column elimination first), avoiding the ill-conditioning of the large-number penalty method. When a support prescribes a non-zero displacement `u_c`, the free right-hand side is corrected by `−K_fc·u_c`; the constrained values are then written back into the displacement vector, so reactions and element forces follow from the same `K·u − F` residual as homogeneous supports.
4. **Linear equation solution**: a self-written Gaussian elimination with partial pivoting (no linear algebra crate). A pivot smaller than `1e-12 × the largest matrix magnitude` is treated as singular and returns `model/mechanism`.
5. **Distributed loads and self-weight**:
   - The beam distributed load (local `+y`) becomes the consistent nodal vector and is then applied through the coordinate transformation. A uniform `w` gives `[0, wL/2, wL²/12, 0, wL/2, −wL²/12]`; a linear load `q(x) = q1 + (q2 − q1)·x/L` gives `[0, L(7q1+3q2)/20, L²(3q1+2q2)/60, 0, L(3q1+7q2)/20, −L²(2q1+3q2)/60]`. Both are the exact integrals for Euler-Bernoulli shape functions.
   - Truss self-weight splits `ρALg` equally between the two end nodes (lumped-mass style: exact total force balance, but without the higher-order terms of a consistent mass matrix).
   - Beam self-weight decomposes gravity into local axial and transverse components: the axial component is split equally as `ρAL(g·t)/2` (exact for linear shape functions), and the transverse component uses the consistent load above (exact for beam shape functions).
6. **Results**:
   - displacement `u = (ux, uy, uz, rz)` (`rz = 0` for pure trusses and 3D models);
   - reaction `R = K·u − F`, including `fx, fy, fz` and `mz`;
   - each element reports local end forces `endForces = {n1,v1,m1,n2,v2,m2} = K_local·d_local − f_eq` and `maxAbsMoment`; the axial force `axialForce` is reported separately with tension positive;
   - `solve_internal_forces(model, stations)` (CLI `ocae forces`) recovers section forces by equilibrium from the end forces and the applied linear/gravity loads: `N(x) = −n1 − p·x`, `V(x) = v1 + ∫q`, `M(x) = −m1 + v1·x + ∫q(x−s)ds`, sampled at equally spaced stations; the end stations reproduce the end forces exactly;
   - total strain energy `Σ ½·dᵀKd` is reported as `strainEnergy`.
7. **Modal analysis**: uses the same `K` as the original problem and a diagonal lumped mass matrix — `ρAL/2` for translational degrees of freedom and `ρIL/2` (rotary inertia) for beam rotational degrees of freedom. It forms the symmetric matrix `A = M^(−1/2)·K·M^(−1/2)`, computes eigenvalues with symmetric Jacobi iterations (off-diagonal tolerance `1e-12·max|A|`, at most 100 sweeps) and frequencies `f = √λ/(2π)`. Zero, negative or non-finite eigenvalues are skipped and counted in `skipped`; NaN is never emitted. Each mode reports the effective modal mass `(φᵀ·M·r)²` in X/Y/Z, and `totalMass` reports the total translational mass so the effective mass fraction can be obtained by division; the fixed-free axial bar first mode approaches the analytical `8/π²` fraction.
8. **Linear buckling**: first solves the same static load case and reads the signed axial force `N` of every member. The geometric stiffness is assembled from that reference state: beams use the consistent 6×6 `N/L` matrix (`6/5`, `L/10`, `2L²/15`, …) transformed to global coordinates, and trusses use the transverse projector `(N/L)·(I − n·nᵀ)` at both ends. The eigenproblem `(K + λ·Kg)·φ = 0` is reduced to a standard symmetric form with a Cholesky factor of `K_ff`, solved by the same Jacobi iteration and reported as ascending positive load factors `λ`; non-positive or non-finite factors are skipped. Modes are normalized so the largest absolute nodal value is one. Two-dimensional beams and two- or three-dimensional trusses are assembled. Buckling under load combinations scales each combination's loads (and gravity) exactly like the static combination solver and runs this analysis per case; the reported factors therefore always multiply that combination's load set, and the envelope names the combination with the lowest first factor.
9. **Mechanism detection**: every solve performs a numerical rank check on the free-degree-of-freedom stiffness matrix (whether `K_ff` has full rank); on failure it reports `model/mechanism` and lists as many unconstrained nodes as possible.
10. **Load combinations**: each combination clones the model, multiplies every nodal load and element distributed load by `loads`, multiplies the gravity vector by `gravity` (clearing it when the result is the zero vector), solves the scaled model and keeps the full static solution. The envelope reports the governing nodal displacement, signed axial force/moment maxima and per-node/per-element extremes with the combination names that produced them. Prescribed displacements and element temperature changes are not scaled.
11. **Material and section library**: presets store Young's modulus and density in `mm` units and convert by `1e6` (Pa) and `1e12` (kg/m³). Sections compute `A`, `I_strong = b h³/12` (rectangle), `I_weak`, the extreme-fibre distance `c` and a Saint-Venant torsion constant (`J ≈ a b³/3·(1 − 0.63 b/a + 0.052 (b/a)⁵)` for rectangles, `π d⁴/32` for circles and a thin-walled sum for I-sections). The elastic section modulus is `I_strong / c`; a beam may omit `inertia` and inherit `I_strong` from the section.
12. **Beam stress recovery**: at every internal-force station the report computes `σ_min/σ_max = N/A ± |M|/S` with `S = I/c`, where `I` is the element inertia (or the material-section inertia) and `c` is the material-section extreme-fibre distance. Each element reports the largest absolute stress, the governing station and, when `allowableStress` is declared, `utilisation = |σ|max / allowableStress`; the report also names the governing element. Trusses report `N/A` only. This is a sampled check, not a fibre-level integration.
13. **Support reaction summary**: after solving, the reaction vector `R = K·u − F` is grouped per support (`fx, fy, fz, mz`) and the report compares `reaction + applied` with a tolerance that scales with the largest resultant. Rigid supports report the residual; spring supports report `−k·u`. `applied` includes nodal forces, consistent distributed loads and self-weight; `balanced` is the global force-equilibrium check. The same summary is included in every static solution and written to `reactions.csv` by the CSV exporter.
14. **Elastic supports**: a spring adds its stiffness to the diagonal of the assembled matrix and leaves the matching degree of freedom free, so a model can be stabilized by springs instead of rigid constraints. Spring and rigid constraint never share a degree of freedom (validation rejects it), which keeps the reaction split unambiguous. The axial bar `P/(EA/L + k)`, the cantilever tip spring `P/(3EI/L³ + k)` and a root rotational spring `P L³/(3EI) + P L²/k` are asserted to relative error `1e-12`.
15. **Thermal expansion**: a uniform member `temperature` change `ΔT` with the material `alpha` adds the equivalent axial load `E·A·α·ΔT·[−1, +1]` in local coordinates (axial only). The reported axial force is `k·δ − E·A·α·ΔT`, so a free member expands by `α·ΔT·L` without stress and a fully restrained member carries `−E·A·α·ΔT`; a free thermal expansion stays moment-free. A temperature change without a finite `alpha` is rejected with `material/missing-alpha`.
16. **Beam end releases**: released bending rotations are statically condensed before assembly (`K_cond = K − K_er·K_rr⁻¹·K_re`, `f_cond = f − K_er·K_rr⁻¹·f_r`) so the assembled element carries no moment at the hinge. The internal released rotation is recovered from `K_rr·θ_r = f_r − K_re·d_e` before end forces are reported, which keeps `m = 0` at the released end. Releasing both ends of a member gives a moment-free (simply supported) member; a node whose beam ends are all released has no rotational degree of freedom.
17. **Deflected shape**: `solve_deflection(model, stations)` (CLI `ocae deflections`) transforms the nodal displacements into the member frame, restores released hinge rotations, and evaluates the cubic Hermite interpolation of `(v1, θ1, v2, θ2)` plus the exact fixed-end particular solution of the linearly varying transverse load (`EI·v'''' = q(x)`, a quartic/quintic polynomial). The axial displacement adds the quadratic term of a uniform axial load. Each station reports `axial`, `transverse`, `rotation` and the deformed global `x/y/z`, so simply supported, cantilever and triangular-load cases reproduce the closed-form deflections exactly.

## Validation and tests

```shell
cargo fmt -p cae-model -p open-cae-napi
cargo test -p cae-model -p open-cae-napi --offline
cargo clippy -p cae-model -p open-cae-napi --offline --all-targets -- -D warnings
pnpm --filter @wae/open-cae run typecheck
```

`projects/crates/cae-model/tests/cae_model.rs` and `tests/session.rs` cover the existing truss, 3D model, mechanism, validation, export and session paths, plus:

- single-bar axial tension: `u = PL/(EA)`, `σ = P/A`, `ε = σ/E`;
- symmetric two-bar truss, 3D vertical single bar, 3D tetrahedral symmetric structure;
- under-constrained mechanism detection, fully constrained reaction equilibrium, backward-compatible loading of old documents;
- validation warnings for `node/unconnected` (a node that no element references) and `node/duplicate-coordinates` (two nodes at the same point); both warnings never block solving;
- CSV/VTK/Markdown/HTML content and `.ocae` round trips.
- `tests/deflection.rs`: simply supported uniform load (`5wL⁴/384EI`), cantilever tip load (`PL³/3EI` and `PL²/2EI`), cantilever uniform load (`wL⁴/8EI`), triangular load (`11wL⁴/120EI`), a straight truss sample and the session JSON export.

`projects/crates/cae-model/tests/beam_modal.rs` asserts analytical solutions:

- cantilever beam with an end concentrated force: end deflection `PL³/(3EI)`, fixed-end reaction moment `PL` (relative error `1e-9`);
- cantilever beam with a uniform distributed load: end deflection `wL⁴/(8EI)` (`1e-9`);
- cantilever beam with a triangular load of peak `q0` at the fixed end: end deflection `q0L⁴/(30EI)` and fixed-end reaction moment `q0L²/6` (`1e-9`);
- simply supported beam with a midspan concentrated force: midspan deflection `PL³/(48EI)` and maximum bending moment `PL/4` (`1e-9`);
- fixed-free axial bar (10 elements): first frequency within `< 3%` of `(1/(4L))√(E/ρ)`, and its first-mode effective mass fraction within `< 5%` of the analytical `8/π²`;
- single free translational degree of freedom: effective mass equals the free nodal mass `ρAL/2` and `totalMass` equals `ρAL` (`1e-12`);
- cantilever beam (40 elements): first bending frequency within `< 5%` of `1.875104²·√(EI/(ρAL⁴))/(2π)`;
- pinned-pinned beam column (20 elements): first linear buckling factor within `0.1%` of the Euler load `P_cr = π²EI/L²`;
- three-dimensional tetrahedron truss: the two lateral buckling factors match the closed-form geometric-stiffness eigenvalue `E·A·R²/(N·(R²+2H²))` (relative error `1e-8`);
- vertical bar self-weight: sum of reactions equals total weight (relative error `1e-9`);
- internal force sampling: cantilever `V(0)=wL`, `M(0)=−wL²/2`, `M(L)=0`; simply supported midspan `M=PL/4`, `V=P/2`; sampled end values reproduce the reported element end forces; truss axial force is constant and shear/moment are zero; fewer than two stations are rejected (`1e-9`);
- rejection/behaviour tests for 3D beams, truss distributed loads, zero-density modal analysis, three-dimensional buckling, and byte-identical Markdown **and** HTML report determinism.

`projects/crates/cae-model/tests/prescribed.rs` asserts prescribed-displacement behaviour:

- axial bar with a prescribed end elongation: `N = EAδ/L`, `σ = Eδ/L` and equal/opposite end reactions (`1e-12`);
- propped cantilever with a `δ` roller settlement: tip rotation `−1.5δ/L`, roller reaction `−3EIδ/L³`, fixed reaction `+3EIδ/L³`, fixed-end moment `3EIδ/L²` (`1e-12`);
- rejection of empty prescriptions, unknown nodes, `uz` in 2D and `rz` in 3D, plus the `support/prescribed-unconstrained` validation finding;
- `.ocae` round trips that preserve the `prescribed` block.

`projects/crates/cae-model/tests/combinations.rs` asserts load-combination behaviour:

- dead (`gravity` only) and live (`loads` only) combinations reproduce the self-weight `wL⁴/(8EI)` and tip-load `PL³/(3EI)` cantilever deflections (`1e-9`);
- the `D+L` solution is exactly the linear superposition of the dead and live solutions, including the fixed-end reaction moment (`1e-12`);
- the envelope selects `D+L`, reports the governing node/element and matches the individual solutions;
- empty lists (`combination/none`), non-finite factors (`combination/invalid-factor`), empty and duplicate names are rejected, combinations round-trip through `.ocae`, and the session writes the JSON report.

`projects/crates/cae-model/tests/library.rs` asserts the material and section library:

- rectangular, circular and I-section area, strong/weak inertia, torsion constant, extreme fibre and section modulus match the closed-form formulas (`1e-12`);
- presets convert between `mm` (`210000 MPa`, `7.85e-9 t/mm³`) and `m` (`2.1e11 Pa`, `7850 kg/m³`);
- a cantilever beam that omits `inertia` uses the stored section and reproduces `PL³/(3EI)` (`1e-9`);
- a preset plus section round-trips through the document, validation accepts the model and the session solves it;
- invalid section dimensions, non-positive allowable stress and a beam without inertia or section are rejected with stable codes.

`projects/crates/cae-model/tests/stress.rs` asserts beam stress recovery:

- a cantilever tip load reproduces `|σ| = PL/S` at the fixed end with `S = I/c` (`1e-9`), a zero free-end moment and the expected utilisation;
- an axial force shifts the extreme-fibre stresses to `N/A ± PL/S`;
- truss elements report `N/A` without a section and no utilisation;
- a beam without a material section is rejected with `material/missing-section`, and fewer than two stations with `analysis/invalid-stations`;
- the session writes a valid `open-cae-stress-report` JSON file.

`projects/crates/cae-model/tests/buckling_combinations.rs` asserts buckling under several load combinations:

- a pinned-pinned column under `P` and `2P` reports Euler factors `π²EI/(L²·P)` and `π²EI/(2L²·P)`, whose ratio is exactly two (`1e-3`);
- the envelope selects `2P` and repeats its first factor (`1e-12`);
- a zero-load combination produces no positive factor, is skipped and cannot win the envelope;
- an empty combination list (`combination/none`) and `modes = 0` (`analysis/invalid-modes`) are rejected;
- the session writes a valid `open-cae-buckling-combination-report` JSON file.

`projects/crates/cae-model/tests/springs.rs` asserts linear elastic spring supports:

- an axial bar with an end spring reproduces `u = P/(EA/L + k)`, the fixed reaction `−EA·u/L`, the spring force `−k·u` and a balanced reaction summary (`1e-12`);
- a cantilever with a tip vertical spring reproduces `δ = P/(3EI/L³ + k)`, a fixed-end reaction `P − kδ`, the spring force `kδ` and a balanced summary (`1e-12`);
- a cantilever whose root rotation is held by a rotational spring reproduces `θ = −PL/kr` and `δ = PL³/(3EI) + PL²/kr`, with the spring moment `PL` (`1e-12`);
- negative, zero and unknown-node springs are rejected; `kz` in 2D, `kr` at a truss-only node and a spring plus rigid flag on the same component produce stable validation codes;
- a spring round-trips through `.ocae`, reopens through the session and solves.

`projects/crates/cae-model/tests/reactions.rs` asserts the reaction summary:

- a cantilever tip load gives `fy = +P`, `mz = +PL`, `applied.fy = −P` and zero unbalanced force (`1e-9`);
- the session writes a JSON report with `balanced: true` and the support node;
- CSV export writes `reactions.csv` with one header row and one row per support (`1e-9`);
- static solutions keep the `reactionSummary` field.

For the single-bar benchmark (L=1000 mm, E=210000 MPa, A=100 mm², P=1000 N) the analytical displacement is `0.0476190476 mm` and the stress is `10 MPa`; the tests assert against the analytical expressions with a relative error threshold of `1e-9`.

## Repository structure

```text
projects/
  sub_projects/          existing template crate (unchanged)
  crates/cae-model/      OpenCAE core library + `ocae` CLI
    src/session.rs       OpenCae session API (all editing/validation/solving/export)
    src/model.rs         data model and document I/O
    src/library.rs       built-in material presets and section properties
    src/validate.rs      validation rules and mechanism detection
    src/solver.rs        truss/beam stiffness, DOF activation, Gaussian elimination, Jacobi modal/buckling solvers, internal forces and stress recovery
    src/export.rs        CSV / VTK / Markdown / HTML export and atomic writes
    src/bin/ocae.rs      thin CLI: argument parsing, API calls, printing and exit codes
    tests/cae_model.rs   truss model solve and export tests
    tests/beam_modal.rs  analytical beam, self-weight, modal and buckling tests
    tests/prescribed.rs  analytical prescribed-displacement and settlement tests
    tests/combinations.rs linear load-combination and envelope tests
    tests/buckling_combinations.rs per-combination buckling tests
    tests/library.rs     material-preset and section-property tests
    tests/stress.rs      beam stress-recovery tests
    tests/reactions.rs   support-reaction summary and CSV tests
    tests/springs.rs     elastic spring-support tests
    tests/thermal.rs     uniform temperature-change and thermal-expansion tests
    tests/releases.rs    beam end-release (hinge) tests
    tests/session.rs     CLI-equivalent path tests for the session API
  crates/napi/           `open-cae-napi`: orthogonal Node-API exports
    src/lib.rs           version/capabilities/create/open/save/…/report
  packages/open-cae/     `@wae/open-cae` TypeScript SDK and `open-cae` CLI
    src/open-cae.ts      `new OpenCAE()` session
    src/document.ts      `CaeDocument` document
    src/service.ts       orthogonal native operations and error translation
    src/cli.ts           thin wrapper equivalent to the Rust CLI
    locales/en-US/cli.json   CLI text
```

The root `Cargo.toml` uses a workspace. Because Cargo treats the intermediate directory `projects/crates` under `projects/*` as a member, the root manifest explicitly excludes that directory and explicitly includes `projects/crates/cae-model` and `projects/crates/cae-napi`. `wae` is imported as a git dependency pinned to a fixed rev, enabling only the Node-API facade and not pulling WebView platform dependencies.

## Limitations

- Beams are **2D Euler-Bernoulli only** (no shear deformation, no axial-bending coupling); bending-moment **end releases are supported** as condensed hinges. **3D beams/frames (beam3d / frame3d) are not implemented** and return `capability/not-implemented`.
- Only **linear elastic, small deformation** analysis is supported; geometric/material nonlinearity, dynamic time history, heat transfer, temperature gradients, contact and prestress are not supported. Prescribed displacements are supported as a single static kinematic boundary condition, not as a time-dependent or multi-step loading history.
- Buckling is a **linearized eigenvalue analysis** (Euler buckling) based on the static axial forces of one load case. It does not track post-buckling behaviour, imperfection sensitivity, plasticity or load-history effects. Three-dimensional beams remain unsupported, so three-dimensional buckling is limited to truss models.
- Loads are limited to **nodal concentrated forces/moments**, **element transverse loads that are uniform or linear along the member in the local `+y` direction**, and **uniform member temperature changes** (axial expansion only, no thermal bending). Arbitrary curved distributed loads, loads in a global direction, temperature gradients and heat transfer are not supported; load combinations do not scale temperature changes.
- Load combinations only scale the **single applied load set and self-weight**; independent named load patterns (separate live-load cases) are not modelled, so a combination cannot mix two different nodal load layouts.
- The material and section are **global**: all elements share one material and one parametric section. Per-element `E`/`A` overrides still exist, but there is no per-element section library yet.
- Spring supports are **linear elastic and two-way**: no tension-only/compression-only (gap) behaviour, no friction, no plasticity, and no nonlinear force-displacement law is modelled.
- Beam stress recovery is **sampled** at the requested stations and uses the global material section. Shear stress distributions, stress concentrations and fibre-level plasticity are not modelled.
- Modal analysis uses a **lumped mass matrix** and ignores constrained degrees of freedom; frequencies and mode shapes are finite element approximations. Results contain only the lowest few modes; buckling and response-spectrum analysis are not performed.
- Self-weight is applied as **lumped forces** (trusses) or **consistent loads** (beam transverse direction); this is exact in equilibrium and approximate in displacement, and does not replace a consistent mass matrix.
- The solver is **dense**: memory `O(n²)` and time `O(n³)`, suitable for small models but not for large engineering models.
- Mechanism detection is based on numerical pivot thresholds; extremely ill-conditioned but theoretically stable structures may be misclassified.
- CSV/VTK numerical units follow the document declaration and the exporter performs no conversion; VTK uses the legacy ASCII format. Markdown and HTML reports contain no timestamps, so repeated exports are byte-identical. CSV node columns are `id,x,y,z,ux,uy,uz` (z/uz are 0 in 2D) and `reactions.csv` columns are `node,fx,fy,fz,mz`.
- External formats such as Abaqus `.inp`, Nastran and STEP are neither imported nor exported.
