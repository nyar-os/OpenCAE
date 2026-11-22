#!/usr/bin/env node
/**
 * Command-line assembly only.
 *
 * Every command maps options onto one session operation of `OpenCAE`; editing,
 * validation, solving and export stay in the native core. The CLI owns argument
 * parsing, localized help text, printing and exit codes, and nothing else.
 */

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import type { CaeDocument } from './document.js';
import { NotImplementedError, OpenCAEError, type OpenCAEErrorCode } from './errors.js';
import { OpenCAE } from './open-cae.js';
import type {
    Analysis,
    Dimension,
    ElementKind,
    ExportFormat,
    Section,
    SpectrumCombination,
    SpectrumPoint,
    Units,
} from './types.js';

interface CliLocale {
    readonly cli: {
        readonly intro: string;
        readonly usage: readonly string[];
        readonly exit: string;
        readonly message: Readonly<Record<string, string>>;
        readonly error: Readonly<Record<string, string>>;
    };
}

const here = path.dirname(fileURLToPath(import.meta.url));
const locale = JSON.parse(
    fs.readFileSync(path.join(here, '..', 'locales', 'en-US', 'cli.json'), 'utf8'),
) as CliLocale;

/** Fills `{name}` placeholders; unknown keys are left untouched. */
function fill(template: string, values: Readonly<Record<string, string>> = {}): string {
    return template.replace(/\{(\w+)\}/g, (_match, key: string) => values[key] ?? `{${key}}`);
}

function message(key: string, values?: Readonly<Record<string, string>>): string {
    return fill(locale.cli.message[key] ?? key, values);
}

function failure(code: OpenCAEErrorCode, key: string, values?: Readonly<Record<string, string>>): OpenCAEError {
    return new OpenCAEError(code, fill(locale.cli.error[key] ?? key, values));
}

function printUsage(): void {
    process.stdout.write(`${locale.cli.intro}\n\n${locale.cli.usage.join('\n')}\n\n${locale.cli.exit}\n`);
}

interface Parsed {
    readonly positionals: readonly string[];
    readonly flags: ReadonlySet<string>;
    readonly options: ReadonlyMap<string, string>;
}

function parse(args: readonly string[], valueOptions: readonly string[], flagOptions: readonly string[]): Parsed {
    const positionals: string[] = [];
    const flags = new Set<string>();
    const options = new Map<string, string>();
    let index = 0;
    while (index < args.length) {
        const argument = args[index] as string;
        if (argument.startsWith('--')) {
            if (flagOptions.includes(argument)) {
                flags.add(argument);
                index += 1;
            } else if (valueOptions.includes(argument)) {
                const value = args[index + 1];
                if (value === undefined || value.startsWith('--')) {
                    throw failure('cli/missing-value', 'missingValue', { option: argument });
                }
                options.set(argument, value);
                index += 2;
            } else {
                throw failure('cli/unknown-option', 'unknownOption', { option: argument });
            }
        } else {
            positionals.push(argument);
            index += 1;
        }
    }
    return { positionals, flags, options };
}

function positional(parsed: Parsed, index: number, usage: string): string {
    const value = parsed.positionals[index];
    if (value === undefined) {
        throw failure('cli/missing-argument', 'missingArgument', { usage });
    }
    return value;
}

function required(parsed: Parsed, name: string): string {
    const value = parsed.options.get(name);
    if (value === undefined) {
        throw failure('cli/missing-option', 'missingOption', { option: name });
    }
    return value;
}

function number(parsed: Parsed, name: string): number | undefined {
    const raw = parsed.options.get(name);
    if (raw === undefined) {
        return undefined;
    }
    const value = Number(raw);
    if (!Number.isFinite(value)) {
        throw failure('cli/invalid-number', 'invalidNumber', { option: name, value: raw });
    }
    return value;
}

function requiredNumber(parsed: Parsed, name: string): number {
    const value = number(parsed, name);
    if (value === undefined) {
        throw failure('cli/missing-option', 'missingOption', { option: name });
    }
    return value;
}

function unitsOption(parsed: Parsed): Units | undefined {
    const raw = parsed.options.get('--units');
    if (raw === undefined) {
        return undefined;
    }
    if (raw === 'mm' || raw === 'm') {
        return raw;
    }
    throw failure('cli/invalid-units', 'invalidUnits', { value: raw });
}

function dimensionOption(parsed: Parsed): Dimension | undefined {
    const raw = parsed.options.get('--dimension');
    if (raw === undefined) {
        return undefined;
    }
    if (raw === '2') {
        return 2;
    }
    if (raw === '3') {
        return 3;
    }
    throw failure('cli/invalid-option', 'invalidDimension', { value: raw });
}

function exportFormat(value: string): ExportFormat {
    if (value === 'csv' || value === 'vtk' || value === 'md' || value === 'svg') {
        return value;
    }
    if (value === 'inp' || value === 'abaqus' || value === 'step' || value === 'stp') {
        throw new NotImplementedError(`${value} export`);
    }
    throw failure('cli/unknown-format', 'unknownFormat', { format: value });
}

