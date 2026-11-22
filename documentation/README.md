# OpenCAE documentation

Maintainer and reference documentation. The user-facing landing page is the
repository [README](../README.md); the npm registry page is the
[`@wae/open-cae` package README](../projects/packages/open-cae/README.md).

| Document | Contents |
|----------|----------|
| [architecture.md](architecture.md) | Structural model, module boundaries, model → native addon → SDK/CLI flow, determinism and error semantics, session/undo ownership, workspace layout |
| [capabilities.md](capabilities.md) | Capability matrix, numerical methods, validation and test map, limitations |
| [cli.md](cli.md) | Full `open-cae` command reference with flags and examples |
| [development.md](development.md) | Environment, build, test, native addon staging, CI and release |
| [AGENTS.md](../AGENTS.md) | Rules for agents working in this repository |

## Source of truth

The Rust `OpenCae` session in `projects/crates/cae-model` is the only
implementation of document I/O, validation, solving and export. The Node-API
crate and the TypeScript SDK/CLI only map values and arguments onto it. When
this documentation and the code disagree, the code wins; update the document
in the same change.
