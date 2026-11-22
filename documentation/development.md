# Development

## Environment

| Tool | Version used |
|------|--------------|
| Node.js | 24 in CI; `@wae/open-cae` declares `>=22.6` |
| pnpm | 10.28.0 (`packageManager` in the root manifest) |
| Rust | nightly with `rustfmt` and `clippy` (CI toolchain) |

No global tooling or path configuration is needed to run the SDK; the native
addon is resolved from the installed platform package.

## Install

```shell
pnpm install
```

## Build

Build order is fixed: native → TypeScript → staging.

| Command | Effect |
|---------|--------|
| `pnpm run build` | `wae build`: builds the addon, compiles the SDK and stages the native file into the platform packages |
| `pnpm run build:napi` | `wae native build`: builds only the Node-API addon |
| `pnpm run stage:native` | `wae native stage`: copies the built addon into the platform packages |
| `pnpm run build:ts` | `tsc -p projects/packages/open-cae/tsconfig.json`, writing `dist/` |
| `pnpm run build:rust` | `cargo build --workspace` |
| `pnpm run typecheck` | Typechecks every package under `projects/packages/*` |

`dist/` and `*.node` are build outputs and are ignored by git.

## Test and check

```shell
cargo fmt -p cae-model -p open-cae-napi
cargo test -p cae-model -p open-cae-napi --offline
cargo clippy -p cae-model -p open-cae-napi --offline --all-targets -- -D warnings
pnpm --filter @wae/open-cae run typecheck
```

Root shortcuts:

| Command | Effect |
|---------|--------|
| `pnpm test` | `cargo test --release` |
| `pnpm run test:cae` | `cargo test -p cae-model --offline` |
| `pnpm run check:rust` | clippy with `-D warnings`, then the offline `cae-model` tests |
| `pnpm run fmt` | `cargo fmt --all` |

The analytical assertions and their tolerances are listed in
[capabilities.md](capabilities.md#validation-and-tests).

## Native addon staging

`wae.config.ts` maps five targets to platform packages:

```text
win32-x64     → projects/packages/open-cae-win32-x64
darwin-x64    → projects/packages/open-cae-darwin-x64
darwin-arm64  → projects/packages/open-cae-darwin-arm64
linux-x64     → projects/packages/open-cae-linux-x64
linux-arm64   → projects/packages/open-cae-linux-arm64
```

`wae native build` compiles the `open-cae-napi` cdylib and writes
`open-cae.<platform>-<arch>.node` into the matching directory. The SDK loads
`@wae/open-cae-<platform>-<arch>` from `process.platform` / `process.arch`.
No `.node` file is committed; the sidecar packages ship the file produced by
the release build.

When the matching platform package is not installed, SDK calls throw
`capability/not-implemented` instead of fabricating results. To debug a
self-built binary, point `OPEN_CAE_NATIVE` at the `.node` file.

## Continuous integration

| Workflow | Trigger | Steps |
|----------|---------|-------|
| `.github/workflows/check-rust.yml` | push/PR to `master`, `dev` | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |
| `.github/workflows/check-typescript.yml` | push/PR to `master`, `dev` | `pnpm install --frozen-lockfile`, `pnpm typecheck`, `pnpm run build:ts` |
| `.github/workflows/publish-npm.yml` | tag `v*` or manual dispatch with a tag | native matrix build, TypeScript build, `nifty publish` |

## Release and publish

- Publishing is tag-driven (`v*`) and uses npm Trusted Publisher (OIDC); the
  workflow declares `id-token: write` and the `NPM_PUBLISH` environment.
- The `native` job builds each target with
  `npx -y -p @wae/wae@0.0.6 wae native build --target <target>` and uploads
  the `.node` artifact; the `publish` job copies artifacts into the platform
  package directories, runs `pnpm run build:ts` and publishes with
  `pnpm exec nifty publish --access public --version="${RELEASE_TAG#v}"`.
- `publish-npm.yml` is the sole npm trusted workflow; keep the filename and
  the repository (`nyar-os/OpenCAE`) unchanged.
- Local `nifty publish --placeholder` is only for `0.0.0` stubs, not for real
  versions.

## Documentation maintenance

User-facing entry points are the root [README](../README.md) and the
[`@wae/open-cae` package README](../projects/packages/open-cae/README.md).
Maintainer details belong in `documentation/` and [AGENTS.md](../AGENTS.md),
not in either README.
