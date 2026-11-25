#!/usr/bin/env node
/**
 * Command-line assembly only.
 *
 * Every command maps options onto one session operation of `OpenCAE`; editing,
 * validation, solving and export stay in the native core. The CLI owns the
 * command grammar, localized help text, printing and exit codes, and nothing
 * else. Commands are declared with `@wae/commander`; the usage strings quoted
 * in error messages, the JSON error envelope and the exit codes are the stable
 * contract and are kept verbatim.
 */

import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { createCli, type CommandContext, type ParsedOptions } from '@wae/commander';

import type { CaeDocument } from './document.js';
import { NotImplementedError, OpenCAEError, type OpenCAEErrorCode } from './errors.js';
import { OpenCAE } from './open-cae.js';
import type {
    Analysis,
    ContourQuantity,
    Dimension,
    DrawingKind,
    ElementKind,
    ExportFormat,
    InfluenceQuantity,
    PolygonPoint,
    ReactionComponent,
    Section,
    SectionComponent,
    SpectrumCombination,
    SpectrumPoint,
    Units,
} from './types.js';

const here = path.dirname(fileURLToPath(import.meta.url));

const cli = createCli('open-cae').locales(path.join(here, '..', 'locales')).intro('cli.intro');

// Exported for contract audits that import the built entry to inspect the
// command tree. Nothing runs until the file is executed as the program entry.
export { cli };

/** A product failure carrying a localized `cli.error.*` message. */
function failure(
    ctx: CommandContext,
    code: OpenCAEErrorCode,
    key: string,
    values?: Record<string, string>,
): OpenCAEError {
    return new OpenCAEError(code, ctx.t(`cli.error.${key}`, values));
}

/**
 * Run one command, translating thrown errors into the JSON envelope on stderr
 * and the documented exit codes: 2 for unimplemented capabilities, 1 for any
 * other failure, otherwise whatever the command returns.
 */
async function attempt(ctx: CommandContext, work: () => number | Promise<number>): Promise<number> {
    try {
        return await work();
    } catch (error) {
        const problem =
            error instanceof OpenCAEError
                ? error
                : new OpenCAEError('native/error', error instanceof Error ? error.message : String(error), {
                      cause: error,
                  });
        ctx.stderr(JSON.stringify({ code: problem.code, message: problem.message }, null, 2));
        return problem.code.startsWith('capability/') ? 2 : 1;
    }
}

/** The option accessors shared by every command handler. */
interface Bound {
    message(key: string, values?: Record<string, string>): string;
    file(usage: string): string;
    raw(name: string): string | undefined;
    has(name: string): boolean;
    required(name: string): string;
    number(name: string): number | undefined;
    requiredNumber(name: string): number;
    flag(name: string): boolean;
    units(): Units | undefined;
    dimension(): Dimension | undefined;
    section(): Section | undefined;
    direction(): readonly [number, number];
}

function bind(ctx: CommandContext, options: ParsedOptions): Bound {
    const raw = (name: string): string | undefined => {
        const value = options[name.slice(2)];
        return typeof value === 'string' ? value : undefined;
    };
    const has = (name: string): boolean => options[name.slice(2)] !== undefined;
    const required = (name: string): string => {
        const value = raw(name);
        if (value === undefined) throw failure(ctx, 'cli/missing-option', 'missingOption', { option: name });
        return value;
    };
    const number = (name: string): number | undefined => {
        const value = raw(name);
        if (value === undefined) return undefined;
        const parsed = Number(value);
        if (!Number.isFinite(parsed)) {
            throw failure(ctx, 'cli/invalid-number', 'invalidNumber', { option: name, value });
        }
        return parsed;
    };
    const requiredNumber = (name: string): number => {
        const value = number(name);
        if (value === undefined) throw failure(ctx, 'cli/missing-option', 'missingOption', { option: name });
        return value;
    };
    const units = (): Units | undefined => {
        const value = raw('--units');
        if (value === undefined) return undefined;
        if (value === 'mm' || value === 'm') return value;
        throw failure(ctx, 'cli/invalid-units', 'invalidUnits', { value });
    };
    const dimension = (): Dimension | undefined => {
        const value = raw('--dimension');
        if (value === undefined) return undefined;
        if (value === '2') return 2;
        if (value === '3') return 3;
        throw failure(ctx, 'cli/invalid-option', 'invalidDimension', { value });
    };
    const section = (): Section | undefined => {
        const shape = raw('--section');
        if (shape === undefined) {
            const geometry = [
                '--width',
                '--height',
                '--diameter',
                '--depth',
                '--flange-width',
                '--flange-thickness',
                '--web-thickness',
                '--wall',
                '--outer-diameter',
                '--inner-diameter',
            ];
            if (geometry.some((option) => has(option))) {
                throw failure(ctx, 'cli/missing-option', 'sectionShape');
            }
            return undefined;
        }
        if (shape === 'rectangle') {
            return { shape: 'rectangle', width: requiredNumber('--width'), height: requiredNumber('--height') };
        }
        if (shape === 'circle') {
            return { shape: 'circle', diameter: requiredNumber('--diameter') };
        }
        if (shape === 'i' || shape === 'i-section') {
            return {
                shape: 'i',
                depth: requiredNumber('--depth'),
                flangeWidth: requiredNumber('--flange-width'),
                flangeThickness: requiredNumber('--flange-thickness'),
                webThickness: requiredNumber('--web-thickness'),
            };
        }
        if (shape === 'box') {
            return {
                shape: 'box',
                width: requiredNumber('--width'),
                height: requiredNumber('--height'),
                wall: requiredNumber('--wall'),
            };
        }
        if (shape === 'tube') {
            return {
                shape: 'tube',
                outerDiameter: requiredNumber('--outer-diameter'),
                innerDiameter: requiredNumber('--inner-diameter'),
            };
        }
        if (shape === 'channel') {
            return {
                shape: 'channel',
                depth: requiredNumber('--depth'),
                flangeWidth: requiredNumber('--flange-width'),
                flangeThickness: requiredNumber('--flange-thickness'),
                webThickness: requiredNumber('--web-thickness'),
            };
        }
        throw failure(ctx, 'cli/invalid-option', 'unknownSection', { value: shape });
    };
    return {
        message: (key, values) => ctx.t(`cli.message.${key}`, values),
        file: (usage) => {
            const value = options._[0];
            if (value === undefined) throw failure(ctx, 'cli/missing-argument', 'missingArgument', { usage });
            return value;
        },
        raw,
        has,
        required,
        number,
        requiredNumber,
        flag: (name) => options[name.slice(2)] === true,
        units,
        dimension,
        section,
        direction: () => {
            const value = raw('--direction') ?? '';
            const parts = value.split(',').map((entry) => Number(entry.trim()));
            if (parts.length !== 2 || !parts.every((axis) => Number.isFinite(axis))) {
                throw failure(ctx, 'cli/invalid-option', 'invalidDirection', { value });
            }
            return [parts[0] as number, parts[1] as number];
        },
    };
}

function analysisOption(ctx: CommandContext, b: Bound, dimension: Dimension): Analysis {
    const analysis = b.raw('--analysis');
    switch (analysis) {
        case undefined:
        case 'static':
            break;
        case 'modal':
        case 'buckling':
        case 'tensionOnly':
        case 'tension-only':
            break;
        case 'nonlinear':
            throw new NotImplementedError('nonlinear analysis');
        case 'thermal':
            throw new NotImplementedError('thermal analysis');
        default:
            throw failure(ctx, 'cli/invalid-option', 'unknownAnalysis', { value: analysis });
    }
    const element = b.raw('--element');
    if (element !== undefined) {
        switch (element) {
            case 'truss':
            case 'beam':
            case 'beam3d':
            case 'frame':
                break;
            default:
                throw failure(ctx, 'cli/invalid-option', 'unknownElement', { value: element });
        }
    }
    const requested = b.dimension();
    if (requested !== undefined && requested !== dimension) {
        throw failure(ctx, 'cli/dimension-mismatch', 'dimensionMismatch', {
            requested: String(requested),
            dimension: String(dimension),
        });
    }
    return analysis === 'modal' || analysis === 'buckling' ? analysis : 'static';
}

