# OpenCAE

Structural analysis for 2D frames and trusses that stays in plain text and reproduces from the command line.

## 💡 What is OpenCAE?

OpenCAE is a small structural-analysis engine for **planar frames, beams and trusses** and **space trusses**. You describe the model as a `.ocae` JSON document, then run linear static, modal, linear buckling, response-spectrum or harmonic-response analysis and read the results as JSON, CSV, VTK, Markdown, HTML or SVG. The same model file and the same commands give the same numbers on every machine, which makes it useful for teaching, algorithm validation and regression checks in CI.

It is for axial-force systems (trusses, bracing, space trusses) and for 2D bending members (cantilevers, simply supported beams, portal frames in one plane). It is built to be explicit about what it can and cannot do.

Typical uses:

- teaching stiffness, buckling and modal behaviour with numbers students can check by hand;
- validating a new solver against closed-form beam and truss solutions;
- small regression models that run from a script in CI;
- early sizing of bracing, cable and beam layouts before a full 3D model exists.

A model is one JSON document: units, dimension, one material (with an optional parametric section), nodes, elements, supports, loads, gravity and named load combinations. Units are not converted — `mm` pairs with N, MPa, t/mm³ and tonnes, while `m` pairs with N, Pa, kg/m³ and kilograms. Modal frequencies are reported in Hz.

**OpenCAE is not:**

- a 3D solid or shell finite element package — there are no solids, shells, plates, contact or heat-transfer elements;
- a general commercial FEA pre/post-processor — there is no geometry kernel, mesher or viewer;
- a place where an unsupported feature silently degrades — an unimplemented capability returns a stable error instead of an approximate result.

## 🚀 Getting started

### 1. Point your agent at the repository

OpenCAE does not ship an agent skills package. Point your agent at the repository documentation and the `open-cae` CLI, then give it a concrete task:

```text
Use OpenCAE (`open-cae`) to model a 1 m steel cantilever beam in 2D
with a 1 kN downward tip load. Solve it and report the tip deflection
and the fixed-end moment. Read documentation/cli.md for the commands
and documentation/capabilities.md before claiming a result.
```

The agent path works because every capability and error code is documented and the CLI prints machine-readable JSON.

### 2. Install and run

The smallest useful model needs nodes, at least one element, a material, enough supports to prevent a mechanism, and a load.

```shell
pnpm add @wae/open-cae

pnpm exec open-cae create bar.ocae --units mm
pnpm exec open-cae material set bar.ocae --E 210000 --A 100
pnpm exec open-cae node add bar.ocae --id n1 --x 0 --y 0
pnpm exec open-cae node add bar.ocae --id n2 --x 1000 --y 0
pnpm exec open-cae element add bar.ocae --id e1 --from n1 --to n2
pnpm exec open-cae support add bar.ocae --node n1 --dx --dy
pnpm exec open-cae support add bar.ocae --node n2 --dy
pnpm exec open-cae load add bar.ocae --node n2 --fx 1000
pnpm exec open-cae solve bar.ocae
pnpm exec open-cae validate bar.ocae
```

`validate` reports `{"ok": true, ...}` for a buildable model; a mechanism or an inconsistent dimension returns an error. `pnpm exec open-cae capabilities` lists what the installed build supports, and `pnpm exec open-cae --help` lists every command.

The bar carries 1000 N in tension and stretches by 0.047619 mm. The SDK is the same engine with a typed API:

```ts
import { OpenCAE } from '@wae/open-cae';

const cae = new OpenCAE({ units: 'mm' });
const doc = await cae.open('bar.ocae');
console.log(doc.solve().elements[0].axialForce);
await cae.dispose();
```

A modal or buckling run reuses the same file once a density or a nodal mass is present:

```shell
pnpm exec open-cae material set beam.ocae --E 210000 --A 100 --density 7.85e-9
pnpm exec open-cae modal beam.ocae --modes 5
pnpm exec open-cae buckling beam.ocae --modes 2
```

Every solve prints JSON to stdout; add `--out report.json` to keep a copy. Exit code `1` means invalid input or an unbuildable model, `2` means the requested capability is not implemented.

## ✨ Highlights

- **Linear static analysis** of 2D frames/beams and 2D/3D trusses with axial force, shear, bending moment and a support-reaction equilibrium check.
- **Modal analysis** with signed participation factors, effective modal mass and concentrated nodal point masses.
- **Linear buckling** for a single load case and per load combination, with a governing-case envelope.
- **Response spectrum** (SDOF spectral acceleration + SRSS) and **harmonic frequency response** with sampled peaks.
- **Load combinations** with load/gravity factors, envelopes and per-case summaries.
- **Rich support modelling**: rigid, spring, inclined, prescribed-displacement and construction-stage element activation.
- **Member actions**: distributed and partial-span loads, end releases, uniform temperature change, through-depth temperature gradient and tension-only cables.
- **Topology edits**: in-place node moves, element reconnection, node merging and parametric member splitting that reuses coincident nodes and keeps material, section and uniform loads.
- **Result queries and exports**: internal-force, stress, deflection and nodal-stress sampling, assembled stiffness/mass matrices, and CSV, VTK, Markdown and HTML reports plus deterministic SVG drawings of the model and deformed shape.

Static results include a force-equilibrium check, and reports are deterministic: repeated Markdown, HTML and SVG exports are byte-identical because they contain no timestamps.

## 🧪 Examples

- [Package README — CLI and SDK usage](projects/packages/open-cae/README.md)
- [Two-bar cable tension-only benchmark](projects/crates/cae-model/tests/tension_only.rs)
- [Beam, modal and buckling tests with analytical targets](projects/crates/cae-model/tests/beam_modal.rs)
- [Partial-span load test with closed-form midspan values](projects/crates/cae-model/tests/partial_load.rs)
- [Response-spectrum and SDOF tests](projects/crates/cae-model/tests/response_spectrum.rs)
- [Construction-stage activation tests](projects/crates/cae-model/tests/construction_stages.rs)
- [Capability matrix and numerical methods](documentation/capabilities.md)

The full test suite lives in `projects/crates/cae-model/tests/`; each file names the analytical solution it asserts.

## 📚 Documentation

| Document | Contents |
|----------|----------|
| [documentation/architecture.md](documentation/architecture.md) | Model, module boundaries, data flow, error semantics |
| [documentation/capabilities.md](documentation/capabilities.md) | Supported features, numerical methods, tests and limitations |
| [documentation/cli.md](documentation/cli.md) | Every `open-cae` command, flag and example |
| [documentation/development.md](documentation/development.md) | Build, test, native addon staging and release |
| [projects/packages/open-cae/README.md](projects/packages/open-cae/README.md) | The npm package page |

## 📜 License

[Mozilla Public License 2.0](License.md)
