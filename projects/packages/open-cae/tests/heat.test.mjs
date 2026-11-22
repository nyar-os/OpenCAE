/**
 * Steady-state heat-conduction smoke test.
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

test('heat solves fixed temperatures and reports element flux', { skip: sdk === undefined }, async (t) => {
    const { OpenCAE, isOpenCAEError } = sdk;
    if (!(await nativeAvailable(OpenCAE, isOpenCAEError))) {
        t.skip('native addon is not installed');
        return;
    }

    const directory = await mkdtemp(join(tmpdir(), 'ocae-heat-'));
    const session = new OpenCAE();
    try {
        const capabilities = await session.capabilities();
        assert.equal(capabilities.solver.thermal, 'supported');
        assert.equal(capabilities.solver.heatTransfer, 'supported');

        const document = await session.create(join(directory, 'model.ocae'), { units: 'm' });
        document.setMaterial({ youngsModulus: 210e9, area: 1e-4, density: 0, conductivity: 50 });
        document.addNode({ id: 'n1', x: 0, y: 0 });
        document.addNode({ id: 'n2', x: 1, y: 0 });
        document.addElement({ id: 'e1', from: 'n1', to: 'n2' });
        document.setNodeTemperature({ id: 'n1', temperature: 100 });
        document.setNodeTemperature({ id: 'n2', temperature: 0 });

        const fixed = document.heat();
        assert.equal(fixed.format, 'open-cae-heat-report');
        assert.equal(fixed.analysis, 'heat');
        assert.equal(fixed.ok, true);
        approx(fixed.temperatures[0].temperature, 100, 1e-12);
        approx(fixed.elementFlux[0].flux, 0.5, 1e-12);
        approx(fixed.reactions[0].heat, 0.5, 1e-12);
        approx(fixed.reactions[1].heat, -0.5, 1e-12);

        document.setNodeTemperature({ id: 'n2', temperature: null });
        document.setNodeHeat({ id: 'n2', heat: 10 });
        const loaded = document.heat();
        approx(loaded.temperatures[1].temperature, 2100, 1e-12);
        approx(loaded.elementFlux[0].flux, -10, 1e-12);
        approx(loaded.reactions[0].heat, -10, 1e-12);

        const noBoundary = await session.create(join(directory, 'no-boundary.ocae'), { units: 'm' });
        noBoundary.setMaterial({ youngsModulus: 210e9, area: 1e-4, density: 0, conductivity: 50 });
        noBoundary.addNode({ id: 'n1', x: 0, y: 0 });
        noBoundary.addNode({ id: 'n2', x: 1, y: 0 });
        noBoundary.addElement({ id: 'e1', from: 'n1', to: 'n2' });
        assert.throws(
            () => noBoundary.heat(),
            (error) => isOpenCAEError(error) && error.code === 'heat/no-fixed-temperature',
        );

        const noConductivity = await session.create(join(directory, 'no-conductivity.ocae'), { units: 'm' });
        noConductivity.setMaterial({ youngsModulus: 210e9, area: 1e-4, density: 0 });
        noConductivity.addNode({ id: 'n1', x: 0, y: 0 });
        noConductivity.addNode({ id: 'n2', x: 1, y: 0 });
        noConductivity.addElement({ id: 'e1', from: 'n1', to: 'n2' });
        noConductivity.setNodeTemperature({ id: 'n1', temperature: 100 });
        assert.throws(
            () => noConductivity.heat(),
            (error) => isOpenCAEError(error) && error.code === 'material/invalid-conductivity',
        );
    } finally {
        await session.dispose();
        await rm(directory, { recursive: true, force: true });
    }
});
