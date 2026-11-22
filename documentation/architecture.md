# Architecture

The API is the core and the CLI only parses arguments.

The `OpenCae` session in `projects/crates/cae-model` is the only
implementation of document I/O, editing, validation, solving and
CSV/VTK/Markdown/HTML export. The Node-API crate and the TypeScript SDK/CLI
map values and arguments onto it.

## Entry points

| Entry point | Location | Responsibility |
|-------------|----------|----------------|
| Rust session API | `projects/crates/cae-model/src/session.rs` | All model logic; `OpenCaeOptions`, `OpenCae` |
| Node-API functions | `projects/crates/cae-napi/src/lib.rs` | Each export does one thing and returns a JSON string |
| TypeScript SDK | `projects/packages/open-cae/src` | `new OpenCAE()` session, `CaeDocument` documents, thin `open-cae` CLI |

The CLI only parses arguments, calls the API, prints results and maps exit
codes. It duplicates no model logic, so validation, solution and export
results are always consistent across the SDK, CLI and native functions.

## Structural model

A model is one UTF-8 JSON document with `format: "open-cae-truss"` and
`version: 1`. It stores:

- `units` (`mm` or `m`) and `dimension` (`2` or `3`);
- one global material with optional parametric `section`;
- `nodes` with unique ids and coordinates;
- `elements` that reference two nodes and are `truss` (default) or `beam`;
- `supports` with rigid flags, prescribed displacements, springs or an
  inclined `direction`;
- nodal `loads`, an optional gravity vector and optional `combinations`;
- a `revision` counter incremented by edits.

Element-level overrides (`youngsModulus`, `area`), beam `inertia`,
distributed loads, a uniform temperature change, a through-depth temperature
gradient, end releases and `tensionOnly` live on the element. The model struct
in `projects/crates/cae-model/src/model.rs` is authoritative for the exact
field list; the supported subset is listed in the
[capability matrix](capabilities.md).

## Flow

```text
TS SDK / CLI (arguments, strings, numbers)
        │  JSON request string per native call
        ▼
Node-API crate (open-cae-napi)   value mapping only
        │
        ▼
OpenCae session (cae-model)      edit → validate → solve → export
        │  write_atomic (temporary file + rename)
        ▼
<name>.ocae on disk            single source of truth
```

- `new OpenCAE()` opens nothing: no network, no disk scan, no file. Native
  initialisation happens inside async `create()` / `open()`.
- Every editing call mutates the in-memory model, writes the document
  atomically and returns a fresh `DocumentSummary`.
- Query and solve calls read the file by path, so the SDK, the CLI and the
  addon cannot drift apart.

## Determinism and errors

- Markdown and HTML reports contain no timestamps, so repeated exports are
  byte-identical. The determinism is asserted in `tests/beam_modal.rs`.
- Errors carry stable `code`s (for example `model/mechanism`,
  `session/no-document`, `capability/not-implemented`). The CLI maps any
  `capability/*` code to exit code `2` and every other error to exit code `1`;
  success is `0`.
- An unsupported capability fails explicitly instead of degrading silently.
- The solver is deterministic: dense assembly, Gaussian elimination with
  partial pivoting and Jacobi eigen-solves have no random or time-dependent
  step.

## Session ownership and undo

- The Rust `OpenCae` session owns **at most one document at a time**. `create`
  and `open` replace the open document.
- The TypeScript `OpenCAE` session tracks the `CaeDocument` objects it created
  or opened; `dispose()` closes them and is idempotent. Each `CaeDocument`
  caches the last summary (`id`, `units`, `dimension`, `revision`).
- There is no undo stack or in-memory transaction log. The document on disk
  after each edit is the state. Reverting means editing the model again or
  restoring the JSON file from version control.
- `create()` refuses to replace an existing file (`file/exists`); delete or
  choose another path first.

## Workspace layout

```text
projects/
  crates/cae-model/      core model, validation, solver, export and `ocae` CLI
    src/session.rs       OpenCae session API (all editing/validation/solving/export)
    src/model.rs         data model and document I/O
    src/library.rs       material presets and section properties
    src/validate.rs      validation rules and mechanism detection
    src/solver.rs        stiffness, DOF activation, elimination, Jacobi modal/buckling,
                         harmonic response, construction stages, truss Newton,
                         tension-only active set, internal forces and stress recovery
    src/export.rs        CSV / VTK / Markdown / HTML export and atomic writes
    src/bin/ocae.rs      thin Rust CLI
    tests/               analytical and regression tests
  crates/cae-napi/       `open-cae-napi` orthogonal Node-API exports
  packages/open-cae/     `@wae/open-cae` TypeScript SDK and `open-cae` CLI
    src/open-cae.ts      `new OpenCAE()` session
    src/document.ts      `CaeDocument` document
    src/service.ts       native calls and error translation
    src/native.ts        platform addon discovery
    src/cli.ts           thin CLI wrapper
    locales/en-US/cli.json  CLI text
  packages/open-cae-<platform>-<arch>/  prebuilt addon sidecars (not public imports)
  sites/homepage/        static project site
```

The root `Cargo.toml` uses a workspace. Because Cargo treats the intermediate
directory `projects/crates` under `projects/*` as a member, the root manifest
explicitly excludes it and explicitly includes `projects/crates/cae-model` and
`projects/crates/cae-napi`. `wae` is imported as a git dependency pinned to a
fixed rev with default features off, so the Node-API facade does not pull
WebView platform dependencies.

The native addon is built from `open-cae-napi` and staged into the
`@wae/open-cae-<platform>-<arch>` packages. The SDK resolves it from
`process.platform` / `process.arch` with no path configuration. When the
matching platform package is absent, calls throw `capability/not-implemented`
rather than fabricating results.
