# OpenCAE

OpenCAE is a **narrowly scoped and verifiable** linear-elastic analysis tool for 2D/3D trusses and 2D Euler-Bernoulli beams, supporting linear static, self-weight, prescribed displacements, modal and linear buckling analysis. It targets:

- trusses, space frames and bracing systems that carry axial force only;
- bending problems such as planar beams, cantilever beams and simply supported beams;
- teaching, algorithm validation and small regression models in CI;
- workflows that store models as plain-text JSON and reproduce results from the command line.

It is **not** a general-purpose commercial finite element package: 3D beams/frames, nonlinear, thermal and contact analysis are not implemented. Every unimplemented capability fails explicitly (`capability/not-implemented`, exit code `2`) instead of degrading silently. Modal analysis uses a lumped mass matrix, so frequencies are finite element approximations; the readme and tests state the error bounds against analytical solutions.

## Architecture: the API is the core and the CLI only parses arguments

The `OpenCae` session API in `projects/crates/cae-model` is the only implementation: document I/O, editing, validation, solving and CSV/VTK/Markdown export all live there. The three entry points share the same logic:

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
document.export('csv', 'out/csv');       // out/csv/nodes.csv, elements.csv
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
```

- Sessions: `capabilities()`, `create(path, options?)`, `open(path)`, `dispose()`.
- Documents: `id/units/dimension/summary`, plus `addNode/addElement/setMaterial/addSupport/addLoad/setGravity/addCombination/removeCombination/validate/solve/solveCombinations/internalForces/export/report/save/close`; every edit operation writes atomically to the file.
- `addSupport()` also accepts `ux`, `uy`, `uz` and `rotation` to enforce a non-zero displacement (support settlement or driven displacement); supplying one constrains the matching degree of freedom.
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
cargo run -p cae-model --bin ocae --offline -- export beam.ocae --format md --out out/report.md
```

`--kind beam` requires `--I`; `--w` is the distributed transverse load in the element-local `+y` direction at the start node (force/length, positive upwards) and `--w2` optionally sets the intensity at the end node. A uniform load (`--w` only, or `--w2` equal to it) is converted to the consistent nodal vector `[0, wL/2, wL²/12, 0, wL/2, −wL²/12]`; `--w2` closes a linearly varying (trapezoidal) load instead. The nodal moment `--mz` is positive counter-clockwise. `self-weight --g -9810 --axis y` is equivalent to `model gravity --gy -9810`.

Prescribed displacements (support settlements or driven degrees of freedom) use the same command; supplying `--ux/--uy/--uz/--rotation` also constrains that degree of freedom:

```shell
cargo run -p cae-model --bin ocae --offline -- support add beam.ocae --node n2 --uy -2
cargo run -p cae-model --bin ocae --offline -- solve beam.ocae   # roller settles 2 mm
```

For a propped cantilever with a 2 mm roller settlement the roller reaction is `3EIδ/L³` downwards, the fixed-end reaction is the opposite force and the fixed-end moment is `3EIδ/L²`; the value is asserted to relative error `1e-12` in the tests.

## CLI commands

