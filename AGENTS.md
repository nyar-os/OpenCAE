# AGENTS.md

Guidance for agents working in the OpenCAE repository.

## Purpose

OpenCAE is a 2D frame/truss and 3D truss structural-analysis product. A
document is one `.ocae` JSON file; the product exposes it through a Rust
session, a Node-API addon, a TypeScript SDK and the `open-cae` CLI.

## Documentation map

| File | Audience |
|------|----------|
| [README.md](README.md) | Product landing page |
| [projects/packages/open-cae/README.md](projects/packages/open-cae/README.md) | npm registry page |
| [documentation/architecture.md](documentation/architecture.md) | Module boundaries and data flow |
| [documentation/capabilities.md](documentation/capabilities.md) | Supported features, numerical methods, tests, limitations |
| [documentation/cli.md](documentation/cli.md) | Command and flag reference |
| [documentation/development.md](documentation/development.md) | Build, test, native staging, release |
| [documentation/README.md](documentation/README.md) | Maintainer doc index |

## Architecture invariants

1. **The Rust session is the only implementation.** Document I/O, editing,
   validation, solving and CSV/VTK/Markdown/HTML export live in
   `projects/crates/cae-model`. Do not reimplement model logic elsewhere.
2. **The Node-API layer maps values only.** Each `projects/crates/cae-napi`
   export accepts a path or JSON request and returns a JSON string. It must not
   add domain rules, defaults that change results, or caching.
3. **The TypeScript SDK/CLI must not duplicate domain logic.** It parses
   arguments, calls native functions, formats output and maps error codes to
   exit codes (`capability/*` → `2`, other failures → `1`).
4. **The capability matrix is the only support claim.** If a feature is not
   marked supported in `documentation/capabilities.md` and
   `CAPABILITIES_JSON`, it is not supported, regardless of an API name.
5. **Unsupported work fails explicitly.** Return `capability/not-implemented`
   (or the specific stable code) instead of approximating silently.
6. **Edits persist atomically.** Every editing call writes the document with
   the temporary-file-and-rename path. There is no undo stack.
7. **Reports stay deterministic.** Markdown/HTML contain no timestamps.
8. **Units are never converted.** `mm` documents pair with N, MPa, t/mm³ and
   tonnes; `m` documents pair with N, Pa, kg/m³ and kilograms.

## Verified commands

```shell
pnpm install
pnpm run build:ts
pnpm --filter @wae/open-cae run typecheck

cargo fmt -p cae-model -p open-cae-napi
cargo test -p cae-model -p open-cae-napi --offline
cargo clippy -p cae-model -p open-cae-napi --offline --all-targets -- -D warnings
```

`pnpm run build` performs the full native → TypeScript → staging build.
`pnpm run build:napi` builds only the addon and `pnpm run stage:native`
copies it into the platform packages. The addon is not committed.

## Change checklist

- Search the repository before assuming a command exists; verify flags against
  `projects/packages/open-cae/src/cli.ts`.
- Add or update a test for every numerical or behavioral change.
- Keep the Rust session, Node-API surface, SDK types and CLI in sync when a
  capability is added or removed.
- Update `documentation/capabilities.md`, `documentation/cli.md` and
  `CAPABILITIES_JSON` when the support claim changes.
- Update the package README only for registry-facing changes (exports, usage,
  constraints). Keep product narrative in the root README.
- Never put build, monorepo, publish or architecture detail in either README.
- Run the checks above plus `git diff --check` before committing.
- Stage explicit paths; never `git add -A`. Do not push unless asked.

## Commit style

Use a one-line subject with an emoji prefix:

- `✨ Add ...` — new capability or user-visible feature
- `🐛 Fix ...` — bug fix
- `📝 ...` — documentation, comments or prose

Keep the subject imperative and specific. If a change spans documentation and
behavior, split it into separate commits.
