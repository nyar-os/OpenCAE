/**
 * Abaqus `.inp` import through the SDK and the CLI.
 *
 * The input file is copied into a temporary directory so the checked-in fixture
 * stays read-only. The test skips when `dist/` or the native addon is absent.
 */

import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { copyFile, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import test from 'node:test';

const run = promisify(execFile);
const here = dirname(fileURLToPath(import.meta.url));
const fixture = join(here, 'fixtures', 'abaqus-import.inp');

let sdk;
try {
    sdk = await import('../dist/index.js');
} catch {
    sdk = undefined;
}

async function nativeAvailable(OpenCAE, isOpenCAEError) {
    try {
        await new OpenCAE().capabilities();
        return true;
    } catch (error) {
        if (isOpenCAEError(error) && error.code === 'capability/not-implemented') {
            return false;
        }
        throw error;
    }
}

function approx(actual, expected, relative) {
    const scale = Math.max(Math.abs(actual), Math.abs(expected), 1e-300);
    assert.ok(
        Math.abs(actual - expected) <= relative * scale,
        `expected ${expected}, got ${actual} (relative error ${Math.abs(actual - expected) / scale})`,
    );
}

test('Abaqus import maps the subset, lists losses and the model solves', { skip: sdk === undefined }, async (t) => {
    const { OpenCAE, isOpenCAEError } = sdk;
    if (!(await nativeAvailable(OpenCAE, isOpenCAEError))) {
        t.skip('native addon is not installed');
        return;
    }

    const directory = await mkdtemp(join(tmpdir(), 'ocae-abaqus-'));
    const session = new OpenCAE();
    try {
        const source = join(directory, 'model.inp');
        const target = join(directory, 'model.ocae');
        await copyFile(fixture, source);

        const capabilities = await session.capabilities();
        assert.equal(capabilities.io.inp, 'supported');

        const report = await session.importAbaqus(source, target, { units: 'mm' });
        assert.equal(report.format, 'open-cae-abaqus-import');
        assert.equal(report.version, 1);
        assert.equal(report.document.nodes, 2);
        assert.equal(report.document.elements, 1);
        assert.ok(report.losses.some((loss) => loss.code === 'import/abaqus-unsupported-keyword'));
        assert.ok(report.losses.every((loss) => loss.line >= 1 && loss.detail.length > 0));
        assert.equal((await readFile(target)).length > 0, true);

        const document = await session.open(target);
        const solution = document.solve();
        approx(solution.nodes.find((node) => node.id === '2').ux, (1000 * 1000) / (210000 * 100), 1e-10);
    } finally {
        await rm(directory, { recursive: true, force: true });
    }
});

test('CLI import prints the report with its loss list', { skip: sdk === undefined }, async (t) => {
    const { OpenCAE, isOpenCAEError } = sdk;
    if (!(await nativeAvailable(OpenCAE, isOpenCAEError))) {
        t.skip('native addon is not installed');
        return;
    }

    const directory = await mkdtemp(join(tmpdir(), 'ocae-abaqus-cli-'));
    try {
        const source = join(directory, 'model.inp');
        const target = join(directory, 'model.ocae');
        await copyFile(fixture, source);

        const cli = join(here, '..', 'dist', 'cli.js');
        const { stdout } = await run(process.execPath, [cli, 'import', source, target, '--units', 'mm']);
        const report = JSON.parse(stdout);
        assert.equal(report.format, 'open-cae-abaqus-import');
        assert.equal(report.document.elements, 1);
        assert.ok(report.losses.some((loss) => loss.keyword === 'Heading'));
    } finally {
        await rm(directory, { recursive: true, force: true });
    }
});