async function withSession<T>(work: (session: OpenCAE) => Promise<T>): Promise<T> {
    const session = new OpenCAE();
    try {
        return await work(session);
    } finally {
        await session.dispose();
    }
}

async function dispatch(args: readonly string[]): Promise<number> {
    const command: string | undefined = args[0];
    if (command === undefined) {
        printUsage();
        return 0;
    }
    switch (command) {
        case '-h':
        case '--help':
        case 'help':
            printUsage();
            return 0;
        case 'capabilities':
            return withSession(async (session) => {
                console.log(JSON.stringify(await session.capabilities(), null, 2));
                return 0;
            });
        case 'create':
            return create(args.slice(1));
        case 'node':
            return nodeCommand(args.slice(1));
        case 'element':
            return elementCommand(args.slice(1));
        case 'material':
            return materialCommand(args.slice(1));
        case 'library':
            return libraryCommand(args.slice(1));
        case 'section':
            return sectionCommand(args.slice(1));
        case 'support':
            return supportCommand(args.slice(1));
        case 'load':
            return loadCommand(args.slice(1));
        case 'model':
            return modelCommand(args.slice(1));
        case 'self-weight':
            return selfWeight(args.slice(1));
        case 'combination':
            return combinationCommand(args.slice(1));
        case 'combinations':
            return combinationsCommand(args.slice(1));
        case 'buckling-combinations':
            return bucklingCombinationsCommand(args.slice(1));
        case 'validate':
            return validate(args.slice(1));
        case 'solve':
            return solveCommand(args.slice(1));
        case 'modal':
            return modalCommand(args.slice(1));
        case 'buckling':
            return bucklingCommand(args.slice(1));
        case 'tension-only':
            return tensionOnlyCommand(args.slice(1));
        case 'forces':
            return forcesCommand(args.slice(1));
        case 'reactions':
            return reactionsCommand(args.slice(1));
        case 'reaction':
            return reactionCommand(args.slice(1));
        case 'stresses':
            return stressesCommand(args.slice(1));
        case 'deflections':
            return deflectionsCommand(args.slice(1));
        case 'nodal-stresses':
            return nodalStressesCommand(args.slice(1));
        case 'load-cases':
            return loadCasesCommand(args.slice(1));
        case 'matrices':
            return matricesCommand(args.slice(1));
        case 'spectrum':
            return spectrumCommand(args.slice(1));
        case 'export':
            return exportCommand(args.slice(1));
        case 'report':
            return report(args.slice(1));
        case 'import':
            throw new NotImplementedError('Abaqus .inp import');
        case 'nonlinear':
            throw new NotImplementedError('nonlinear analysis');
        case 'beam':
        case 'beam2d':
            throw failure('cli/unknown-command', 'unknownCommand', { command });
        case 'frame':
        case 'frame3d':
            throw new NotImplementedError('3D frame element');
        case 'thermal':
            throw new NotImplementedError('thermal analysis');
        case 'contact':
            throw new NotImplementedError('contact analysis');
        default:
            throw failure('cli/unknown-command', 'unknownCommand', { command });
    }
}

async function nodeCommand(args: readonly string[]): Promise<number> {
    const action = args[0];
    if (action === 'add') {
        return nodeAdd(args.slice(1));
    }
    if (action === 'update') {
        return nodeUpdate(args.slice(1));
    }
    if (action === 'mass') {
        return nodeMass(args.slice(1));
    }
    if (action === 'remove') {
        return nodeRemove(args.slice(1));
    }
    if (action === 'merge') {
        return nodeMerge(args.slice(1));
    }
    if (action === 'displacement') {
        return nodeDisplacement(args.slice(1));
    }
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', {
            group: 'node',
            expected: 'add, update, mass, remove, merge or displacement',
        });
    }
    throw failure('cli/unknown-command', 'unknownSubcommand', {
        group: 'node',
        action,
        expected: 'add, update, mass, remove, merge or displacement',
    });
}

async function supportCommand(args: readonly string[]): Promise<number> {
    const action = args[0];
    if (action === 'add') {
        return supportAdd(args.slice(1));
    }
    if (action === 'remove') {
        return supportRemove(args.slice(1));
    }
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', { group: 'support', expected: 'add or remove' });
    }
    throw failure('cli/unknown-command', 'unknownSubcommand', { group: 'support', action, expected: 'add or remove' });
}

async function loadCommand(args: readonly string[]): Promise<number> {
    const action = args[0];
    if (action === 'add') {
        return loadAdd(args.slice(1));
    }
    if (action === 'remove') {
        return loadRemove(args.slice(1));
    }
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', { group: 'load', expected: 'add or remove' });
    }
    throw failure('cli/unknown-command', 'unknownSubcommand', { group: 'load', action, expected: 'add or remove' });
}

