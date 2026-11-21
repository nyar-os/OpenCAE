/**
 * Orthogonal native operations.
 *
 * One function performs one operation and returns data; argument parsing, help
 * text and exit codes belong to `cli.ts` and to the session surface. Nothing
 * here knows about a command line. Every function forwards to the optional
 * native addon and decodes one JSON string.
 */

import { NotImplementedError, OpenCAEError, errorCodes, type OpenCAEErrorCode } from './errors.js';
import { nativeBinding } from './native.js';
import type {
    BucklingCombinationOptions,
    BucklingCombinationSolution,
    BucklingSolution,
    CombinationAddRequest,
    CombinationSolveOptions,
    CombinationSolution,
    CreateOptions,
    DeflectionOptions,
    DeflectionSolution,
    DocumentSummary,
    ElementAddRequest,
    ElementReleaseRequest,
    ExportFormat,
    ExportReport,
    GravitySetRequest,
    InternalForceOptions,
    InternalForceSolution,
    LoadAddRequest,
    MaterialLibrary,
    MaterialPresetRequest,
    MaterialSetRequest,
    ModalSolution,
    NodeAddRequest,
    OpenCAECapabilities,
    Report,
    ReactionOptions,
    ReactionSummary,
    Solution,
    SolveOptions,
    StressOptions,
    StressSolution,
    SupportAddRequest,
    ValidationReport,
} from './types.js';

const knownCodes = new Set<string>(errorCodes);

function binding(feature: string) {
    const native = nativeBinding();
    if (!native) {
        throw new NotImplementedError(feature);
    }
    return native;
}

function decode<T>(work: () => string): T {
    try {
        return JSON.parse(work()) as T;
    } catch (error) {
        throw translate(error);
    }
}

function translate(error: unknown): OpenCAEError {
    if (error instanceof OpenCAEError) {
        return error;
    }
    const message = error instanceof Error ? error.message : String(error);
    const separator = message.indexOf(': ');
    const code = separator > 0 ? message.slice(0, separator) : '';
    if (knownCodes.has(code)) {
        return new OpenCAEError(code as OpenCAEErrorCode, message.slice(separator + 2));
    }
    return new OpenCAEError('native/error', message, { cause: error });
}

/** Reads the version of the installed addon. */
export function version(): string {
    return decode<string>(() => binding('version').version());
}

/** Reads the capability matrix of the installed addon. */
export function capabilities(): OpenCAECapabilities {
    const matrix = decode<{ solver: Record<string, 'supported' | 'unsupported'>; io: Record<string, 'supported' | 'unsupported'> }>(
        () => binding('capabilities').capabilities(),
    );
    return {
        engine: { package: 'open-cae-napi', version: version() },
        solver: matrix.solver,
        io: matrix.io,
    };
}

/** Creates an empty document and returns its summary. */
export function create(path: string, options: Required<CreateOptions>): DocumentSummary {
    return decode<DocumentSummary>(() => binding('create').create(path, JSON.stringify(options)));
}

/** Opens a document and returns its summary. */
export function open(path: string): DocumentSummary {
    return decode<DocumentSummary>(() => binding('open').open(path));
}

/** Writes a document to `target`, or in place when `target` is undefined. */
export function save(path: string, target?: string): DocumentSummary {
    return decode<DocumentSummary>(() =>
        target === undefined ? binding('save').save(path) : binding('save').save(path, target),
    );
}

/** Adds a node and returns the updated summary. */
export function nodeAdd(path: string, request: NodeAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('node.add').node_add(path, JSON.stringify(request)));
}

/** Adds a truss or beam element and returns the updated summary. */
export function elementAdd(path: string, request: ElementAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('element.add').element_add(path, JSON.stringify(request)));
}

/** Sets or clears the bending-moment releases of a beam element. */
export function elementRelease(path: string, request: ElementReleaseRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('element.release').element_release(path, JSON.stringify(request)));
}

/** Replaces the global material and returns the updated summary. */
export function materialSet(path: string, request: MaterialSetRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('material.set').material_set(path, JSON.stringify(request)));
}

/** Applies a built-in material preset and returns the updated summary. */
export function materialPreset(path: string, request: MaterialPresetRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('material.preset').material_preset(path, JSON.stringify(request)));
}

/** Reads the built-in material and section library in the requested units. */
export function library(units: 'mm' | 'm'): MaterialLibrary {
    return decode<MaterialLibrary>(() => binding('library').library(JSON.stringify({ units })));
}

/** Adds or merges a support entry and returns the updated summary. */
export function supportAdd(path: string, request: SupportAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('support.add').support_add(path, JSON.stringify(request)));
}

/** Adds or updates a concentrated load and returns the updated summary. */
export function loadAdd(path: string, request: LoadAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('load.add').load_add(path, JSON.stringify(request)));
}