function modesOption(ctx: CommandContext, b: Bound): number {
    const value = b.raw('--modes');
    if (value === undefined) {
        return 5;
    }
    const modes = Number(value);
    if (!Number.isInteger(modes) || modes < 1) {
        throw failure(ctx, 'cli/invalid-number', 'invalidModes', { value });
    }
    return modes;
}

function iterationsOption(ctx: CommandContext, b: Bound): number | undefined {
    const value = b.raw('--iterations');
    if (value === undefined) {
        return undefined;
    }
    const iterations = Number(value);
    if (!Number.isInteger(iterations) || iterations < 1) {
        throw failure(ctx, 'cli/invalid-number', 'invalidIterations', { value });
    }
    return iterations;
}

function stationsOption(ctx: CommandContext, b: Bound): number {
    const value = b.raw('--stations');
    if (value === undefined) {
        return 11;
    }
    const stations = Number(value);
    if (!Number.isInteger(stations) || stations < 2) {
        throw failure(ctx, 'cli/invalid-number', 'invalidStations', { value });
    }
    return stations;
}

function pointsOption(ctx: CommandContext, b: Bound): number | undefined {
    const value = b.raw('--points');
    if (value === undefined) {
        return undefined;
    }
    const points = Number(value);
    if (!Number.isInteger(points) || points < 2) {
        throw failure(ctx, 'cli/invalid-number', 'invalidPoints', { value });
    }
    return points;
}

function numberListOption(ctx: CommandContext, b: Bound, name: string): number[] {
    const raw = b.required(name);
    const values = raw
        .split(',')
        .map((entry) => entry.trim())
        .filter((entry) => entry.length > 0)
        .map((entry) => Number(entry));
    if (values.length === 0 || !values.every((value) => Number.isFinite(value))) {
        throw failure(ctx, 'cli/invalid-number', 'invalidNumberList', { option: name, value: raw });
    }
    return values;
}

function exportFormat(ctx: CommandContext, value: string): ExportFormat {
    if (value === 'csv' || value === 'vtk' || value === 'md' || value === 'svg') {
        return value;
    }
    if (value === 'inp' || value === 'abaqus' || value === 'step' || value === 'stp') {
        throw new NotImplementedError(`${value} export`);
    }
    throw failure(ctx, 'cli/unknown-format', 'unknownFormat', { format: value });
}

function importFormat(ctx: CommandContext, value: string | undefined, file: string): void {
    if (value !== undefined) {
        if (value === 'abaqus' || value === 'inp') {
            return;
        }
        if (value === 'step' || value === 'stp') {
            throw new NotImplementedError('STEP import');
        }
        throw failure(ctx, 'cli/unknown-format', 'unknownImportFormat', { format: value });
    }
    if (path.extname(file).toLowerCase() !== '.inp') {
        throw failure(ctx, 'import/abaqus-unsupported-format', 'unsupportedImportFormat', { file });
    }
}

function contourOption(ctx: CommandContext, b: Bound): ContourQuantity | undefined {
    const value = b.raw('--contour');
    if (value === undefined) {
        return undefined;
    }
    if (value === 'axial' || value === 'shear' || value === 'moment' || value === 'displacement') {
        return value;
    }
    throw failure(ctx, 'cli/invalid-option', 'unknownContour', { value });
}

function drawingOption(ctx: CommandContext, b: Bound): DrawingKind | undefined {
    const value = b.raw('--drawing');
    if (value === undefined) {
        return undefined;
    }
    if (value === 'forces' || value === 'displacement') {
        return value;
    }
    throw failure(ctx, 'cli/invalid-option', 'unknownDrawing', { value });
}

function parsePolygonPoints(ctx: CommandContext, value: string): readonly PolygonPoint[] {
    return value
        .split(';')
        .map((entry) => entry.trim())
        .filter((entry) => entry.length > 0)
        .map((entry) => {
            const parts = entry.split(',').map((part) => Number(part.trim()));
            if (parts.length !== 2 || !parts.every((part) => Number.isFinite(part))) {
                throw failure(ctx, 'cli/invalid-option', 'invalidPoint', { value: entry });
            }
            return [parts[0] as number, parts[1] as number] as const;
        });
}

function parseSpectrum(ctx: CommandContext, value: string): readonly SpectrumPoint[] {
    const points: SpectrumPoint[] = [];
    for (const item of value
        .split(',')
        .map((entry) => entry.trim())
        .filter((entry) => entry.length > 0)) {
        const separator = item.indexOf(':');
        if (separator < 0) {
            throw failure(ctx, 'cli/invalid-spectrum', 'invalidSpectrumPoint', { point: item });
        }
        const period = Number(item.slice(0, separator).trim());
        const acceleration = Number(item.slice(separator + 1).trim());
        if (!Number.isFinite(period) || !Number.isFinite(acceleration)) {
            throw failure(ctx, 'cli/invalid-spectrum', 'invalidSpectrumPoint', { point: item });
        }
        points.push({ period, acceleration });
    }
    if (points.length === 0) {
        throw failure(ctx, 'cli/invalid-spectrum', 'emptySpectrum', {});
    }
    return points;
}

function influenceDirectionOption(ctx: CommandContext, b: Bound): 'x' | 'y' {
    const value = b.raw('--direction') ?? 'y';
    if (value === 'x' || value === 'y') {
        return value;
    }
    throw failure(ctx, 'cli/invalid-option', 'unknownInfluenceDirection', { value });
}

function influenceComponentOption(ctx: CommandContext, quantityKind: string, b: Bound): string {
    const value = b.required('--component');
    if (quantityKind === 'reaction') {
        if (value === 'fx' || value === 'fy' || value === 'mz') {
            return value;
        }
        throw failure(ctx, 'cli/invalid-option', 'unknownReactionComponent', { value });
    }
    if (value === 'axial' || value === 'shear' || value === 'moment') {
        return value;
    }
    throw failure(ctx, 'cli/invalid-option', 'unknownSectionComponent', { value });
}

function spectrumAxisOption(ctx: CommandContext, b: Bound): 'x' | 'y' | 'z' {
    const value = b.raw('--axis') ?? 'x';
    if (value === 'x' || value === 'y' || value === 'z') {
        return value;
    }
    throw failure(ctx, 'cli/invalid-option', 'unknownSpectrumAxis', { value });
}

function spectrumCombinationOption(ctx: CommandContext, b: Bound): SpectrumCombination {
    const value = b.raw('--combination') ?? 'srss';
    if (value === 'srss' || value === 'cqc') {
        return value;
    }
    throw failure(ctx, 'spectrum/invalid-combination', 'unknownSpectrumCombination', { value });
}

/**
 * Parse a `--patterns` value such as `live=1.0,wind=1.5` into pattern factor
 * entries. Empty parts are ignored; a malformed part or an empty list is
 * rejected with `cli/invalid-option`.
 */
function parsePatternList(
    ctx: CommandContext,
    value: string,
): readonly { pattern: string; factor: number }[] {
    const entries: { pattern: string; factor: number }[] = [];
    for (const part of value.split(',')) {
        const trimmed = part.trim();
        if (trimmed.length === 0) {
            continue;
        }
        const separator = trimmed.indexOf('=');
        const pattern = separator === -1 ? '' : trimmed.slice(0, separator).trim();
        const factor = separator === -1 ? Number.NaN : Number(trimmed.slice(separator + 1).trim());
        if (pattern.length === 0 || !Number.isFinite(factor)) {
            throw failure(ctx, 'cli/invalid-option', 'invalidPatternList', { value });
        }
        entries.push({ pattern, factor });
    }
    if (entries.length === 0) {
        throw failure(ctx, 'cli/invalid-option', 'invalidPatternList', { value });
    }
    return entries;
}

/** One short-lived session per command, mirroring the process lifetime. */
async function withSession(work: (session: OpenCAE) => Promise<number>): Promise<number> {
    const session = new OpenCAE();
    try {
        return await work(session);
    } finally {
        await session.dispose();
    }
}

async function openDocument(session: OpenCAE, file: string): Promise<CaeDocument> {
    return session.open(file);
}

/** Group commands keep the old subcommand diagnostics as their action. */
function groupAction(group: string, expected: string) {
    return async (options: ParsedOptions, _args: string[], ctx: CommandContext): Promise<number> =>
        attempt(ctx, () => {
            const action = options._[0];
            if (action === undefined) {
                throw failure(ctx, 'cli/missing-argument', 'missingSubcommand', { group, expected });
            }
            throw failure(ctx, 'cli/unknown-command', 'unknownSubcommand', { group, action, expected });
        });
}

