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
import type { Analysis, Dimension, ElementKind, ExportFormat, Units } from './types.js';

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
    if (value === 'csv' || value === 'vtk' || value === 'md') {
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
            return subcommand(args, 'add', 'node');
        case 'element':
            return subcommand(args, 'add', 'element');
        case 'material':
            return subcommand(args, 'set', 'material');
        case 'support':
            return subcommand(args, 'add', 'support');
        case 'load':
            return subcommand(args, 'add', 'load');
        case 'model':
            return modelCommand(args.slice(1));
        case 'self-weight':
            return selfWeight(args.slice(1));
        case 'validate':
            return validate(args.slice(1));
        case 'solve':
            return solveCommand(args.slice(1));
        case 'modal':
            return modalCommand(args.slice(1));
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

async function subcommand(args: readonly string[], expected: string, group: string): Promise<number> {
    const action: string | undefined = args[1];
    if (action === undefined) {
        throw failure('cli/missing-argument', 'missingSubcommand', { group, expected });
    }
    if (action !== expected) {
        throw failure('cli/unknown-command', 'unknownSubcommand', { group, action, expected });
    }
    switch (group) {
        case 'node':
            return nodeAdd(args.slice(2));
        case 'element':
            return elementAdd(args.slice(2));
        case 'material':
            return materialSet(args.slice(2));
        case 'support':
            return supportAdd(args.slice(2));
        case 'load':
            return loadAdd(args.slice(2));
        default:
            throw failure('cli/unknown-command', 'unknownGroup', { group, action });
    }
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
    const parsed = parse(args, ['--id', '--from', '--to', '--E', '--A', '--kind', '--I', '--w'], []);
    const file = positional(
        parsed,
        0,
        'element add <file> --id <id> --from <node> --to <node> [--kind truss|beam] [--I <I>] [--w <w>]',
    );
    const id = required(parsed, '--id');
    const from = required(parsed, '--from');
    const to = required(parsed, '--to');
    const youngsModulus = number(parsed, '--E');
    const area = number(parsed, '--A');
    const kind = elementKindOption(parsed);
    const inertia = number(parsed, '--I');
    const distributedLoad = number(parsed, '--w');
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
        });
        console.log(message('elementAdded', { id, revision: String(summary.revision) }));
        return 0;
    });
}

async function materialSet(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--E', '--A', '--density', '--name'], []);
    const file = positional(parsed, 0, 'material set <file> --E <E> --A <A> [--density <rho>] [--name <name>]');
    const youngsModulus = requiredNumber(parsed, '--E');
    const area = requiredNumber(parsed, '--A');
    const density = number(parsed, '--density');
    const name = parsed.options.get('--name');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.setMaterial({ youngsModulus, area, density, name });
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

async function supportAdd(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--node'], ['--dx', '--dy', '--dz', '--rz']);
    const file = positional(parsed, 0, 'support add <file> --node <node> [--dx] [--dy] [--dz] [--rz]');
    const node = required(parsed, '--node');
    const dx = parsed.flags.has('--dx');
    const dy = parsed.flags.has('--dy');
    const dz = parsed.flags.has('--dz');
    const rz = parsed.flags.has('--rz');
    if (!dx && !dy && !dz && !rz) {
        throw failure('cli/missing-option', 'noFreedom');
    }
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const summary = document.addSupport({ node, dx, dy, dz, rz });
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
            break;
        case 'nonlinear':
            throw new NotImplementedError('nonlinear analysis');
        case 'buckling':
            throw new NotImplementedError('buckling analysis');
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
    return analysis === 'modal' ? 'modal' : 'static';
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

async function solveCommand(args: readonly string[]): Promise<number> {
    const parsed = parse(args, ['--out', '--analysis', '--element', '--dimension', '--modes'], []);
    const file = positional(
        parsed,
        0,
        'solve <file> [--out report.json] [--analysis static|modal] [--modes N] [--element truss|beam] [--dimension 2|3]',
    );
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const analysis = analysisOption(parsed, document.dimension);
        const out = parsed.options.get('--out');
        if (analysis === 'modal') {
            const result = document.solve({ analysis: 'modal', modes: modesOption(parsed), out });
            console.log(JSON.stringify(result, null, 2));
        } else {
            if (parsed.options.has('--modes')) {
                throw failure('cli/invalid-option', 'modesRequireModal');
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
    const parsed = parse(args, ['--format', '--out'], []);
    const file = positional(parsed, 0, 'export <file> --format csv|vtk|md --out <dir|file>');
    const format = exportFormat(required(parsed, '--format'));
    const out = required(parsed, '--out');
    return withSession(async (session) => {
        const document = await openDocument(session, file);
        const result = document.export(format, out);
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
