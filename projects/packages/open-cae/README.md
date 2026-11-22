# @wae/open-cae

The Node.js session API and `open-cae` command line for OpenCAE's 2D frame, truss and cable solver.

This package is the typed entry point to the same document, solver and exporters used by the CLI. It opens `.ocae` files, edits them, validates, solves and writes reports.

## Exports

| Export | Role |
|--------|------|
| `OpenCAE` | Analysis session; `create()`, `open()`, `library()`, `sectionProperties()`, `capabilities()`, `dispose()` |
| `CaeDocument` | Path-backed document with editing, solve, query, export and save methods |
| `OpenCAEError`, `isOpenCAEError`, `errorCodes` | Stable error codes such as `model/mechanism` and `capability/not-implemented` |
| `NotImplementedError` | Subclass thrown for unimplemented capabilities |
| types | Requests, options and reports for every method |

## Usage

```shell
pnpm exec open-cae create bar.ocae --units mm
pnpm exec open-cae material set bar.ocae --E 210000 --A 100
pnpm exec open-cae node add bar.ocae --id n1 --x 0 --y 0
pnpm exec open-cae node add bar.ocae --id n2 --x 1000 --y 0
pnpm exec open-cae element add bar.ocae --id e1 --from n1 --to n2
pnpm exec open-cae support add bar.ocae --node n1 --dx --dy
pnpm exec open-cae support add bar.ocae --node n2 --dy
pnpm exec open-cae load add bar.ocae --node n2 --fx 1000
pnpm exec open-cae solve bar.ocae
```

The bar carries 1000 N in tension. From code, the document object is the same model:

```ts
import { OpenCAE } from '@wae/open-cae';

const cae = new OpenCAE({ units: 'mm' });
const doc = await cae.open('bar.ocae');
console.log(doc.solve().elements[0].axialForce); // 1000
await cae.dispose();
```

## Constraints

- **2D beams only** (Euler-Bernoulli) plus **2D and 3D trusses**. 3D beams, frames, shells and solids are not implemented; they fail with `capability/not-implemented`.
- Analysis is **linear static**, **modal**, **linear buckling**, **response spectrum**, **harmonic response** and **tension-only cables**. Geometric nonlinearity is limited to truss models.
- Modal analysis needs a positive `density` or a positive nodal `mass`; self-weight needs `density`; a beam needs `inertia` or a material `section`.
- The addon is loaded from `@wae/open-cae-<platform>-<arch>`. If that package is missing, calls throw `capability/not-implemented` instead of returning fabricated results.
- The document is the state: each edit is written atomically, and there is no undo stack.

## Related

- [OpenCAE README](../../../README.md) — product overview and examples
- [Capability matrix](../../../documentation/capabilities.md) — supported features and numerical methods
- [CLI reference](../../../documentation/cli.md) — every command and flag