async function create(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--units', '--dimension'], []);
    const file = positional(parsed, 0, 'create <file.ocae> [--units mm|m] [--dimension 2|3]');
    const units = unitsOption(parsed);
    const dimension = dimensionOption(parsed);
    return withSession(async (session) => {
        const document = await session.create(file, { units, dimension });
        console.log(
            message('created', {
                path: file,
                units: document.units ?? 'mm',
                dimension: String(document.dimension),
            }),
        );
        return 0;
    });
}

async function openDocument(session: OpenCAE, file: string): Promise<CaeDocument> {
    return session.open(file);
}

async function nodeAdd(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id', '--x', '--y', '--z'], []);
    const file = positional(parsed, 0, 'node add <file> --id <id> --x <x> --y <y> [--z <z>]');
    const id = required(parsed, '--id');
    const x = requiredNumber(parsed, '--x');
    const y = requiredNumber(parsed, '--y');
    const z = number(parsed, '--z') ?? 0;
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.addNode({ id, x, y, z });
        console.log(message('nodeAdded', { id, z: String(z), revision: String(summary.revision) }));
        return 0;
    });
}

async function nodeUpdate(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id', '--x', '--y', '--z'], []);
    const file = positional(parsed, 0, 'node update <file> --id <id> [--x <x>] [--y <y>] [--z <z>]');
    const id = required(parsed, '--id');
    const x = number(parsed, '--x');
    const y = number(parsed, '--y');
    const z = number(parsed, '--z');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.updateNode({ id, x, y, z });
        console.log(message('nodeUpdated', { id, revision: String(summary.revision) }));
        return 0;
    });
}

async function nodeMass(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id', '--mass'], []);
    const file = positional(parsed, 0, 'node mass <file> --id <id> --mass <mass>');
    const id = required(parsed, '--id');
    const mass = requiredNumber(parsed, '--mass');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.setNodeMass({ id, mass });
        console.log(message('nodeMassSet', { id, mass: String(mass), revision: String(summary.revision) }));
        return 0;
    });
}

async function nodeMerge(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--from', '--into'], []);
    const file = positional(parsed, 0, 'node merge <file> --from <id> --into <id>');
    const from = required(parsed, '--from');
    const into = required(parsed, '--into');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.mergeNodes({ from, into });
        console.log(message('nodeMerged', { from, into, revision: String(summary.revision) }));
        return 0;
    });
}

async function nodeRemove(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id'], []);
    const file = positional(parsed, 0, 'node remove <file> --id <id>');
    const id = required(parsed, '--id');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.removeNode(id);
        console.log(message('nodeRemoved', { id, revision: String(summary.revision) }));
        return 0;
    });
}

async function nodeDisplacement(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id'], []);
    const file = positional(parsed, 0, 'node displacement <file> --id <id>');
    const id = required(parsed, '--id');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        console.log(JSON.stringify(document.nodeDisplacement(id), null, 2));
        return 0;
    });
}

function elementKindOption(parsed: Parsed): ElementKind | undefined {
    const raw = parsed.options.get('--kind');
    if (raw === undefined) {
        return undefined;
    }
    if (raw === 'truss' || raw === 'beam') {
        return raw;
    }
    throw failure('cli/invalid-option', 'unknownElement', { value: raw });
}

async function elementAdd(args: readonly string[]): Promise<number> {
    const parsed = parse(
        args,
        ['--id', '--from', '--to', '--E', '--A', '--kind', '--I', '--w', '--w2', '--dt', '--dt-grad', '--w-from', '--w-to'],
        ['--tension-only'],
    );
    const file = positional(
        parsed,
        0,
        'element add <file> --id <id> --from <node> --to <node> [--kind truss|beam] [--I <I>] [--w <w>] [--w2 <w2>] [--dt <dT>] [--dt-grad <dT>] [--w-from <s>] [--w-to <s>] [--tension-only]',
    );
    const id = required(parsed, '--id');
    const from = required(parsed, '--from');
    const to = required(parsed, '--to');
    const youngsModulus = number(parsed, '--E');
    const area = number(parsed, '--A');
    const kind = elementKindOption(parsed);
    const inertia = number(parsed, '--I');
    const distributedLoad = number(parsed, '--w');
    const distributedLoadEnd = number(parsed, '--w2');
    const temperature = number(parsed, '--dt');
    const temperatureGradient = number(parsed, '--dt-grad');
    const distributedLoadFrom = number(parsed, '--w-from');
    const distributedLoadTo = number(parsed, '--w-to');
    if ((distributedLoadFrom === undefined) !== (distributedLoadTo === undefined)) {
        throw failure('cli/missing-option', 'missingOption', { option: '--w-from and --w-to' });
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
            distributedLoad,
            distributedLoadEnd,
            distributedLoadFrom,
            distributedLoadTo,
            temperature,
            temperatureGradient,
            tensionOnly: parsed.flags.has('--tension-only') ? true : undefined,
        });
        console.log(message('elementAdded', { id, revision: String(summary.revision) }));
        return 0;
    });
}