/**
 * The old grammar treated only `--long` tokens as options, so a negative value
 * like `--fy -10` was accepted as the option value. `@wae/commander` treats
 * every token starting with `-` as an option marker, so join such pairs into
 * `--fy=-10` before parsing.
 */
function joinNegativeValues(argv: string[]): string[] {
    const joined: string[] = [];
    for (let index = 0; index < argv.length; index += 1) {
        const token = argv[index] as string;
        const next = argv[index + 1];
        if (
            token.startsWith('--') &&
            token !== '--' &&
            !token.includes('=') &&
            next !== undefined &&
            /^-\d/.test(next)
        ) {
            joined.push(`${token}=${next}`);
            index += 1;
        } else {
            joined.push(token);
        }
    }
    return joined;
}

cli
    .command('capabilities', 'cli.cmd.capabilities')
    .actionCtx(async (_options, _args, ctx) =>
        attempt(ctx, async () =>
            withSession(async (session) => {
                ctx.stdout(JSON.stringify(await session.capabilities(), null, 2));
                return 0;
            }),
        ),
    );

cli
    .command('create', 'cli.cmd.create')
    .option('--units <u>', 'cli.opt.units')
    .option('--dimension <n>', 'cli.opt.dimension')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('create <file.ocae> [--units mm|m] [--dimension 2|3]');
            const units = b.units();
            const dimension = b.dimension();
            return withSession(async (session) => {
                const document = await session.create(file, { units, dimension });
                ctx.stdout(
                    b.message('created', {
                        path: file,
                        units: document.units ?? 'mm',
                        dimension: String(document.dimension),
                    }),
                );
                return 0;
            });
        }),
    );

const nodeCommand = cli
    .command('node', 'cli.cmd.node')
    .actionCtx(groupAction('node', 'add, update, mass, temperature, heat, remove, merge or displacement'));
