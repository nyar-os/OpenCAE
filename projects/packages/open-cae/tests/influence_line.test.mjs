/**
 * Influence-line smoke test.
 *
 * The simply supported beam has closed-form influence ordinates, so the test
 * checks the reaction line against `-t` and the midspan moment line against
 * `-L/4·min(t, 1-t)`, plus the stable error code for an unknown track.
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
    if (OpenCAE === undefined || isOpenCAEError === undefined) {
        return false;
    }
    const directory = await mkdtemp(join(tmpdir(), 'ocae-influence-capability-'));
    try {
        const session = new OpenCAE();
        try {
            const document = await session.create(join(directory, 'probe.ocae'));
            document.addNode({ id: 'n1', x: 0, y: 0 });
            document.addNode({ id: 'n2', x: 1000, y: 0 });
            document.addElement({ id: 'b1', from: 'n1', to: 'n2', kind: 'beam', inertia: 1e6 });
            document.addSupport({ node: 'n1', dx: true, dy: true });
            document.addSupport({ node: 'n2', dy: true });
            document.influenceLine({ track: 'b1', quantity: { kind: 'reaction', node: 'n2', component: 'fy' } });
            return true;
        } catch (error) {
            if (isOpenCAEError(error) && error.code === 'capability/not-implemented') {
                return false;
            }
            throw error;
        } finally {
            await session.dispose();
        }
    } finally {
        await rm(directory, { recursive: true, force: true });
    }
}

function approx(actual, expected, tolerance = 1e-9) {
    assert.ok(Math.abs(actual - expected) <= tolerance * Math.max(1, Math.abs(expected)), `expected ${expected}, got ${actual}`);
}

test('influence line reactions and section ordinates match the analytic solution', { skip: sdk === undefined }, async (t) => {
    const { OpenCAE, isOpenCAEError } = sdk;
    if (!(await nativeAvailable(OpenCAE, isOpenCAEError))) {
        t.skip('native addon is not installed');
        return;
    }
    const directory = await mkdtemp(join(tmpdir(), 'ocae-influence-'));
    const session = new OpenCAE();
    try {
        const document = await session.create(join(directory, 'model.ocae'));
        document.addNode({ id: 'n1', x: 0, y: 0 });
        document.addNode({ id: 'n2', x: 1000, y: 0 });
        document.addElement({ id: 'b1', from: 'n1', to: 'n2', kind: 'beam', inertia: 1e6 });
        document.addSupport({ node: 'n1', dx: true, dy: true });
        document.addSupport({ node: 'n2', dy: true });

        const reaction = document.influenceLine({
            track: 'b1',
            quantity: { kind: 'reaction', node: 'n2', component: 'fy' },
            points: 5,
        });
        assert.equal(reaction.format, 'open-cae-influence-line-report');
        assert.equal(reaction.analysis, 'influenceLine');
        assert.equal(reaction.track, 'b1');
        assert.equal(reaction.direction, 'y');
        assert.equal(reaction.points, 5);
        approx(reaction.length, 1000, 1e-12);
        reaction.samples.forEach((sample, index) => {
            approx(sample.position, index / 4, 1e-12);
            approx(sample.value, -sample.position, 1e-9);
        });
        approx(reaction.min.value, -1, 1e-9);
        approx(reaction.governing.value, -1, 1e-9);

        const moment = document.influenceLine({
            track: 'b1',
            quantity: { kind: 'section', position: 0.5, component: 'moment' },
            points: 5,
        });
        const expected = [0, -125, -250, -125, 0];
        moment.samples.forEach((sample, index) => approx(sample.value, expected[index], 1e-9));
        approx(moment.governing.value, -250, 1e-9);
        assert.deepEqual(moment.quantity, { kind: 'section', element: 'b1', position: 0.5, component: 'moment' });

        assert.throws(
            () => document.influenceLine({ track: 'missing', quantity: { kind: 'reaction', node: 'n2', component: 'fy' } }),
            (error) => isOpenCAEError(error) && error.code === 'element/unknown-id',
        );
    } finally {
        await session.dispose();
        await rm(directory, { recursive: true, force: true });
    }
});