/** Sets the gravity acceleration vector and returns the updated summary. */
export function setGravity(path: string, request: GravitySetRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('gravity.set').gravity_set(path, JSON.stringify(request)));
}

/** Validates a document. */
export function validate(path: string): ValidationReport {
    return decode<ValidationReport>(() => binding('validate').validate(path));
}

/** Solves a document for a static load case. */
export function solve(path: string, options?: SolveOptions & { analysis?: 'static' }): Solution;
/** Solves a document for `options.modes` natural frequencies. */
export function solve(path: string, options: SolveOptions & { analysis: 'modal' }): ModalSolution;
/** Solves a document for `options.modes` linear buckling load factors. */
export function solve(path: string, options: SolveOptions & { analysis: 'buckling' }): BucklingSolution;
/** Solves a document, optionally writing the JSON report to `options.out`. */
export function solve(path: string, options: SolveOptions = {}): Solution | ModalSolution | BucklingSolution {
    const request =
        options.out === undefined && options.analysis === undefined && options.modes === undefined
            ? undefined
            : JSON.stringify({ out: options.out, analysis: options.analysis, modes: options.modes });
    return decode<Solution | ModalSolution | BucklingSolution>(() =>
        request === undefined ? binding('solve').solve(path) : binding('solve').solve(path, request),
    );
}

/** Adds or replaces a linear load combination. */
export function addCombination(path: string, request: CombinationAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() =>
        binding('combination.add').combination_add(path, JSON.stringify(request)),
    );
}

/** Removes a load combination and returns the updated summary. */
export function removeCombination(path: string, name: string): DocumentSummary {
    return decode<DocumentSummary>(() =>
        binding('combination.remove').combination_remove(path, JSON.stringify({ name })),
    );
}

/** Solves every load combination and returns the solutions plus envelope. */
export function solveCombinations(path: string, options: CombinationSolveOptions = {}): CombinationSolution {
    const request = options.out === undefined ? undefined : JSON.stringify({ out: options.out });
    return decode<CombinationSolution>(() =>
        request === undefined
            ? binding('combination.solve').solve_combinations(path)
            : binding('combination.solve').solve_combinations(path, request),
    );
}

/** Solves linear buckling for every load combination and returns the factors. */
export function solveBucklingCombinations(
    path: string,
    options: BucklingCombinationOptions = {},
): BucklingCombinationSolution {
    const request =
        options.modes === undefined && options.out === undefined
            ? undefined
            : JSON.stringify({ modes: options.modes, out: options.out });
    return decode<BucklingCombinationSolution>(() =>
        request === undefined
            ? binding('combination.buckling').solve_buckling_combinations(path)
            : binding('combination.buckling').solve_buckling_combinations(path, request),
    );
}

/** Returns support reactions and the global force equilibrium check. */
export function reactions(path: string, options: ReactionOptions = {}): ReactionSummary {
    const request = options.out === undefined ? undefined : JSON.stringify({ out: options.out });
    return decode<ReactionSummary>(() =>
        request === undefined ? binding('solve.reactions').reactions(path) : binding('solve.reactions').reactions(path, request),
    );
}

/** Samples internal axial force, shear and bending moment per element. */
export function internalForces(path: string, options: InternalForceOptions = {}): InternalForceSolution {
    const request = options.stations === undefined && options.out === undefined
        ? undefined
        : JSON.stringify({ stations: options.stations, out: options.out });
    return decode<InternalForceSolution>(() =>
        request === undefined ? binding('internal.forces').internal_forces(path) : binding('internal.forces').internal_forces(path, request),
    );
}

/** Recovers extreme-fibre stresses and utilisation per element. */
export function stresses(path: string, options: StressOptions = {}): StressSolution {
    const request = options.stations === undefined && options.out === undefined
        ? undefined
        : JSON.stringify({ stations: options.stations, out: options.out });
    return decode<StressSolution>(() =>
        request === undefined ? binding('internal.stresses').stresses(path) : binding('internal.stresses').stresses(path, request),
    );
}

/** Samples the deflected centreline of every element. */
export function deflections(path: string, options: DeflectionOptions = {}): DeflectionSolution {
    const request = options.stations === undefined && options.out === undefined
        ? undefined
        : JSON.stringify({ stations: options.stations, out: options.out });
    return decode<DeflectionSolution>(() =>
        request === undefined
            ? binding('deflection.sample').deflections(path)
            : binding('deflection.sample').deflections(path, request),
    );
}

/** Exports a solved document. */
export function exportDocument(path: string, format: ExportFormat, target: string): ExportReport {
    return decode<ExportReport>(() => binding('export').export(path, format, target));
}

/** Solves a document and returns the full report. */
export function report(path: string): Report {
    return decode<Report>(() => binding('report').report(path));
}