async function elementUpdate(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id', '--from', '--to'], []);
    const file = positional(parsed, 0, 'element update <file> --id <id> [--from <node>] [--to <node>]');
    const id = required(parsed, '--id');
    const from = parsed.options.get('--from');
    const to = parsed.options.get('--to');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.updateElement({ id, from, to });
        console.log(message('elementUpdated', { id, revision: String(summary.revision) }));
        return 0;
    });
}

async function elementCommand(args: readonly string[]): Promise<number> {
    const action = args[0];
    if (action === 'add') {
        return elementAdd(args.slice(1));
    }
    if (action === 'update') {
        return elementUpdate(args.slice(1));
    }
    if (action === 'remove') {
        return elementRemove(args.slice(1));
    }
    if (action === 'forces') {
        return elementForces(args.slice(1));
    }
    if (action === 'release') {
        return elementRelease(args.slice(1));
    }
    if (action === 'tension-only') {
        return elementTensionOnly(args.slice(1));
    }
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', {
            group: 'element',
            expected: 'add, update, remove, forces, release or tension-only',
        });
    }
    throw failure('cli/unknown-command', 'unknownSubcommand', {
        group: 'element',
        action,
        expected: 'add, update, remove, forces, release or tension-only',
    });
}

async function elementRelease(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id'], ['--start', '--end']);
    const file = positional(parsed, 0, 'element release <file> --id <id> [--start] [--end]');
    const id = required(parsed, '--id');
    const start = parsed.flags.has('--start');
    const end = parsed.flags.has('--end');
    if (!start && !end) {
        throw failure('cli/missing-option', 'missingOption', { option: '--start or --end' });
    }
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.setElementRelease({ id, start, end });
        console.log(message('elementReleased', { id, start: String(start), end: String(end), revision: String(summary.revision) }));
        return 0;
    });
}

async function elementTensionOnly(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id'], ['--clear']);
    const file = positional(parsed, 0, 'element tension-only <file> --id <id> [--clear]');
    const id = required(parsed, '--id');
    const tensionOnly = !parsed.flags.has('--clear');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.setElementTensionOnly({ id, tensionOnly });
        console.log(
            message('elementTensionOnly', {
                id,
                tensionOnly: String(tensionOnly),
                revision: String(summary.revision),
            }),
        );
        return 0;
    });
}

async function elementRemove(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id'], []);
    const file = positional(parsed, 0, 'element remove <file> --id <id>');
    const id = required(parsed, '--id');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.removeElement(id);
        console.log(message('elementRemoved', { id, revision: String(summary.revision) }));
        return 0;
    });
}

async function elementForces(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--id'], []);
    const file = positional(parsed, 0, 'element forces <file> --id <id>');
    const id = required(parsed, '--id');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        console.log(JSON.stringify(document.elementResult(id), null, 2));
        return 0;
    });
}

async function materialCommand(args: readonly string[]): Promise<number> {
    if (args[0] === 'set') {
        return materialSet(args.slice(1));
    }
    if (args[0] === 'preset') {
        return materialPreset(args.slice(1));
    }
    if (args[0] === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', { group: 'material', expected: 'set or preset' });
    }
    throw failure('cli/unknown-command', 'unknownSubcommand', {
        group: 'material',
        action: args[0],
        expected: 'set or preset',
    });
}

async function materialSet(args: readonly string[]): Promise<number> {
    const parsed = parse(
        args,
        [
            '--E',
            '--A',
            '--density',
            '--name',
            '--allowable',
            '--alpha',
            '--section',
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
        ],
        [],
    );
    const file = positional(parsed, 0, 'material set <file> --E <E> --A <A> [--density <rho>] [--name <name>]');
    const youngsModulus = requiredNumber(parsed, '--E');
    const area = requiredNumber(parsed, '--A');
    const density = number(parsed, '--density');
    const name = parsed.options.get('--name');
    const allowableStress = number(parsed, '--allowable');
    const alpha = number(parsed, '--alpha');
    const section = sectionOption(parsed);
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.setMaterial({ youngsModulus, area, density, name, allowableStress, alpha, section });
        console.log(
            message('materialSet', {
                youngsModulus: String(summary.material.youngsModulus),
                area: String(summary.material.area),
                density: String(summary.material.density),
                revision: String(summary.revision),
            }),
        );
        return 0;
    });
}

async function materialPreset(args: readonly string[]): Promise<number> {
    const parsed = parse(
        args,
        [
            '--preset',
            '--area',
            '--allowable',
            '--section',
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
        ],
        [],
    );
    const file = positional(
        parsed,
        0,
        'material preset <file> --preset <name> [--section rectangle|circle|i|box|tube|channel] [--area A] [--allowable S]',
    );
    const preset = required(parsed, '--preset');
    const area = number(parsed, '--area');
    const allowableStress = number(parsed, '--allowable');
    const section = sectionOption(parsed);
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.applyMaterialPreset({ preset, area, allowableStress, section });
        console.log(
            message('materialPreset', {
                preset,
                youngsModulus: String(summary.material.youngsModulus),
                area: String(summary.material.area),
                revision: String(summary.revision),
            }),
        );
        return 0;
    });
}