nodeCommand
    .command('add', 'cli.cmd.node-add')
    .option('--id <id>', 'cli.opt.id')
    .option('--x <x>', 'cli.opt.x')
    .option('--y <y>', 'cli.opt.y')
    .option('--z <z>', 'cli.opt.z')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node add <file> --id <id> --x <x> --y <y> [--z <z>]');
            const id = b.required('--id');
            const x = b.requiredNumber('--x');
            const y = b.requiredNumber('--y');
            const z = b.number('--z') ?? 0;
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.addNode({ id, x, y, z });
                ctx.stdout(b.message('nodeAdded', { id, z: String(z), revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
nodeCommand
    .command('update', 'cli.cmd.node-update')
    .option('--id <id>', 'cli.opt.id')
    .option('--x <x>', 'cli.opt.x')
    .option('--y <y>', 'cli.opt.y')
    .option('--z <z>', 'cli.opt.z')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node update <file> --id <id> [--x <x>] [--y <y>] [--z <z>]');
            const id = b.required('--id');
            const x = b.number('--x');
            const y = b.number('--y');
            const z = b.number('--z');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.updateNode({ id, x, y, z });
                ctx.stdout(b.message('nodeUpdated', { id, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
nodeCommand
    .command('mass', 'cli.cmd.node-mass')
    .option('--id <id>', 'cli.opt.id')
    .option('--mass <mass>', 'cli.opt.mass')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node mass <file> --id <id> --mass <mass>');
            const id = b.required('--id');
            const mass = b.requiredNumber('--mass');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setNodeMass({ id, mass });
                ctx.stdout(b.message('nodeMassSet', { id, mass: String(mass), revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
nodeCommand
    .command('temperature', 'cli.cmd.node-temperature')
    .option('--id <id>', 'cli.opt.id')
    .option('--temperature <t>', 'cli.opt.temperature')
    .option('--clear', 'cli.opt.clear')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node temperature <file> --id <id> [--temperature <t>] [--clear]');
            const id = b.required('--id');
            const temperature = b.flag('--clear') ? null : b.requiredNumber('--temperature');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setNodeTemperature({ id, temperature });
                ctx.stdout(
                    b.message('nodeTemperatureSet', {
                        id,
                        temperature: temperature === null ? 'cleared' : String(temperature),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );
nodeCommand
    .command('heat', 'cli.cmd.node-heat')
    .option('--id <id>', 'cli.opt.id')
    .option('--heat <w>', 'cli.opt.heat')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node heat <file> --id <id> --heat <w>');
            const id = b.required('--id');
            const heat = b.requiredNumber('--heat');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setNodeHeat({ id, heat });
                ctx.stdout(b.message('nodeHeatSet', { id, heat: String(heat), revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
nodeCommand
    .command('remove', 'cli.cmd.node-remove')
    .option('--id <id>', 'cli.opt.id')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node remove <file> --id <id>');
            const id = b.required('--id');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.removeNode(id);
                ctx.stdout(b.message('nodeRemoved', { id, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
nodeCommand
    .command('merge', 'cli.cmd.node-merge')
    .option('--from <id>', 'cli.opt.from')
    .option('--into <id>', 'cli.opt.into')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node merge <file> --from <id> --into <id>');
            const from = b.required('--from');
            const into = b.required('--into');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.mergeNodes({ from, into });
                ctx.stdout(b.message('nodeMerged', { from, into, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
nodeCommand
    .command('displacement', 'cli.cmd.node-displacement')
    .option('--id <id>', 'cli.opt.id')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('node displacement <file> --id <id>');
            const id = b.required('--id');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                ctx.stdout(JSON.stringify(document.nodeDisplacement(id), null, 2));
                return 0;
            });
        }),
    );

const elementCommand = cli
    .command('element', 'cli.cmd.element')
    .actionCtx(groupAction('element', 'add, update, split, remove, forces, release, tension-only, gradient or span'));
elementCommand
    .command('add', 'cli.cmd.element-add')
    .option('--id <id>', 'cli.opt.id')
    .option('--from <node>', 'cli.opt.from')
    .option('--to <node>', 'cli.opt.to')
    .option('--E <E>', 'cli.opt.E')
    .option('--A <A>', 'cli.opt.A')
    .option('--kind <kind>', 'cli.opt.kind')
    .option('--I <I>', 'cli.opt.I')
    .option('--inertia-weak <Iy>', 'cli.opt.inertia-weak')
    .option('--torsion <J>', 'cli.opt.torsion')
    .option('--orientation-vector <x,y,z>', 'cli.opt.orientation-vector')
    .option('--orientation-angle <theta>', 'cli.opt.orientation-angle')
    .option('--w <w>', 'cli.opt.w')
    .option('--w2 <w2>', 'cli.opt.w2')
    .option('--distributed-load-z <wz>', 'cli.opt.distributed-load-z')
    .option('--distributed-load-z-end <wz2>', 'cli.opt.distributed-load-z-end')
    .option('--dt <dT>', 'cli.opt.dt')
    .option('--dt-grad <dT>', 'cli.opt.dt-grad')
    .option('--w-from <s>', 'cli.opt.w-from')
    .option('--w-to <s>', 'cli.opt.w-to')
    .option('--convection <h>', 'cli.opt.convection')
    .option('--perimeter <P>', 'cli.opt.perimeter')
    .option('--tension-only', 'cli.opt.tension-only')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'element add <file> --id <id> --from <node> --to <node> [--kind truss|beam|beam3d] [--I <I>] [--inertia-weak <Iy>] [--torsion <J>] [--orientation-vector <x,y,z>] [--orientation-angle <theta>] [--w <w>] [--w2 <w2>] [--distributed-load-z <wz>] [--distributed-load-z-end <wz2>] [--dt <dT>] [--dt-grad <dT>] [--w-from <s>] [--w-to <s>] [--convection <h>] [--perimeter <P>] [--tension-only]',
            );
            const id = b.required('--id');
            const from = b.required('--from');
            const to = b.required('--to');
            const youngsModulus = b.number('--E');
            const area = b.number('--A');
            const kind = elementKindOption(ctx, b);
            const inertia = b.number('--I');
            const inertiaWeak = b.number('--inertia-weak');
            const torsion = b.number('--torsion');
            const orientationVector = orientationVectorOption(ctx, b);
            const orientationAngle = b.number('--orientation-angle');
            const distributedLoad = b.number('--w');
            const distributedLoadEnd = b.number('--w2');
            const distributedLoadZ = b.number('--distributed-load-z');
            const distributedLoadZEnd = b.number('--distributed-load-z-end');
            const temperature = b.number('--dt');
            const temperatureGradient = b.number('--dt-grad');
            const distributedLoadFrom = b.number('--w-from');
            const distributedLoadTo = b.number('--w-to');
            const convection = b.number('--convection');
            const perimeter = b.number('--perimeter');
            if ((distributedLoadFrom === undefined) !== (distributedLoadTo === undefined)) {
                throw failure(ctx, 'cli/missing-option', 'missingOption', { option: '--w-from and --w-to' });
            }
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.addElement({
                    id,
                    from,
                    to,
                    youngsModulus,
                    area,
                    kind,
                    inertia,
                    inertiaWeak,
                    torsion,
                    orientationVector,
                    orientationAngle,
                    distributedLoad,
                    distributedLoadEnd,
                    distributedLoadZ,
                    distributedLoadZEnd,
                    distributedLoadFrom,
                    distributedLoadTo,
                    temperature,
                    temperatureGradient,
                    convection,
                    perimeter,
                    tensionOnly: b.flag('--tension-only') ? true : undefined,
                });
                ctx.stdout(b.message('elementAdded', { id, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
elementCommand
    .command('update', 'cli.cmd.element-update')
    .option('--id <id>', 'cli.opt.id')
    .option('--from <node>', 'cli.opt.from')
    .option('--to <node>', 'cli.opt.to')
    .option('--w <w>', 'cli.opt.w')
    .option('--w2 <w2>', 'cli.opt.w2')
    .option('--w-from <s>', 'cli.opt.w-from')
    .option('--w-to <s>', 'cli.opt.w-to')
    .option('--dt <dT>', 'cli.opt.dt')
    .option('--dt-grad <dT>', 'cli.opt.dt-grad')
    .option('--convection <h>', 'cli.opt.convection')
    .option('--perimeter <P>', 'cli.opt.perimeter')
    .option('--clear-load', 'cli.opt.clear-load')
    .option('--clear-temperature', 'cli.opt.clear-temperature')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'element update <file> --id <id> [--from <node>] [--to <node>] [--w <w>] [--w2 <w2>] [--w-from <s>] [--w-to <s>] [--dt <dT>] [--dt-grad <dT>] [--convection <h>] [--perimeter <P>] [--clear-load] [--clear-temperature]',
            );
            const id = b.required('--id');
            const from = b.raw('--from');
            const to = b.raw('--to');
            const distributedLoad = b.number('--w');
            const distributedLoadEnd = b.number('--w2');
            const distributedLoadFrom = b.number('--w-from');
            const distributedLoadTo = b.number('--w-to');
            const temperature = b.number('--dt');
            const temperatureGradient = b.number('--dt-grad');
            const convection = b.number('--convection');
            const perimeter = b.number('--perimeter');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.updateElement({
                    id,
                    from,
                    to,
                    distributedLoad,
                    distributedLoadEnd,
                    distributedLoadFrom,
                    distributedLoadTo,
                    temperature,
                    temperatureGradient,
                    convection,
                    perimeter,
                    clearLoad: b.flag('--clear-load') ? true : undefined,
                    clearTemperature: b.flag('--clear-temperature') ? true : undefined,
                });
                ctx.stdout(b.message('elementUpdated', { id, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
elementCommand
    .command('split', 'cli.cmd.element-split')
    .option('--id <id>', 'cli.opt.id')
    .option('--at <fraction>', 'cli.opt.at')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('element split <file> --id <id> --at <fraction>');
            const id = b.required('--id');
            const at = b.number('--at');
            if (at === undefined) {
                throw failure(ctx, 'cli/missing-option', 'missingOption', { option: '--at' });
            }
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const report = document.splitElement({ id, at });
                ctx.stdout(
                    b.message('elementSplit', {
                        id,
                        at: String(at),
                        node: report.node,
                        first: report.first,
                        second: report.second,
                        revision: String(report.revision),
                    }),
                );
                return 0;
            });
        }),
    );
elementCommand
    .command('remove', 'cli.cmd.element-remove')
    .option('--id <id>', 'cli.opt.id')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('element remove <file> --id <id>');
            const id = b.required('--id');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.removeElement(id);
                ctx.stdout(b.message('elementRemoved', { id, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
elementCommand
    .command('forces', 'cli.cmd.element-forces')
    .option('--id <id>', 'cli.opt.id')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('element forces <file> --id <id>');
            const id = b.required('--id');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                ctx.stdout(JSON.stringify(document.elementResult(id), null, 2));
                return 0;
            });
        }),
    );
elementCommand
    .command('release', 'cli.cmd.element-release')
    .option('--id <id>', 'cli.opt.id')
    .option('--start', 'cli.opt.start')
    .option('--end', 'cli.opt.end')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('element release <file> --id <id> [--start] [--end]');
            const id = b.required('--id');
            const start = b.flag('--start');
            const end = b.flag('--end');
            if (!start && !end) {
                throw failure(ctx, 'cli/missing-option', 'missingOption', { option: '--start or --end' });
            }
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setElementRelease({ id, start, end });
                ctx.stdout(
                    b.message('elementReleased', {
                        id,
                        start: String(start),
                        end: String(end),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );
elementCommand
    .command('tension-only', 'cli.cmd.element-tension-only')
    .option('--id <id>', 'cli.opt.id')
    .option('--clear', 'cli.opt.clear')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('element tension-only <file> --id <id> [--clear]');
            const id = b.required('--id');
            const tensionOnly = !b.flag('--clear');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setElementTensionOnly({ id, tensionOnly });
                ctx.stdout(
                    b.message('elementTensionOnly', {
                        id,
                        tensionOnly: String(tensionOnly),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );
elementCommand
    .command('gradient', 'cli.cmd.element-gradient')
    .option('--id <id>', 'cli.opt.id')
    .option('--value <dT>', 'cli.opt.value')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('element gradient <file> --id <id> [--value <dT>]');
            const id = b.required('--id');
            const gradient = b.number('--value');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setElementTemperatureGradient({ id, gradient: gradient ?? null });
                ctx.stdout(
                    gradient === undefined
                        ? b.message('elementGradientCleared', { id, revision: String(summary.revision) })
                        : b.message('elementGradientSet', {
                              id,
                              value: String(gradient),
                              revision: String(summary.revision),
                          }),
                );
                return 0;
            });
        }),
    );
elementCommand
    .command('span', 'cli.cmd.element-span')
    .option('--id <id>', 'cli.opt.id')
    .option('--start <s>', 'cli.opt.start')
    .option('--end <s>', 'cli.opt.end')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('element span <file> --id <id> [--start <s>] [--end <s>]');
            const id = b.required('--id');
            const start = b.number('--start');
            const end = b.number('--end');
            if ((start === undefined) !== (end === undefined)) {
                throw failure(ctx, 'cli/missing-option', 'spanPair');
            }
            const span = start === undefined || end === undefined ? null : ([start, end] as const);
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setElementLoadSpan({ id, span });
                ctx.stdout(
                    span === null
                        ? b.message('elementSpanCleared', { id, revision: String(summary.revision) })
                        : b.message('elementSpanSet', {
                              id,
                              start: String(start),
                              end: String(end),
                              revision: String(summary.revision),
                          }),
                );
                return 0;
            });
        }),
    );

function elementKindOption(ctx: CommandContext, b: Bound): ElementKind | undefined {
    const value = b.raw('--kind');
    if (value === undefined) {
        return undefined;
    }
    if (value === 'truss' || value === 'beam' || value === 'beam3d') {
        return value;
    }
    throw failure(ctx, 'cli/invalid-option', 'unknownElement', { value });
}

function orientationVectorOption(ctx: CommandContext, b: Bound): readonly [number, number, number] | undefined {
    const value = b.raw('--orientation-vector');
    if (value === undefined) {
        return undefined;
    }
    const parts = value.split(',').map((entry) => Number(entry.trim()));
    if (parts.length !== 3 || !parts.every((axis) => Number.isFinite(axis))) {
        throw failure(ctx, 'cli/invalid-option', 'invalidOrientationVector', { value });
    }
    return [parts[0] as number, parts[1] as number, parts[2] as number];
}

const materialCommand = cli.command('material', 'cli.cmd.material').actionCtx(groupAction('material', 'set or preset'));
materialCommand
    .command('set', 'cli.cmd.material-set')
    .option('--E <E>', 'cli.opt.E')
    .option('--A <A>', 'cli.opt.A')
    .option('--density <rho>', 'cli.opt.density')
    .option('--name <name>', 'cli.opt.material-name')
    .option('--allowable <s>', 'cli.opt.allowable')
    .option('--alpha <a>', 'cli.opt.alpha')
    .option('--conductivity <k>', 'cli.opt.conductivity')
    .option('--shear-modulus <G>', 'cli.opt.shear-modulus')
    .option('--poissons-ratio <nu>', 'cli.opt.poissons-ratio')
    .option('--section <shape>', 'cli.opt.section')
    .option('--width <w>', 'cli.opt.width')
    .option('--height <h>', 'cli.opt.height')
    .option('--diameter <d>', 'cli.opt.diameter')
    .option('--depth <d>', 'cli.opt.depth')
    .option('--flange-width <b>', 'cli.opt.flange-width')
    .option('--flange-thickness <t>', 'cli.opt.flange-thickness')
    .option('--web-thickness <t>', 'cli.opt.web-thickness')
    .option('--wall <t>', 'cli.opt.wall')
    .option('--outer-diameter <D>', 'cli.opt.outer-diameter')
    .option('--inner-diameter <d>', 'cli.opt.inner-diameter')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('material set <file> --E <E> --A <A> [--density <rho>] [--name <name>] [--shear-modulus <G>] [--poissons-ratio <nu>]');
            const youngsModulus = b.requiredNumber('--E');
            const area = b.requiredNumber('--A');
            const density = b.number('--density');
            const name = b.raw('--name');
            const allowableStress = b.number('--allowable');
            const alpha = b.number('--alpha');
            const conductivity = b.number('--conductivity');
            const shearModulus = b.number('--shear-modulus');
            const poissonsRatio = b.number('--poissons-ratio');
            const section = b.section();
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setMaterial({
                    youngsModulus,
                    area,
                    density,
                    name,
                    allowableStress,
                    alpha,
                    conductivity,
                    shearModulus,
                    poissonsRatio,
                    section,
                });
                ctx.stdout(
                    b.message('materialSet', {
                        youngsModulus: String(summary.material.youngsModulus),
                        area: String(summary.material.area),
                        density: String(summary.material.density),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );
materialCommand
    .command('preset', 'cli.cmd.material-preset')
    .option('--preset <name>', 'cli.opt.preset')
    .option('--area <A>', 'cli.opt.area')
    .option('--allowable <s>', 'cli.opt.allowable')
    .option('--section <shape>', 'cli.opt.section')
    .option('--width <w>', 'cli.opt.width')
    .option('--height <h>', 'cli.opt.height')
    .option('--diameter <d>', 'cli.opt.diameter')
    .option('--depth <d>', 'cli.opt.depth')
    .option('--flange-width <b>', 'cli.opt.flange-width')
    .option('--flange-thickness <t>', 'cli.opt.flange-thickness')
    .option('--web-thickness <t>', 'cli.opt.web-thickness')
    .option('--wall <t>', 'cli.opt.wall')
    .option('--outer-diameter <D>', 'cli.opt.outer-diameter')
    .option('--inner-diameter <d>', 'cli.opt.inner-diameter')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'material preset <file> --preset <name> [--section rectangle|circle|i|box|tube|channel] [--area A] [--allowable S]',
            );
            const preset = b.required('--preset');
            const area = b.number('--area');
            const allowableStress = b.number('--allowable');
            const section = b.section();
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.applyMaterialPreset({ preset, area, allowableStress, section });
                ctx.stdout(
                    b.message('materialPreset', {
                        preset,
                        youngsModulus: String(summary.material.youngsModulus),
                        area: String(summary.material.area),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );

cli
    .command('library', 'cli.cmd.library')
    .option('--units <u>', 'cli.opt.units')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units();
            return withSession(async (session) => {
                const library = await session.library(units);
                ctx.stdout(JSON.stringify(library, null, 2));
                return 0;
            });
        }),
    );

const sectionCommand = cli
    .command('section', 'cli.cmd.section')
    .actionCtx(
        groupAction(
            'section',
            'properties, polygon, i, tube, channel, angle, t, ellipse, triangle, z or principal',
        ),
    );
sectionCommand
    .command('properties', 'cli.cmd.section-properties')
    .option('--units <u>', 'cli.opt.units')
    .option('--section <shape>', 'cli.opt.section')
    .option('--width <w>', 'cli.opt.width')
    .option('--height <h>', 'cli.opt.height')
    .option('--diameter <d>', 'cli.opt.diameter')
    .option('--depth <d>', 'cli.opt.depth')
    .option('--flange-width <b>', 'cli.opt.flange-width')
    .option('--flange-thickness <t>', 'cli.opt.flange-thickness')
    .option('--web-thickness <t>', 'cli.opt.web-thickness')
    .option('--wall <t>', 'cli.opt.wall')
    .option('--outer-diameter <D>', 'cli.opt.outer-diameter')
    .option('--inner-diameter <d>', 'cli.opt.inner-diameter')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const section = b.section();
            if (section === undefined) {
                throw failure(ctx, 'cli/missing-option', 'missingSection', {});
            }
            return withSession(async (session) => {
                const report = await session.sectionProperties(section, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('polygon', 'cli.cmd.section-polygon')
    .option('--units <u>', 'cli.opt.units')
    .option('--points <x,y;...>', 'cli.opt.polygon-points')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const points = parsePolygonPoints(ctx, b.required('--points'));
            return withSession(async (session) => {
                const report = await session.sectionPolygon(points, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('i', 'cli.cmd.section-i')
    .option('--units <u>', 'cli.opt.units')
    .option('--height <h>', 'cli.opt.height')
    .option('--width <b>', 'cli.opt.width')
    .option('--web-thickness <tw>', 'cli.opt.web-thickness')
    .option('--flange-thickness <tf>', 'cli.opt.flange-thickness')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                height: b.requiredNumber('--height'),
                width: b.requiredNumber('--width'),
                webThickness: b.requiredNumber('--web-thickness'),
                flangeThickness: b.requiredNumber('--flange-thickness'),
            };
            return withSession(async (session) => {
                const report = await session.sectionI(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('tube', 'cli.cmd.section-tube')
    .option('--units <u>', 'cli.opt.units')
    .option('--height <h>', 'cli.opt.height')
    .option('--width <b>', 'cli.opt.width')
    .option('--thickness <t>', 'cli.opt.thickness')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                height: b.requiredNumber('--height'),
                width: b.requiredNumber('--width'),
                thickness: b.requiredNumber('--thickness'),
            };
            return withSession(async (session) => {
                const report = await session.sectionTube(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('channel', 'cli.cmd.section-channel')
    .option('--units <u>', 'cli.opt.units')
    .option('--height <h>', 'cli.opt.height')
    .option('--width <b>', 'cli.opt.width')
    .option('--web-thickness <tw>', 'cli.opt.web-thickness')
    .option('--flange-thickness <tf>', 'cli.opt.flange-thickness')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                height: b.requiredNumber('--height'),
                width: b.requiredNumber('--width'),
                webThickness: b.requiredNumber('--web-thickness'),
                flangeThickness: b.requiredNumber('--flange-thickness'),
            };
            return withSession(async (session) => {
                const report = await session.sectionChannel(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('angle', 'cli.cmd.section-angle')
    .option('--units <u>', 'cli.opt.units')
    .option('--height <h>', 'cli.opt.height')
    .option('--width <w>', 'cli.opt.width')
    .option('--thickness <t>', 'cli.opt.thickness')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                height: b.requiredNumber('--height'),
                width: b.requiredNumber('--width'),
                thickness: b.requiredNumber('--thickness'),
            };
            return withSession(async (session) => {
                const report = await session.sectionAngle(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('t', 'cli.cmd.section-t')
    .option('--units <u>', 'cli.opt.units')
    .option('--height <h>', 'cli.opt.height')
    .option('--width <b>', 'cli.opt.width')
    .option('--web-thickness <tw>', 'cli.opt.web-thickness')
    .option('--flange-thickness <tf>', 'cli.opt.flange-thickness')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                height: b.requiredNumber('--height'),
                width: b.requiredNumber('--width'),
                webThickness: b.requiredNumber('--web-thickness'),
                flangeThickness: b.requiredNumber('--flange-thickness'),
            };
            return withSession(async (session) => {
                const report = await session.sectionT(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('ellipse', 'cli.cmd.section-ellipse')
    .option('--units <u>', 'cli.opt.units')
    .option('--width <w>', 'cli.opt.width')
    .option('--height <h>', 'cli.opt.height')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                width: b.requiredNumber('--width'),
                height: b.requiredNumber('--height'),
            };
            return withSession(async (session) => {
                const report = await session.sectionEllipse(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('triangle', 'cli.cmd.section-triangle')
    .option('--units <u>', 'cli.opt.units')
    .option('--side <a>', 'cli.opt.side')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                side: b.requiredNumber('--side'),
            };
            return withSession(async (session) => {
                const report = await session.sectionTriangle(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('z', 'cli.cmd.section-z')
    .option('--units <u>', 'cli.opt.units')
    .option('--height <h>', 'cli.opt.height')
    .option('--width <w>', 'cli.opt.width')
    .option('--thickness <t>', 'cli.opt.thickness')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const geometry = {
                height: b.requiredNumber('--height'),
                width: b.requiredNumber('--width'),
                thickness: b.requiredNumber('--thickness'),
            };
            return withSession(async (session) => {
                const report = await session.sectionZ(geometry, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );
sectionCommand
    .command('principal', 'cli.cmd.section-principal')
    .option('--units <u>', 'cli.opt.units')
    .option('--ix <Ix>', 'cli.opt.ix')
    .option('--iy <Iy>', 'cli.opt.iy')
    .option('--ixy <Ixy>', 'cli.opt.ixy')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const units = b.units() ?? 'mm';
            const input = {
                ix: b.requiredNumber('--ix'),
                iy: b.requiredNumber('--iy'),
                ixy: b.requiredNumber('--ixy'),
            };
            return withSession(async (session) => {
                const report = await session.sectionPrincipal(input, units);
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );

const supportCommand = cli.command('support', 'cli.cmd.support').actionCtx(groupAction('support', 'add or remove'));
supportCommand
    .command('add', 'cli.cmd.support-add')
    .option('--node <node>', 'cli.opt.node')
    .option('--ux <v>', 'cli.opt.ux')
    .option('--uy <v>', 'cli.opt.uy')
    .option('--uz <v>', 'cli.opt.uz')
    .option('--rotation <v>', 'cli.opt.rotation')
    .option('--kx <k>', 'cli.opt.kx')
    .option('--ky <k>', 'cli.opt.ky')
    .option('--kz <k>', 'cli.opt.kz')
    .option('--kr <k>', 'cli.opt.kr')
    .option('--direction <nx,ny>', 'cli.opt.support-direction')
    .option('--dx', 'cli.opt.dx')
    .option('--dy', 'cli.opt.dy')
    .option('--dz', 'cli.opt.dz')
    .option('--rz', 'cli.opt.rz')
    .option('--rx', 'cli.opt.rx')
    .option('--ry', 'cli.opt.ry')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'support add <file> --node <node> [--dx] [--dy] [--dz] [--rz] [--rx] [--ry] [--direction <nx,ny>] [--ux <v>] [--uy <v>] [--uz <v>] [--rotation <v>] [--kx <k>] [--ky <k>] [--kz <k>] [--kr <k>]',
            );
            const node = b.required('--node');
            const dx = b.flag('--dx');
            const dy = b.flag('--dy');
            const dz = b.flag('--dz');
            const rz = b.flag('--rz');
            const rx = b.flag('--rx');
            const ry = b.flag('--ry');
            const ux = b.number('--ux');
            const uy = b.number('--uy');
            const uz = b.number('--uz');
            const rotation = b.number('--rotation');
            const kx = b.number('--kx');
            const ky = b.number('--ky');
            const kz = b.number('--kz');
            const kr = b.number('--kr');
            const direction = b.raw('--direction') === undefined ? undefined : b.direction();
            const spring = kx !== undefined || ky !== undefined || kz !== undefined || kr !== undefined;
            if (
                direction !== undefined &&
                (dx ||
                    dy ||
                    dz ||
                    rx ||
                    ry ||
                    rz ||
                    ux !== undefined ||
                    uy !== undefined ||
                    uz !== undefined ||
                    rotation !== undefined ||
                    spring)
            ) {
                throw failure(ctx, 'cli/direction-conflict', 'directionConflict', {});
            }
            if (
                direction === undefined &&
                !dx &&
                !dy &&
                !dz &&
                !rx &&
                !ry &&
                !rz &&
                ux === undefined &&
                uy === undefined &&
                uz === undefined &&
                rotation === undefined &&
                !spring
            ) {
                throw failure(ctx, 'cli/missing-option', 'noFreedom');
            }
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.addSupport({
                    node,
                    dx,
                    dy,
                    dz,
                    rx,
                    ry,
                    rz,
                    ux,
                    uy,
                    uz,
                    rotation,
                    kx,
                    ky,
                    kz,
                    kr,
                    direction,
                });
                ctx.stdout(
                    b.message('supportAdded', {
                        node,
                        dx: String(dx),
                        dy: String(dy),
                        dz: String(dz),
                        rx: String(rx),
                        ry: String(ry),
                        rz: String(rz),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );
supportCommand
    .command('remove', 'cli.cmd.support-remove')
    .option('--node <node>', 'cli.opt.node')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('support remove <file> --node <node>');
            const node = b.required('--node');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.removeSupport(node);
                ctx.stdout(b.message('supportRemoved', { node, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );

const loadCommand = cli.command('load', 'cli.cmd.load').actionCtx(groupAction('load', 'add or remove'));
loadCommand
    .command('add', 'cli.cmd.load-add')
    .option('--node <node>', 'cli.opt.node')
    .option('--fx <fx>', 'cli.opt.fx')
    .option('--fy <fy>', 'cli.opt.fy')
    .option('--fz <fz>', 'cli.opt.fz')
    .option('--mz <mz>', 'cli.opt.mz')
    .option('--mx <mx>', 'cli.opt.mx')
    .option('--my <my>', 'cli.opt.my')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('load add <file> --node <node> [--fx <fx>] [--fy <fy>] [--fz <fz>] [--mz <mz>] [--mx <mx>] [--my <my>]');
            const node = b.required('--node');
            const fx = b.number('--fx');
            const fy = b.number('--fy');
            const fz = b.number('--fz');
            const mz = b.number('--mz');
            const mx = b.number('--mx');
            const my = b.number('--my');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.addLoad({ node, fx, fy, fz, mz, mx, my });
                ctx.stdout(b.message('loadAdded', { node, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );
loadCommand
    .command('remove', 'cli.cmd.load-remove')
    .option('--node <node>', 'cli.opt.node')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('load remove <file> --node <node>');
            const node = b.required('--node');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.removeLoad(node);
                ctx.stdout(b.message('loadRemoved', { node, revision: String(summary.revision) }));
                return 0;
            });
        }),
    );

const modelCommand = cli.command('model', 'cli.cmd.model').actionCtx(groupAction('model', 'gravity, ambient or summary'));
modelCommand
    .command('gravity', 'cli.cmd.model-gravity')
    .option('--gx <gx>', 'cli.opt.gx')
    .option('--gy <gy>', 'cli.opt.gy')
    .option('--gz <gz>', 'cli.opt.gz')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('model gravity <file> [--gx <gx>] [--gy <gy>] [--gz <gz>]');
            const gx = b.number('--gx') ?? 0;
            const gy = b.number('--gy') ?? 0;
            const gz = b.number('--gz') ?? 0;
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setGravity({ gx, gy, gz });
                ctx.stdout(
                    b.message('gravitySet', {
                        gx: String(gx),
                        gy: String(gy),
                        gz: String(gz),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );
modelCommand
    .command('ambient', 'cli.cmd.model-ambient')
    .option('--temperature <Ta>', 'cli.opt.temperature')
    .option('--clear', 'cli.opt.clear')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('model ambient <file> [--temperature <Ta>] [--clear]');
            const temperature = b.flag('--clear') ? null : b.requiredNumber('--temperature');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setAmbientTemperature({ temperature });
                ctx.stdout(
                    b.message('ambientTemperatureSet', {
                        temperature: temperature === null ? 'cleared' : String(temperature),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );

modelCommand
    .command('summary', 'cli.cmd.model-summary')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('model summary <file> [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                ctx.stdout(JSON.stringify(document.modelSummary({ out: b.raw('--out') }), null, 2));
                return 0;
            });
        }),
    );

cli
    .command('self-weight', 'cli.cmd.self-weight')
    .option('--g <g>', 'cli.opt.g')
    .option('--axis <axis>', 'cli.opt.axis')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('self-weight <file> --g <g> [--axis x|y|z]');
            const value = b.requiredNumber('--g');
            const axis = b.raw('--axis') ?? 'y';
            const vector =
                axis === 'x'
                    ? { gx: value, gy: 0, gz: 0 }
                    : axis === 'y'
                      ? { gx: 0, gy: value, gz: 0 }
                      : axis === 'z'
                        ? { gx: 0, gy: 0, gz: value }
                        : undefined;
            if (vector === undefined) {
                throw failure(ctx, 'cli/invalid-option', 'unknownAxis', { value: axis });
            }
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.setGravity(vector);
                ctx.stdout(
                    b.message('gravitySet', {
                        gx: String(vector.gx),
                        gy: String(vector.gy),
                        gz: String(vector.gz),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );

const combinationCommand = cli.command('combination', 'cli.cmd.combination').actionCtx(groupAction('combination', 'add'));
combinationCommand
    .command('add', 'cli.cmd.combination-add')
    .option('--name <name>', 'cli.opt.name')
    .option('--loads <f>', 'cli.opt.loads')
    .option('--gravity <f>', 'cli.opt.gravity')
    .option('--thermal <f>', 'cli.opt.thermal')
    .option('--patterns <list>', 'cli.opt.patterns')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'combination add <file> --name <name> [--loads <f>] [--gravity <f>] [--thermal <f>] [--patterns <list>]',
            );
            const name = b.required('--name');
            const loads = b.number('--loads') ?? 1;
            const gravity = b.number('--gravity') ?? 0;
            const thermal = b.number('--thermal') ?? 1;
            const patterns = b.raw('--patterns') === undefined ? undefined : parsePatternList(ctx, b.required('--patterns'));
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const summary = document.addCombination({ name, loads, gravity, thermal, patterns });
                ctx.stdout(
                    b.message('combinationSet', {
                        name,
                        loads: String(loads),
                        gravity: String(gravity),
                        thermal: String(thermal),
                        revision: String(summary.revision),
                    }),
                );
                return 0;
            });
        }),
    );
combinationCommand
    .command('remove', 'cli.cmd.combination-remove')
    .option('--name <name>', 'cli.opt.name')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('combination remove <file> --name <name>');
            const name = b.required('--name');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                document.removeCombination(name);
                ctx.stdout(b.message('combinationRemoved', { name }));
                return 0;
            });
        }),
    );

cli
    .command('combinations', 'cli.cmd.combinations')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('combinations <file> [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.solveCombinations({ out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('envelope', 'cli.cmd.envelope')
    .option('--out <file>', 'cli.opt.out')
    .option('--csv <file>', 'cli.opt.csv')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('envelope <file> [--out report.json] [--csv envelope.csv]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.envelope({ out: b.raw('--out'), csv: b.raw('--csv') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('buckling-combinations', 'cli.cmd.buckling-combinations')
    .option('--modes <n>', 'cli.opt.modes')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('buckling-combinations <file> [--modes N] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.solveBucklingCombinations({
                    modes: modesOption(ctx, b),
                    out: b.raw('--out'),
                });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('validate', 'cli.cmd.validate')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('validate <file>');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const report = document.validate();
                ctx.stdout(JSON.stringify(report, null, 2));
                return report.ok ? 0 : 1;
            });
        }),
    );

cli
    .command('solve', 'cli.cmd.solve')
    .option('--out <file>', 'cli.opt.out')
    .option('--analysis <kind>', 'cli.opt.analysis')
    .option('--element <kind>', 'cli.opt.element-kind')
    .option('--dimension <n>', 'cli.opt.dimension')
    .option('--modes <n>', 'cli.opt.modes')
    .option('--iterations <n>', 'cli.opt.iterations')
    .option('--tolerance <t>', 'cli.opt.tolerance')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'solve <file> [--out report.json] [--analysis static|modal|buckling|tension-only] [--modes N] [--iterations N] [--tolerance t] [--element truss|beam|beam3d] [--dimension 2|3]',
            );
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const analysis = analysisOption(ctx, b, document.dimension);
                const out = b.raw('--out');
                if (analysis === 'modal') {
                    const result = document.solve({ analysis: 'modal', modes: modesOption(ctx, b), out });
                    ctx.stdout(JSON.stringify(result, null, 2));
                } else if (analysis === 'buckling') {
                    const result = document.solve({ analysis: 'buckling', modes: modesOption(ctx, b), out });
                    ctx.stdout(JSON.stringify(result, null, 2));
                } else if (analysis === 'tensionOnly') {
                    const result = document.tensionOnly({
                        iterations: iterationsOption(ctx, b),
                        tolerance: b.number('--tolerance'),
                        out,
                    });
                    ctx.stdout(JSON.stringify(result, null, 2));
                } else {
                    if (b.has('--modes')) {
                        throw failure(ctx, 'cli/invalid-option', 'modesRequireModal');
                    }
                    if (b.has('--iterations') || b.has('--tolerance')) {
                        throw failure(ctx, 'cli/invalid-option', 'tensionOnlyOptionsRequireAnalysis');
                    }
                    const result = document.solve({ out });
                    ctx.stdout(JSON.stringify(result, null, 2));
                }
                return 0;
            });
        }),
    );

cli
    .command('modal', 'cli.cmd.modal')
    .option('--modes <n>', 'cli.opt.modes')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('modal <file> [--modes N] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.solve({
                    analysis: 'modal',
                    modes: modesOption(ctx, b),
                    out: b.raw('--out'),
                });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('buckling', 'cli.cmd.buckling')
    .option('--modes <n>', 'cli.opt.modes')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('buckling <file> [--modes N] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.solve({
                    analysis: 'buckling',
                    modes: modesOption(ctx, b),
                    out: b.raw('--out'),
                });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('tension-only', 'cli.cmd.tension-only')
    .option('--iterations <n>', 'cli.opt.iterations')
    .option('--tolerance <t>', 'cli.opt.tolerance')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('tension-only <file> [--iterations N] [--tolerance t] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.tensionOnly({
                    iterations: iterationsOption(ctx, b),
                    tolerance: b.number('--tolerance'),
                    out: b.raw('--out'),
                });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('heat|thermal', 'cli.cmd.heat')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('heat <file> [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.heat({ out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('forces', 'cli.cmd.forces')
    .option('--stations <n>', 'cli.opt.stations')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('forces <file> [--stations N] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.internalForces({ stations: stationsOption(ctx, b), out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('influence', 'cli.cmd.influence')
    .option('--track <element>', 'cli.opt.track')
    .option('--quantity <kind>', 'cli.opt.quantity')
    .option('--node <node>', 'cli.opt.node')
    .option('--component <c>', 'cli.opt.component')
    .option('--element <id>', 'cli.opt.element')
    .option('--position <f>', 'cli.opt.position')
    .option('--direction <d>', 'cli.opt.direction')
    .option('--points <n>', 'cli.opt.points')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'influence <file> --track <element> --quantity reaction|section [--node <node>] [--component <c>] [--element <id>] [--position <f>] [--direction x|y] [--points N] [--out report.json]',
            );
            const track = b.required('--track');
            const quantityKind = b.required('--quantity');
            if (quantityKind !== 'reaction' && quantityKind !== 'section') {
                throw failure(ctx, 'cli/invalid-option', 'unknownInfluenceQuantity', { value: quantityKind });
            }
            const direction = influenceDirectionOption(ctx, b);
            const points = stationsOption(ctx, b);
            const component = influenceComponentOption(ctx, quantityKind, b);
            const quantity: InfluenceQuantity =
                quantityKind === 'reaction'
                    ? { kind: 'reaction', node: b.required('--node'), component: component as ReactionComponent }
                    : {
                          kind: 'section',
                          element: b.raw('--element'),
                          position: b.requiredNumber('--position'),
                          component: component as SectionComponent,
                      };
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.influenceLine({ track, quantity, direction, points, out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('influence-train', 'cli.cmd.influence-train')
    .option('--track <element>', 'cli.opt.track')
    .option('--quantity <kind>', 'cli.opt.quantity')
    .option('--node <node>', 'cli.opt.node')
    .option('--component <c>', 'cli.opt.component')
    .option('--element <id>', 'cli.opt.element')
    .option('--position <f>', 'cli.opt.position')
    .option('--direction <d>', 'cli.opt.direction')
    .option('--axles <p,...>', 'cli.opt.axles')
    .option('--spacings <d,...>', 'cli.opt.spacings')
    .option('--points <n>', 'cli.opt.points')
    .option('--step <f>', 'cli.opt.step')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'influence-train <file> --track <element> --quantity reaction|section [--node <node>] [--component <c>] [--element <id>] [--position <f>] [--direction x|y] --axles <P,...> --spacings <d,...> [--points N] [--step f] [--out report.json]',
            );
            const track = b.required('--track');
            const quantityKind = b.required('--quantity');
            if (quantityKind !== 'reaction' && quantityKind !== 'section') {
                throw failure(ctx, 'cli/invalid-option', 'unknownInfluenceQuantity', { value: quantityKind });
            }
            const direction = influenceDirectionOption(ctx, b);
            const points = pointsOption(ctx, b);
            const step = b.number('--step');
            const component = influenceComponentOption(ctx, quantityKind, b);
            const quantity: InfluenceQuantity =
                quantityKind === 'reaction'
                    ? { kind: 'reaction', node: b.required('--node'), component: component as ReactionComponent }
                    : {
                          kind: 'section',
                          element: b.raw('--element'),
                          position: b.requiredNumber('--position'),
                          component: component as SectionComponent,
                      };
            const axles = numberListOption(ctx, b, '--axles');
            const spacings = numberListOption(ctx, b, '--spacings');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.influenceLineTrain({
                    track,
                    quantity,
                    direction,
                    axles,
                    spacings,
                    points,
                    step,
                    out: b.raw('--out'),
                });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('reactions', 'cli.cmd.reactions')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('reactions <file> [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.reactions({ out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('reaction', 'cli.cmd.reaction')
    .option('--node <node>', 'cli.opt.node')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('reaction <file> --node <node>');
            const node = b.required('--node');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                ctx.stdout(JSON.stringify(document.nodeReaction(node), null, 2));
                return 0;
            });
        }),
    );

cli
    .command('stresses', 'cli.cmd.stresses')
    .option('--stations <n>', 'cli.opt.stations')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('stresses <file> [--stations N] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.stresses({ stations: stationsOption(ctx, b), out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('deflections', 'cli.cmd.deflections')
    .option('--stations <n>', 'cli.opt.stations')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('deflections <file> [--stations N] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.deflections({ stations: stationsOption(ctx, b), out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('nodal-stresses', 'cli.cmd.nodal-stresses')
    .option('--stations <n>', 'cli.opt.stations')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('nodal-stresses <file> [--stations N] [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.nodalStresses({ stations: stationsOption(ctx, b), out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('load-cases', 'cli.cmd.load-cases')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('load-cases <file> [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.loadCases({ out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('matrices', 'cli.cmd.matrices')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('matrices <file> [--out report.json]');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.matrices({ out: b.raw('--out') });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('spectrum', 'cli.cmd.spectrum')
    .option('--modes <n>', 'cli.opt.modes')
    .option('--axis <axis>', 'cli.opt.axis')
    .option('--combination <kind>', 'cli.opt.combination')
    .option('--damping <z>', 'cli.opt.damping')
    .option('--spectrum <t:sa,...>', 'cli.opt.spectrum')
    .option('--out <file>', 'cli.opt.out')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'spectrum <file> [--modes N] [--axis x|y|z] [--combination srss|cqc] [--damping <z>] --spectrum <T:Sa,...> [--out report.json]',
            );
            const spectrum = parseSpectrum(ctx, b.required('--spectrum'));
            const axis = spectrumAxisOption(ctx, b);
            const combination = spectrumCombinationOption(ctx, b);
            const damping = b.number('--damping');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.responseSpectrum({
                    modes: modesOption(ctx, b),
                    axis,
                    combination,
                    damping,
                    spectrum,
                    out: b.raw('--out'),
                });
                ctx.stdout(JSON.stringify(result, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('export', 'cli.cmd.export')
    .option('--format <format>', 'cli.opt.format')
    .option('--out <path>', 'cli.opt.out')
    .option('--scale <n>', 'cli.opt.scale')
    .option('--stations <n>', 'cli.opt.stations')
    .option('--contour <quantity>', 'cli.opt.contour')
    .option('--drawing <kind>', 'cli.opt.drawing')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file(
                'export <file> --format csv|vtk|md|svg --out <dir|file> [--scale N] [--stations N] [--contour axial|shear|moment|displacement] [--drawing forces|displacement]',
            );
            const format = exportFormat(ctx, b.required('--format'));
            const out = b.required('--out');
            const scale = b.number('--scale');
            const stations = b.raw('--stations') === undefined ? undefined : stationsOption(ctx, b);
            const contour = contourOption(ctx, b);
            const drawing = drawingOption(ctx, b);
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                const result = document.export(format, out, { scale, stations, contour, drawing });
                ctx.stdout(b.message('exported', { format: result.format, path: result.path }));
                return 0;
            });
        }),
    );

cli
    .command('report', 'cli.cmd.report')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const file = b.file('report <file>');
            return withSession(async (session) => {
                const document = await openDocument(session, file);
                ctx.stdout(JSON.stringify(document.report(), null, 2));
                return 0;
            });
        }),
    );

cli
    .command('import', 'cli.cmd.import')
    .option('--format <format>', 'cli.opt.importFormat')
    .option('--units <u>', 'cli.opt.units')
    .actionCtx(async (options, _args, ctx) =>
        attempt(ctx, async () => {
            const b = bind(ctx, options);
            const usage = 'import <input.inp> <output.ocae> [--format abaqus|inp] [--units mm|m]';
            const source = b.file(usage);
            const target = options._[1];
            if (target === undefined) throw failure(ctx, 'cli/missing-argument', 'missingArgument', { usage });
            importFormat(ctx, b.raw('--format'), source);
            const units = b.units();
            return withSession(async (session) => {
                const report = await session.importAbaqus(source, target, { units });
                ctx.stdout(JSON.stringify(report, null, 2));
                return 0;
            });
        }),
    );

cli
    .command('nonlinear', 'cli.cmd.nonlinear')
    .actionCtx(async (_options, _args, ctx) =>
        attempt(ctx, () => {
            throw new NotImplementedError('nonlinear analysis');
        }),
    );

cli
    .command('beam|beam2d', 'cli.cmd.beam')
    .actionCtx(async (_options, _args, ctx) =>
        attempt(ctx, () => {
            throw failure(ctx, 'cli/unknown-command', 'unknownCommand', {
                command: ctx.commandPath[1] ?? 'beam',
            });
        }),
    );

cli
    .command('frame|frame3d', 'cli.cmd.frame')
    .actionCtx(async (_options, _args, ctx) =>
        attempt(ctx, () => {
            throw failure(ctx, 'cli/unknown-command', 'unknownCommand', {
                command: ctx.commandPath[1] ?? 'frame',
            });
        }),
    );

cli
    .command('contact', 'cli.cmd.contact')
    .actionCtx(async (_options, _args, ctx) =>
        attempt(ctx, () => {
            throw new NotImplementedError('contact analysis');
        }),
    );

const entry = process.argv[1];
if (entry !== undefined && import.meta.url === pathToFileURL(entry).href) {
    process.exitCode = await cli.parse(joinNegativeValues(process.argv));
}
