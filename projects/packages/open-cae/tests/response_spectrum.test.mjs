/**
 * Response-spectrum smoke test.
 *
 * The SDK imports the built `dist/` output and the optional native addon. When
 * either is absent the test skips, so a plain checkout stays verifiable.
 */

import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

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

test('response spectrum defaults to SRSS and supports CQC', { skip: sdk === undefined }, async (t) => {
    const { OpenCAE, isOpenCAEError } = sdk;
    if (!(await nativeAvailable(OpenCAE, isOpenCAEError))) {
        t.skip('native addon is not installed');
        return;
    }

    const directory = await mkdtemp(join(tmpdir(), 'ocae-spectrum-'));
    const session = new OpenCAE();
    try {
        const document = await session.create(join(directory, 'model.ocae'));
        document.setMaterial({ youngsModulus: 210_000, area: 100, density: 7.85e-9 });
        document.addNode({ id: 'n1', x: 0, y: 0 });
        document.addNode({ id: 'n2', x: 1000, y: 0 });
        document.addElement({ id: 'e1', from: 'n1', to: 'n2' });
        document.addSupport({ node: 'n1', dx: true, dy: true });
        document.addSupport({ node: 'n2', dy: true });

        const spectrum = [
            { period: 0, acceleration: 2 },
            { period: 10, acceleration: 2 },
        ];
        const expected = (2 * (7.85e-9 * 100 * 1000) / 2) / ((210_000 * 100) / 1000);

        const srss = document.responseSpectrum({ spectrum });
        assert.equal(srss.combination, 'srss');
        assert.equal(srss.damping, undefined);
        approx(srss.nodes[1].ux, expected, 1e-9);

        const cqc = document.responseSpectrum({ combination: 'cqc', damping: 0.05, spectrum });
        assert.equal(cqc.combination, 'cqc');
        assert.equal(cqc.damping, 0.05);
        assert.equal(cqc.nodes[1].ux, srss.nodes[1].ux);

        assert.throws(
            () => document.responseSpectrum({ combination: 'cqc', spectrum }),
            (error) => isOpenCAEError(error) && error.code === 'spectrum/invalid-damping',
        );
        assert.throws(
            () => document.responseSpectrum({ combination: 'complete', spectrum }),
            (error) => isOpenCAEError(error) && error.code === 'spectrum/invalid-combination',
        );
    } finally {
        await session.dispose();
        await rm(directory, { recursive: true, force: true });
    }
});