function sectionOption(parsed: Parsed): Section | undefined {
    const shape = parsed.options.get('--section');
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
        if (geometry.some((option) => parsed.options.has(option))) {
            throw failure('cli/missing-option', 'sectionShape');
        }
        return undefined;
    }
    if (shape === 'rectangle') {
        return {
            shape: 'rectangle',
            width: requiredNumber(parsed, '--width'),
            height: requiredNumber(parsed, '--height'),
        };
    }
    if (shape === 'circle') {
        return { shape: 'circle', diameter: requiredNumber(parsed, '--diameter') };
    }
    if (shape === 'i' || shape === 'i-section') {
        return {
            shape: 'i',
            depth: requiredNumber(parsed, '--depth'),
            flangeWidth: requiredNumber(parsed, '--flange-width'),
            flangeThickness: requiredNumber(parsed, '--flange-thickness'),
            webThickness: requiredNumber(parsed, '--web-thickness'),
        };
    }
    if (shape === 'box') {
        return {
            shape: 'box',
            width: requiredNumber(parsed, '--width'),
            height: requiredNumber(parsed, '--height'),
            wall: requiredNumber(parsed, '--wall'),
        };
    }
    if (shape === 'tube') {
        return {
            shape: 'tube',
            outerDiameter: requiredNumber(parsed, '--outer-diameter'),
            innerDiameter: requiredNumber(parsed, '--inner-diameter'),
        };
    }
    if (shape === 'channel') {
        return {
            shape: 'channel',
            depth: requiredNumber(parsed, '--depth'),
            flangeWidth: requiredNumber(parsed, '--flange-width'),
            flangeThickness: requiredNumber(parsed, '--flange-thickness'),
            webThickness: requiredNumber(parsed, '--web-thickness'),
        };
    }
    throw failure('cli/invalid-option', 'unknownSection', { value: shape });
}

async function libraryCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--units'], []);
    const units = unitsOption(parsed);
    return withSession(async (session) => {
        const library = await session.library(units);
        console.log(JSON.stringify(library, null, 2));
        return 0;
    });
}

async function sectionCommand(args: readonly string[]): Promise<number> {
    const action = args[0];
    if (action === 'properties') {
        return sectionPropertiesCommand(args.slice(1));
    }
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', { group: 'section', expected: 'properties' });
    }
    throw failure('cli/unknown-command', 'unknownSubcommand', { group: 'section', action, expected: 'properties' });
}

async function sectionPropertiesCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(
        args,
        [
            '--units',
            '--section',
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
        ],
        [],
    );
    const units = unitsOption(parsed) ?? 'mm';
    const section = sectionOption(parsed);
    if (section === undefined) {
        throw failure('cli/missing-option', 'missingSection', {});
    }
    return withSession(async (session) => {
        const report = await session.sectionProperties(section, units);
        console.log(JSON.stringify(report, null, 2));
        return 0;
    });
}

function directionOption(parsed: Parsed): readonly [number, number] {
    const raw = parsed.options.get('--direction') ?? '';
    const parts = raw.split(',').map((entry) => Number(entry.trim()));
    if (parts.length !== 2 || !parts.every((value) => Number.isFinite(value))) {
        throw failure('cli/invalid-option', 'invalidDirection', { value: raw });
    }
    return [parts[0], parts[1]];
}

async function supportAdd(args: readonly string[]): Promise<number> {
    const parsed = parse(
        args,
        ['--node', '--ux', '--uy', '--uz', '--rotation', '--kx', '--ky', '--kz', '--kr', '--direction'],
        ['--dx', '--dy', '--dz', '--rz'],
    );
    const file = positional(
        parsed,
        0,
        'support add <file> --node <node> [--dx] [--dy] [--dz] [--rz] [--direction <nx,ny>] [--ux <v>] [--uy <v>] [--uz <v>] [--rotation <v>] [--kx <k>] [--ky <k>] [--kz <k>] [--kr <k>]',
    );
    const node = required(parsed, '--node');
    const dx = parsed.flags.has('--dx');
    const dy = parsed.flags.has('--dy');
    const dz = parsed.flags.has('--dz');
    const rz = parsed.flags.has('--rz');
    const ux = number(parsed, '--ux');
    const uy = number(parsed, '--uy');
    const uz = number(parsed, '--uz');
    const rotation = number(parsed, '--rotation');
    const kx = number(parsed, '--kx');
    const ky = number(parsed, '--ky');
    const kz = number(parsed, '--kz');
    const kr = number(parsed, '--kr');
    const direction = parsed.options.get('--direction') === undefined ? undefined : directionOption(parsed);
    const spring = kx !== undefined || ky !== undefined || kz !== undefined || kr !== undefined;
    if (
        direction !== undefined &&
        (dx || dy || dz || rz || ux !== undefined || uy !== undefined || uz !== undefined || rotation !== undefined || spring)
    ) {
        throw failure('cli/direction-conflict', 'directionConflict', {});
    }
    if (
        direction === undefined &&
        !dx &&
        !dy &&
        !dz &&
        !rz &&
        ux === undefined &&
        uy === undefined &&
        uz === undefined &&
        rotation === undefined &&
        !spring
    ) {
        throw failure('cli/missing-option', 'noFreedom');
    }
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.addSupport({ node, dx, dy, dz, rz, ux, uy, uz, rotation, kx, ky, kz, kr, direction });
        console.log(
            message('supportAdded', {
                node,
                dx: String(dx),
                dy: String(dy),
                dz: String(dz),
                rz: String(rz),
                revision: String(summary.revision),
            }),
        );
        return 0;
    });
}