| Command | Description |
|------|------|
| `ocae create <file.ocae> [--units mm\|m] [--dimension 2\|3]` | Create a document; the default dimension is 2 and consistent steel defaults are written (millimetre system E=210000 MPa, A=100 mm²; metre system E=2.1e11 Pa, A=1e-4 m²) |
| `ocae node add <file> --id <id> --x <x> --y <y> [--z <z>]` | Add a node; duplicate ids are rejected; `--z` defaults to 0 and may be non-zero only in 3D documents |
| `ocae element add <file> --id <id> --from <n> --to <n> [--E <E>] [--A <A>] [--kind truss\|beam] [--I <I>] [--w <w>] [--w2 <w2>]` | Add a truss/beam element; `beam` is 2D-only and requires a positive `--I`; `--w`/`--w2` are the local `+y` transverse intensities at the start/end node (uniform when `--w2` is omitted) |
| `ocae material set <file> --E <E> --A <A> [--density <rho>] [--name <name>]` | Set the global material, section and density (modal/self-weight analysis requires `--density > 0`) |
| `ocae support add <file> --node <n> [--dx] [--dy] [--dz] [--rz] [--ux <v>] [--uy <v>] [--uz <v>] [--rotation <v>]` | Add/merge support constraints; `--rz` is available only on beam nodes and `--dz` only in 3D documents. A supplied displacement clamps that degree of freedom to the given value and implies its flag |
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
| `ocae forces <file> [--stations N] [--out report.json]` | Print sampled axial force, shear and bending moment per element; `--stations` counts both ends and defaults to 11 |
| `ocae export <file> --format csv --out <dir>` | Write `<dir>/nodes.csv` and `<dir>/elements.csv`; node columns include `x,y,z` and `ux,uy,uz` |
| `ocae export <file> --format vtk --out <dir\|file.vtk>` | Write legacy ASCII VTK; points are 3D coordinates and displacements are 3D vectors |
| `ocae export <file> --format md --out <dir\|file.md>` | Write a deterministic Markdown report; appends the first 5 modes when density is positive |
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
  "material": { "name": "steel", "youngsModulus": 210000.0, "area": 100.0, "density": 7.85e-9 },
  "nodes": [
    { "id": "n1", "x": 0.0, "y": 0.0, "z": 0.0 },
    { "id": "n2", "x": 1000.0, "y": 0.0, "z": 0.0 }
  ],
  "elements": [
    { "id": "e1", "from": "n1", "to": "n2", "kind": "truss" },
    { "id": "b1", "from": "n1", "to": "n2", "kind": "beam", "inertia": 1000000.0, "distributedLoad": -2.0 }
  ],
  "supports": [
    { "node": "n1", "dx": true, "dy": true, "dz": false, "rz": true }
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
| `material` | Global material: `youngsModulus` (E), `area` (A), `density` (default 0), optional `name` |
| `nodes` | Node ids must be unique; coordinates are `x`, `y`, `z`, with `z` defaulting to 0 |
| `elements` | `kind` is `truss` (default) or `beam`; `from`/`to` reference nodes; optional `material: { "youngsModulus": ..., "area": ... }` overrides E/A; `beam` requires a positive `inertia` and accepts optional `distributedLoad`/`distributedLoadEnd` intensities in the local `+y` direction (equal or omitted end value means uniform, otherwise linearly varying) |
| `supports` | `dx`/`dy`/`dz` constrain X/Y/Z translation when `true`; `rz` constrains rotation about Z and applies only at beam nodes; all default to `false`. Optional `prescribed: { ux, uy, uz, rz }` enforces non-zero values of the constrained degrees of freedom (omitted components are `0`), so old documents load unchanged |
| `loads` | Nodal concentrated forces `fx`/`fy`/`fz` and moment `mz`, defaulting to 0; `mz` applies only at beam nodes |
| `gravity` | Optional `[gx, gy, gz]`; when set, self-weight is applied from the density |
| `combinations` | Optional list of `{ name, loads, gravity }` linear combinations; `loads` multiplies nodal and element applied loads (default 1), `gravity` multiplies self-weight (default 0). Names must be unique and factors finite |
| `revision` | Edit counter incremented by CLI modifications; may be 0 |

**Dimension consistency**: if a `dimension: 2` document contains a non-zero `z`, `dz: true`, a non-zero `fz` or a non-zero `gz`, validation returns a `document/dimension-mismatch` error (edit commands also reject it before writing) instead of silently ignoring those values. 3D documents do not allow `beam` elements, `rz: true` or a non-zero `mz` (returning `capability/not-implemented` or `document/dimension-mismatch`). `rz`/`mz` may only be used on nodes connected by beam elements; otherwise `support/inactive-dof` / `load/inactive-dof` is reported. A `prescribed` value must be finite and its matching flag must be `true`, otherwise `support/invalid-prescribed` or `support/prescribed-unconstrained` is reported; a non-zero `prescribed.uz` in 2D or `prescribed.rz` in 3D is a `document/dimension-mismatch`.

**Degree-of-freedom activation**: a 2D document that contains any beam element uses 3 degrees of freedom per node (`ux, uy, rz`); however, `rz` is active only at nodes touched by a beam, so purely truss parts never produce zero-stiffness rotation rows. Pure truss documents still use 2 degrees of freedom per node. 3D trusses keep 3 translational degrees of freedom per node.

**Unit convention**: the solver performs no unit conversion and only requires numerically consistent input. With `units: "mm"`, use N, mm, mm² and MPa, with density in t/mm³ (steel is about `7.85e-9`); with `units: "m"`, use N, m, m² and Pa, with density in kg/m³. Mixing units is not an error, but the results have no physical meaning. Modal frequencies are in Hz.

## Capability matrix

| Capability | Status |
|------|------|
| 2D truss linear static (`truss2d`) | ✅ supported |
| 3D truss linear static (`truss3d`) | ✅ supported |
| 2D Euler-Bernoulli beam (`beam2d`, including axial force/shear/bending moment, distributed loads, end moments) | ✅ supported |
| self-weight (`selfWeight`, lumped truss distribution + consistent beam loads) | ✅ supported |
| prescribed displacements (`prescribedDisplacement`, support settlements) | ✅ supported |
| modal analysis (`modal`, lumped mass + Jacobi) | ✅ supported |
| linear buckling (`buckling`, 2D truss/beam and 3D truss geometric stiffness + Jacobi) | ✅ supported |
| internal force diagrams (`internalForces`, sampled N/V/M per element) | ✅ supported |
| load combinations (`loadCombinations`, linear factors + envelope) | ✅ supported |
| `.ocae` JSON I/O | ✅ supported |
| CSV / legacy VTK / Markdown report export | ✅ supported |
| 3D beam (`beam3d`) | ❌ not implemented |
| 3D frame (`frame3d`) | ❌ not implemented |
| nonlinear analysis (`nonlinear`) | ❌ not implemented |
| thermal analysis (`thermal`) | ❌ not implemented |
| Abaqus `.inp` import, STEP export | ❌ not implemented |
| contact, prestress, dynamic time history | ❌ not implemented |

The machine-readable version is printed by `ocae capabilities` and matches actual behaviour: `beam3d`/`frame3d`/`thermal`/`nonlinear` are rejected at every entry point, while `buckling` and `prescribedDisplacement` are available through the Rust API, the native addon and the TypeScript SDK/CLI for the supported model families.

## Numerical methods

1. **Direct stiffness method**: truss elements are assembled from the 3D length `L`, direction cosines `c` and axial stiffness `k = EA/L`. 2D beam elements use the 6×6 Euler-Bernoulli stiffness in local `[u1x, u1y, θ1, u2x, u2y, θ2]` coordinates (`EA/L`, `12EI/L³`, `6EI/L²`, `4EI/L`, `2EI/L`) and are assembled as `TᵀKT` with the rotation transformation `c = Δx/L, s = Δy/L`.
2. **Degree-of-freedom activation**: a 2D document containing beams uses `ux, uy, rz` globally, but `rz` is active only at beam nodes; 2D documents without beams use only `ux, uy`; 3D trusses use `ux, uy, uz`. This prevents zero-stiffness rotation rows in purely truss parts.
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
7. **Modal analysis**: uses the same `K` as the original problem and a diagonal lumped mass matrix — `ρAL/2` for translational degrees of freedom and `ρIL/2` (rotary inertia) for beam rotational degrees of freedom. It forms the symmetric matrix `A = M^(−1/2)·K·M^(−1/2)`, computes eigenvalues with symmetric Jacobi iterations (off-diagonal tolerance `1e-12·max|A|`, at most 100 sweeps) and frequencies `f = √λ/(2π)`. Zero, negative or non-finite eigenvalues are skipped and counted in `skipped`; NaN is never emitted.
8. **Linear buckling**: first solves the same static load case and reads the signed axial force `N` of every member. The geometric stiffness is assembled from that reference state: beams use the consistent 6×6 `N/L` matrix (`6/5`, `L/10`, `2L²/15`, …) transformed to global coordinates, and trusses use the transverse projector `(N/L)·(I − n·nᵀ)` at both ends. The eigenproblem `(K + λ·Kg)·φ = 0` is reduced to a standard symmetric form with a Cholesky factor of `K_ff`, solved by the same Jacobi iteration and reported as ascending positive load factors `λ`; non-positive or non-finite factors are skipped. Modes are normalized so the largest absolute nodal value is one. Two-dimensional beams and two- or three-dimensional trusses are assembled.
9. **Mechanism detection**: every solve performs a numerical rank check on the free-degree-of-freedom stiffness matrix (whether `K_ff` has full rank); on failure it reports `model/mechanism` and lists as many unconstrained nodes as possible.
10. **Load combinations**: each combination clones the model, multiplies every nodal load and element distributed load by `loads`, multiplies the gravity vector by `gravity` (clearing it when the result is the zero vector), solves the scaled model and keeps the full static solution. The envelope reports the governing nodal displacement, signed axial force/moment maxima and per-node/per-element extremes with the combination names that produced them. Prescribed displacements are not scaled.

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
- CSV/VTK/Markdown content and `.ocae` round trips.

`projects/crates/cae-model/tests/beam_modal.rs` asserts analytical solutions:

- cantilever beam with an end concentrated force: end deflection `PL³/(3EI)`, fixed-end reaction moment `PL` (relative error `1e-9`);
- cantilever beam with a uniform distributed load: end deflection `wL⁴/(8EI)` (`1e-9`);
- cantilever beam with a triangular load of peak `q0` at the fixed end: end deflection `q0L⁴/(30EI)` and fixed-end reaction moment `q0L²/6` (`1e-9`);
- simply supported beam with a midspan concentrated force: midspan deflection `PL³/(48EI)` and maximum bending moment `PL/4` (`1e-9`);
- fixed-free axial bar (10 elements): first frequency within `< 3%` of `(1/(4L))√(E/ρ)`;
- cantilever beam (40 elements): first bending frequency within `< 5%` of `1.875104²·√(EI/(ρAL⁴))/(2π)`;
- pinned-pinned beam column (20 elements): first linear buckling factor within `0.1%` of the Euler load `P_cr = π²EI/L²`;
- three-dimensional tetrahedron truss: the two lateral buckling factors match the closed-form geometric-stiffness eigenvalue `E·A·R²/(N·(R²+2H²))` (relative error `1e-8`);
- vertical bar self-weight: sum of reactions equals total weight (relative error `1e-9`);
- internal force sampling: cantilever `V(0)=wL`, `M(0)=−wL²/2`, `M(L)=0`; simply supported midspan `M=PL/4`, `V=P/2`; sampled end values reproduce the reported element end forces; truss axial force is constant and shear/moment are zero; fewer than two stations are rejected (`1e-9`);
- rejection/behaviour tests for 3D beams, truss distributed loads, zero-density modal analysis, three-dimensional buckling and Markdown report determinism.

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

For the single-bar benchmark (L=1000 mm, E=210000 MPa, A=100 mm², P=1000 N) the analytical displacement is `0.0476190476 mm` and the stress is `10 MPa`; the tests assert against the analytical expressions with a relative error threshold of `1e-9`.

## Repository structure

```text
projects/
  sub_projects/          existing template crate (unchanged)
  crates/cae-model/      OpenCAE core library + `ocae` CLI
    src/session.rs       OpenCae session API (all editing/validation/solving/export)
    src/model.rs         data model and document I/O
    src/validate.rs      validation rules and mechanism detection
    src/solver.rs        truss/beam stiffness, DOF activation, Gaussian elimination, Jacobi modal/buckling solvers
    src/export.rs        CSV / VTK / Markdown export and atomic writes
    src/bin/ocae.rs      thin CLI: argument parsing, API calls, printing and exit codes
    tests/cae_model.rs   truss model solve and export tests
    tests/beam_modal.rs  analytical beam, self-weight, modal and buckling tests
    tests/prescribed.rs  analytical prescribed-displacement and settlement tests
    tests/combinations.rs linear load-combination and envelope tests
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

- Beams are **2D Euler-Bernoulli only** (no shear deformation, no axial-bending coupling, no moment releases); **3D beams/frames (beam3d / frame3d) are not implemented** and return `capability/not-implemented`.
- Only **linear elastic, small deformation** analysis is supported; geometric/material nonlinearity, dynamic time history, thermal, contact and prestress are not supported. Prescribed displacements are supported as a single static kinematic boundary condition, not as a time-dependent or multi-step loading history.
- Buckling is a **linearized eigenvalue analysis** (Euler buckling) based on the static axial forces of one load case. It does not track post-buckling behaviour, imperfection sensitivity, plasticity or load-history effects. Three-dimensional beams remain unsupported, so three-dimensional buckling is limited to truss models.
- Loads are limited to **nodal concentrated forces/moments** and **element transverse loads that are uniform or linear along the member in the local `+y` direction**; arbitrary curved distributed loads, loads in a global direction and temperature loads are not supported.
- Load combinations only scale the **single applied load set and self-weight**; independent named load patterns (separate live-load cases) are not modelled, so a combination cannot mix two different nodal load layouts.
- Modal analysis uses a **lumped mass matrix** and ignores constrained degrees of freedom; frequencies and mode shapes are finite element approximations. Results contain only the lowest few modes; buckling and response-spectrum analysis are not performed.
- Self-weight is applied as **lumped forces** (trusses) or **consistent loads** (beam transverse direction); this is exact in equilibrium and approximate in displacement, and does not replace a consistent mass matrix.
- The solver is **dense**: memory `O(n²)` and time `O(n³)`, suitable for small models but not for large engineering models.
- Mechanism detection is based on numerical pivot thresholds; extremely ill-conditioned but theoretically stable structures may be misclassified.
- CSV/VTK numerical units follow the document declaration and the exporter performs no conversion; VTK uses the legacy ASCII format. CSV node columns are `id,x,y,z,ux,uy,uz` (z/uz are 0 in 2D).
- External formats such as Abaqus `.inp`, Nastran and STEP are neither imported nor exported.
