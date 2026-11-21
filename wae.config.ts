/**
 * Build and release configuration for OpenCAE.
 *
 * Pure literals only: `wae` parses this file statically and never executes it.
 * Build order is fixed: native → ts → staging.
 *
 * The package lives under `projects/packages`; the native addon is built from
 * the `open-cae-napi` crate and staged into the per-platform packages, so
 * `@wae/open-cae` resolves the addon without any environment variable.
 */
import { defineConfig } from '@wae/wae';

export default defineConfig({
    native: {
        crate: 'open-cae-napi',
        cdylib: 'open_cae_napi',
        targets: [
            {
                platform: 'win32',
                arch: 'x64',
                triple: 'x86_64-pc-windows-msvc',
                package: '@wae/open-cae-win32-x64',
                nodeFile: 'open-cae.win32-x64.node',
                dir: 'projects/packages/open-cae-win32-x64',
            },
            {
                platform: 'darwin',
                arch: 'x64',
                triple: 'x86_64-apple-darwin',
                package: '@wae/open-cae-darwin-x64',
                nodeFile: 'open-cae.darwin-x64.node',
                dir: 'projects/packages/open-cae-darwin-x64',
            },
            {
                platform: 'darwin',
                arch: 'arm64',
                triple: 'aarch64-apple-darwin',
                package: '@wae/open-cae-darwin-arm64',
                nodeFile: 'open-cae.darwin-arm64.node',
                dir: 'projects/packages/open-cae-darwin-arm64',
            },
            {
                platform: 'linux',
                arch: 'x64',
                triple: 'x86_64-unknown-linux-gnu',
                package: '@wae/open-cae-linux-x64',
                nodeFile: 'open-cae.linux-x64.node',
                dir: 'projects/packages/open-cae-linux-x64',
            },
            {
                platform: 'linux',
                arch: 'arm64',
                triple: 'aarch64-unknown-linux-gnu',
                package: '@wae/open-cae-linux-arm64',
                nodeFile: 'open-cae.linux-arm64.node',
                dir: 'projects/packages/open-cae-linux-arm64',
            },
        ],
    },
    packages: [
        {
            name: '@wae/open-cae-win32-x64',
            dir: 'projects/packages/open-cae-win32-x64',
            os: ['win32'],
            cpu: ['x64'],
            main: 'open-cae.win32-x64.node',
        },
        {
            name: '@wae/open-cae-darwin-x64',
            dir: 'projects/packages/open-cae-darwin-x64',
            os: ['darwin'],
            cpu: ['x64'],
            main: 'open-cae.darwin-x64.node',
        },
        {
            name: '@wae/open-cae-darwin-arm64',
            dir: 'projects/packages/open-cae-darwin-arm64',
            os: ['darwin'],
            cpu: ['arm64'],
            main: 'open-cae.darwin-arm64.node',
        },
        {
            name: '@wae/open-cae-linux-x64',
            dir: 'projects/packages/open-cae-linux-x64',
            os: ['linux'],
            cpu: ['x64'],
            main: 'open-cae.linux-x64.node',
        },
        {
            name: '@wae/open-cae-linux-arm64',
            dir: 'projects/packages/open-cae-linux-arm64',
            os: ['linux'],
            cpu: ['arm64'],
            main: 'open-cae.linux-arm64.node',
        },
    ],
    build: {
        ts: { packages: ['@wae/open-cae'] },
        outputs: ['projects/packages/open-cae/dist/cli.js', 'projects/packages/open-cae/dist/index.js'],
    },
});