async function supportRemove(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--node'], []);
    const file = positional(parsed, 0, 'support remove <file> --node <node>');
    const node = required(parsed, '--node');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.removeSupport(node);
        console.log(message('supportRemoved', { node, revision: String(summary.revision) }));
        return 0;
    });
}

async function loadAdd(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--node', '--fx', '--fy', '--fz', '--mz'], []);
    const file = positional(parsed, 0, 'load add <file> --node <node> [--fx <fx>] [--fy <fy>] [--fz <fz>] [--mz <mz>]');
    const node = required(parsed, '--node');
    const fx = number(parsed, '--fx');
    const fy = number(parsed, '--fy');
    const fz = number(parsed, '--fz');
    const mz = number(parsed, '--mz');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.addLoad({ node, fx, fy, fz, mz });
        console.log(message('loadAdded', { node, revision: String(summary.revision) }));
        return 0;
    });
}

async function loadRemove(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--node'], []);
    const file = positional(parsed, 0, 'load remove <file> --node <node>');
    const node = required(parsed, '--node');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.removeLoad(node);
        console.log(message('loadRemoved', { node, revision: String(summary.revision) }));
        return 0;
    });
}

async function modelCommand(args: readonly string[]): Promise<number> {
    const action = args[0];
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', { group: 'model', expected: 'gravity' });
    }
    if (action !== 'gravity') {
        throw failure('cli/unknown-command', 'unknownSubcommand', { group: 'model', action, expected: 'gravity' });
    }
    return gravity(args.slice(1));
}

async function gravity(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--gx', '--gy', '--gz'], []);
    const file = positional(parsed, 0, 'model gravity <file> [--gx <gx>] [--gy <gy>] [--gz <gz>]');
    const gx = number(parsed, '--gx') ?? 0;
    const gy = number(parsed, '--gy') ?? 0;
    const gz = number(parsed, '--gz') ?? 0;
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.setGravity({ gx, gy, gz });
        console.log(
            message('gravitySet', {
                gx: String(gx),
                gy: String(gy),
                gz: String(gz),
                revision: String(summary.revision),
            }),
        );
        return 0;
    });
}

async function selfWeight(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--g', '--axis'], []);
    const file = positional(parsed, 0, 'self-weight <file> --g <g> [--axis x|y|z]');
    const value = requiredNumber(parsed, '--g');
    const axis = parsed.options.get('--axis') ?? 'y';
    const vector =
        axis === 'x'
            ? { gx: value, gy: 0, gz: 0 }
            : axis === 'y'
              ? { gx: 0, gy: value, gz: 0 }
              : axis === 'z'
                ? { gx: 0, gy: 0, gz: value }
                : undefined;
    if (vector === undefined) {
        throw failure('cli/invalid-option', 'unknownAxis', { value: axis });
    }
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.setGravity(vector);
        console.log(
            message('gravitySet', {
                gx: String(vector.gx),
                gy: String(vector.gy),
                gz: String(vector.gz),
                revision: String(summary.revision),
            }),
        );
        return 0;
    });
}

async function combinationCommand(args: readonly string[]): Promise<number> {
    const action = args[0];
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', { group: 'combination', expected: 'add' });
    }
    if (action === 'add') {
        return combinationAdd(args.slice(1));
    }
    if (action === 'remove') {
        return combinationRemove(args.slice(1));
    }
    throw failure('cli/unknown-command', 'unknownSubcommand', {
        group: 'combination',
        action,
        expected: 'add',
    });
}

async function combinationAdd(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--name', '--loads', '--gravity'], []);
    const file = positional(parsed, 0, 'combination add <file> --name <name> [--loads <f>] [--gravity <f>]');
    const name = required(parsed, '--name');
    const loads = number(parsed, '--loads') ?? 1;
    const gravity = number(parsed, '--gravity') ?? 0;
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.addCombination({ name, loads, gravity });
        console.log(
            message('combinationSet', {
                name,
                loads: String(loads),
                gravity: String(gravity),
                revision: String(summary.revision),
            }),
        );
        return 0;
    });
}

async function combinationRemove(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--name'], []);
    const file = positional(parsed, 0, 'combination remove <file> --name <name>');
    const name = required(parsed, '--name');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        document.removeCombination(name);
        console.log(message('combinationRemoved', { name }));
        return 0;
    });
}

