/**
 * SVG engineering drawing export smoke test.
 *
 * The SDK imports the built `dist/` output and the optional native addon. When
 * either is absent the test skips, so a plain checkout stays verifiable. The
 * cantilever has closed-form results, so the sheets are checked against the
 * analytic tip deflection and the stable drawing error codes.
 */
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
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

function cantilever(session, directory, name) {
    return session.create(join(directory, `${name}.ocae`)).then((document) => {
        document.setMaterial({ youngsModulus: 210000, area: 100, density: 7.85e-9 });
        document.addNode({ id: 'n1', x: 0, y: 0 });
        document.addNode({ id: 'n2', x: 1000, y: 0 });
        document.addElement({ id: 'b1', from: 'n1', to: 'n2', kind: 'beam', inertia: 1e6 });
        document.addSupport({ node: 'n1', dx: true, dy: true, rz: true });
        document.addLoad({ node: 'n2', fy: -1000 });
        return document;
    });
}

test('drawing exports write deterministic sheets', { skip: sdk === undefined }, async (t) => {
    const { OpenCAE, isOpenCAEError } = sdk;
    if (!(await nativeAvailable(OpenCAE, isOpenCAEError))) {
        t.skip('native addon is not installed');
        return;
    }
    const directory = await mkdtemp(join(tmpdir(), 'ocae-drawings-'));
    const session = new OpenCAE();
    try {
        const capabilities = await session.capabilities();
        assert.equal(capabilities.io.svgForces, 'supported');
        assert.equal(capabilities.io.svgDisplacement, 'supported');

        const document = await cantilever(session, directory, 'beam');

        const forcesPath = join(directory, 'forces.svg');
        document.export('svg', forcesPath, { drawing: 'forces' });
        const forces = await readFile(forcesPath, 'utf8');
        assert.match(forces, /OpenCAE shear and moment diagrams/);

        const displacementPath = join(directory, 'displacements.svg');
        document.export('svg', displacementPath, { drawing: 'displacement' });
        const displacement = await readFile(displacementPath, 'utf8');
        assert.match(displacement, /OpenCAE nodal displacement vectors/);
        // Analytic tip deflection PL³/(3EI) = 1000·1000³/(3·210000·10⁶) = 1.587 mm.
        assert.match(displacement, /max \|u\| = 1\.587 mm/);

        document.export('svg', displacementPath, { drawing: 'displacement' });
        const repeated = await readFile(displacementPath, 'utf8');
        assert.equal(repeated, displacement, 'repeated drawing exports must be byte-identical');

        assert.throws(
            () => document.export('svg', join(directory, 'bad.svg'), { drawing: 'pressure' }),
            (error) => isOpenCAEError(error) && error.code === 'export/invalid-drawing',
        );
    } finally {
        await session.dispose();
        await rm(directory, { recursive: true, force: true });
    }
});