async function combinationsCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--out'], []);
    const file = positional(parsed, 0, 'combinations <file> [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.solveCombinations({ out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function bucklingCombinationsCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--modes', '--out'], []);
    const file = positional(parsed, 0, 'buckling-combinations <file> [--modes N] [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.solveBucklingCombinations({
            modes: modesOption(parsed),
            out: parsed.options.get('--out'),
        });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function validate(args: readonly string[]): Promise<number> {
    const parsed = parse(args, [], []);
    const file = positional(parsed, 0, 'validate <file>');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const report = document.validate();
        console.log(JSON.stringify(report, null, 2));
        return report.ok ? 0 : 1;
    });
}

function analysisOption(parsed: Parsed, dimension: Dimension): Analysis {
    const analysis = parsed.options.get('--analysis');
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
            throw failure('cli/invalid-option', 'unknownAnalysis', { value: analysis });
    }
    const element = parsed.options.get('--element');
    if (element !== undefined) {
        switch (element) {
            case 'truss':
            case 'beam':
                break;
            case 'frame':
                throw new NotImplementedError('3D frame element');
            default:
                throw failure('cli/invalid-option', 'unknownElement', { value: element });
        }
    }
    const requested = dimensionOption(parsed);
    if (requested !== undefined && requested !== dimension) {
        throw failure('cli/dimension-mismatch', 'dimensionMismatch', {
            requested: String(requested),
            dimension: String(dimension),
        });
    }
    return analysis === 'modal' || analysis === 'buckling' ? analysis : 'static';
}

function modesOption(parsed: Parsed): number {
    const raw = parsed.options.get('--modes');
    if (raw === undefined) {
        return 5;
    }
    const modes = Number(raw);
    if (!Number.isInteger(modes) || modes < 1) {
        throw failure('cli/invalid-number', 'invalidModes', { value: raw });
    }
    return modes;
}

function iterationsOption(parsed: Parsed): number | undefined {
    const raw = parsed.options.get('--iterations');
    if (raw === undefined) {
        return undefined;
    }
    const iterations = Number(raw);
    if (!Number.isInteger(iterations) || iterations < 1) {
        throw failure('cli/invalid-number', 'invalidIterations', { value: raw });
    }
    return iterations;
}

async function solveCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(
        args,
        ['--out', '--analysis', '--element', '--dimension', '--modes', '--iterations', '--tolerance'],
        [],
    );
    const file = positional(
        parsed,
        0,
        'solve <file> [--out report.json] [--analysis static|modal|buckling|tension-only] [--modes N] [--iterations N] [--tolerance t] [--element truss|beam] [--dimension 2|3]',
    );
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const analysis = analysisOption(parsed, document.dimension);
        const out = parsed.options.get('--out');
        if (analysis === 'modal') {
            const result = document.solve({ analysis: 'modal', modes: modesOption(parsed), out });
            console.log(JSON.stringify(result, null, 2));
        } else if (analysis === 'buckling') {
            const result = document.solve({ analysis: 'buckling', modes: modesOption(parsed), out });
            console.log(JSON.stringify(result, null, 2));
        } else if (analysis === 'tensionOnly') {
            const result = document.tensionOnly({
                iterations: iterationsOption(parsed),
                tolerance: number(parsed, '--tolerance'),
                out,
            });
            console.log(JSON.stringify(result, null, 2));
        } else {
            if (parsed.options.has('--modes')) {
                throw failure('cli/invalid-option', 'modesRequireModal');
            }
            if (parsed.options.has('--iterations') || parsed.options.has('--tolerance')) {
                throw failure('cli/invalid-option', 'tensionOnlyOptionsRequireAnalysis');
            }
            const result = document.solve({ out });
            console.log(JSON.stringify(result, null, 2));
        }
        return 0;
    });
}

async function modalCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--modes', '--out'], []);
    const file = positional(parsed, 0, 'modal <file> [--modes N] [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.solve({ analysis: 'modal', modes: modesOption(parsed), out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function bucklingCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--modes', '--out'], []);
    const file = positional(parsed, 0, 'buckling <file> [--modes N] [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.solve({ analysis: 'buckling', modes: modesOption(parsed), out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function tensionOnlyCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--iterations', '--tolerance', '--out'], []);
    const file = positional(
        parsed,
        0,
        'tension-only <file> [--iterations N] [--tolerance t] [--out report.json]',
    );
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.tensionOnly({
            iterations: iterationsOption(parsed),
            tolerance: number(parsed, '--tolerance'),
            out: parsed.options.get('--out'),
        });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

function stationsOption(parsed: Parsed): number {
    const raw = parsed.options.get('--stations');
    if (raw === undefined) {
        return 11;
    }
    const stations = Number(raw);
    if (!Number.isInteger(stations) || stations < 2) {
        throw failure('cli/invalid-number', 'invalidStations', { value: raw });
    }
    return stations;
}

async function forcesCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--stations', '--out'], []);
    const file = positional(parsed, 0, 'forces <file> [--stations N] [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.internalForces({ stations: stationsOption(parsed), out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function reactionsCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--out'], []);
    const file = positional(parsed, 0, 'reactions <file> [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.reactions({ out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function reactionCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--node'], []);
    const file = positional(parsed, 0, 'reaction <file> --node <node>');
    const node = required(parsed, '--node');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        console.log(JSON.stringify(document.nodeReaction(node), null, 2));
        return 0;
    });
}

async function stressesCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--stations', '--out'], []);
    const file = positional(parsed, 0, 'stresses <file> [--stations N] [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.stresses({ stations: stationsOption(parsed), out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function deflectionsCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--stations', '--out'], []);
    const file = positional(parsed, 0, 'deflections <file> [--stations N] [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.deflections({ stations: stationsOption(parsed), out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function nodalStressesCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--stations', '--out'], []);
    const file = positional(parsed, 0, 'nodal-stresses <file> [--stations N] [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.nodalStresses({ stations: stationsOption(parsed), out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function loadCasesCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--out'], []);
    const file = positional(parsed, 0, 'load-cases <file> [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.loadCases({ out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function matricesCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--out'], []);
    const file = positional(parsed, 0, 'matrices <file> [--out report.json]');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.matrices({ out: parsed.options.get('--out') });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

function parseSpectrum(value: string): readonly SpectrumPoint[] {
    const points: SpectrumPoint[] = [];
    for (const item of value.split(',').map((entry) => entry.trim()).filter((entry) => entry.length > 0)) {
        const separator = item.indexOf(':');
        if (separator < 0) {
            throw failure('cli/invalid-spectrum', 'invalidSpectrumPoint', { point: item });
        }
        const period = Number(item.slice(0, separator).trim());
        const acceleration = Number(item.slice(separator + 1).trim());
        if (!Number.isFinite(period) || !Number.isFinite(acceleration)) {
            throw failure('cli/invalid-spectrum', 'invalidSpectrumPoint', { point: item });
        }
        points.push({ period, acceleration });
    }
    if (points.length === 0) {
        throw failure('cli/invalid-spectrum', 'emptySpectrum', {});
    }
    return points;
}

function spectrumAxisOption(parsed: Parsed): 'x' | 'y' | 'z' {
    const raw = parsed.options.get('--axis') ?? 'x';
    if (raw === 'x' || raw === 'y' || raw === 'z') {
        return raw;
    }
    throw failure('cli/invalid-option', 'unknownSpectrumAxis', { value: raw });
}

function spectrumCombinationOption(parsed: Parsed): SpectrumCombination {
    const raw = parsed.options.get('--combination') ?? 'srss';
    if (raw === 'srss' || raw === 'cqc') {
        return raw;
    }
    throw failure('spectrum/invalid-combination', 'unknownSpectrumCombination', { value: raw });
}

async function spectrumCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--modes', '--axis', '--combination', '--damping', '--spectrum', '--out'], []);
    const file = positional(
        parsed,
        0,
        'spectrum <file> [--modes N] [--axis x|y|z] [--combination srss|cqc] [--damping <z>] --spectrum <T:Sa,...> [--out report.json]',
    );
    const spectrum = parseSpectrum(required(parsed, '--spectrum'));
    const axis = spectrumAxisOption(parsed);
    const combination = spectrumCombinationOption(parsed);
    const damping = number(parsed, '--damping');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.responseSpectrum({
            modes: modesOption(parsed),
            axis,
            combination,
            damping,
            spectrum,
            out: parsed.options.get('--out'),
        });
        console.log(JSON.stringify(result, null, 2));
        return 0;
    });
}

async function report(args: readonly string[]): Promise<number> {
    const parsed = parse(args, [], []);
    const file = positional(parsed, 0, 'report <file>');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        console.log(JSON.stringify(document.report(), null, 2));
        return 0;
    });
}

async function exportCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--format', '--out', '--scale'], []);
    const file = positional(parsed, 0, 'export <file> --format csv|vtk|md|svg --out <dir|file> [--scale N]');
    const format = exportFormat(required(parsed, '--format'));
    const out = required(parsed, '--out');
    const scale = number(parsed, '--scale');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.export(format, out, scale === undefined ? {} : { scale });
        console.log(message('exported', { format: result.format, path: result.path }));
        return 0;
    });
}

function exitCodeOf(error: OpenCAEError): number {
    return error.code.startsWith('capability/') ? 2 : 1;
}

async function main(): Promise<void> {
    const args = process.argv.slice(2);
    try {
        process.exitCode = await dispatch(args);
    } catch (error) {
        const failure =
            error instanceof OpenCAEError
                ? error
                : new OpenCAEError('native/error', error instanceof Error ? error.message : String(error), {
                      cause: error,
                  });
        process.stderr.write(`${JSON.stringify({ code: failure.code, message: failure.message }, null, 2)}\n`);
        process.exitCode = exitCodeOf(failure);
    }
}

await main();
