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
    ConstructionStage,
    ConstructionStageOptions,
    ConstructionStageSolution,
    CreateOptions,
    DeflectionOptions,
    DeflectionSolution,
    DocumentSummary,
    ElementAddRequest,
    ElementReleaseRequest,
    ElementResult,
    ElementSplitReport,
    ElementSplitRequest,
    ElementTensionOnlyRequest,
    ElementUpdateRequest,
    ExportFormat,
    ExportOptions,
    ExportReport,
    FrequencyResponseOptions,
    FrequencyResponseSolution,
    GravitySetRequest,
    HeatOptions,
    HeatSolution,
    InternalForceOptions,
    InternalForceSolution,
    LoadAddRequest,
    LoadCaseOptions,
    LoadCaseSummary,
    MaterialLibrary,
    MaterialPresetRequest,
    MaterialSetRequest,
    MatrixOptions,
    ModalSolution,
    NodeAddRequest,
    NodeDisplacement,
    NodeHeatRequest,
    NodeMassRequest,
    NodeMergeRequest,
    NodeTemperatureRequest,
    NodeUpdateRequest,
    NodalStressOptions,
    NodalStressSolution,
    NonlinearOptions,
    NonlinearSolution,
    OpenCAECapabilities,
    Report,
    ReactionOptions,
    ReactionSummary,
    Section,
    SectionReport,
    ResponseSpectrumOptions,
    ResponseSpectrumSolution,
    Solution,
    SolveOptions,
    StressOptions,
    StressSolution,
    SupportAddRequest,
    SupportReaction,
    SystemMatrices,
    TensionOnlyOptions,
    TensionOnlySolution,
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
    const matrix = decode<{
        solver: Record<string, 'supported' | 'unsupported'>;
        edits: Record<string, 'supported' | 'unsupported'>;
        io: Record<string, 'supported' | 'unsupported'>;
    }>(() => binding('capabilities').capabilities());
    return {
        engine: { package: 'open-cae-napi', version: version() },
        solver: matrix.solver,
        edits: matrix.edits,
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

/** Moves an existing node and returns the updated summary. */
export function updateNode(path: string, request: NodeUpdateRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('node.update').node_update(path, JSON.stringify(request)));
}

/** Sets or clears the concentrated mass of a node and returns the updated summary. */
export function setNodeMass(path: string, request: NodeMassRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('node.mass').node_mass(path, JSON.stringify(request)));
}

/** Sets or clears the fixed temperature of a node. */
export function setNodeTemperature(path: string, request: NodeTemperatureRequest): DocumentSummary {
    return decode<DocumentSummary>(() =>
        binding('node.temperature').node_temperature(path, JSON.stringify(request)),
    );
}

/** Sets the concentrated nodal heat flow. */
export function setNodeHeat(path: string, request: NodeHeatRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('node.heat').node_heat(path, JSON.stringify(request)));
}

/** Merges one node into another and returns the updated summary. */
export function mergeNodes(path: string, request: NodeMergeRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('node.merge').node_merge(path, JSON.stringify(request)));
}

/** Adds a truss or beam element and returns the updated summary. */
export function elementAdd(path: string, request: ElementAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('element.add').element_add(path, JSON.stringify(request)));
}

/** Reconnects an existing element to new endpoints and returns the updated summary. */
export function updateElement(path: string, request: ElementUpdateRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('element.update').element_update(path, JSON.stringify(request)));
}

/** Splits one element at a length fraction and returns the split identifiers. */
export function splitElement(path: string, request: ElementSplitRequest): ElementSplitReport {
    return decode<ElementSplitReport>(() => binding('element.split').element_split(path, JSON.stringify(request)));
}

/** Removes a node and returns the updated summary. */
export function nodeRemove(path: string, id: string): DocumentSummary {
    return decode<DocumentSummary>(() => binding('node.remove').node_remove(path, JSON.stringify({ id })));
}

/** Removes an element and returns the updated summary. */
export function elementRemove(path: string, id: string): DocumentSummary {
    return decode<DocumentSummary>(() => binding('element.remove').element_remove(path, JSON.stringify({ id })));
}

/** Sets or clears the bending-moment releases of a beam element. */
export function elementRelease(path: string, request: ElementReleaseRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('element.release').element_release(path, JSON.stringify(request)));
}

/** Sets or clears the tension-only (cable) flag of a truss element. */
export function elementTensionOnly(path: string, request: ElementTensionOnlyRequest): DocumentSummary {
    return decode<DocumentSummary>(() =>
        binding('element.tensionOnly').element_tension_only(path, JSON.stringify(request)),
    );
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

/** Reports derived properties of a parametric section. */
export function sectionProperties(section: Section, units: 'mm' | 'm'): SectionReport {
    return decode<SectionReport>(() =>
        binding('section.properties').section_properties(JSON.stringify({ section, units })),
    );
}

/** Adds or merges a support entry and returns the updated summary. */
export function supportAdd(path: string, request: SupportAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('support.add').support_add(path, JSON.stringify(request)));
}

/** Removes the support entry of a node and returns the updated summary. */
export function supportRemove(path: string, node: string): DocumentSummary {
    return decode<DocumentSummary>(() => binding('support.remove').support_remove(path, JSON.stringify({ node })));
}

/** Adds or updates a concentrated load and returns the updated summary. */
export function loadAdd(path: string, request: LoadAddRequest): DocumentSummary {
    return decode<DocumentSummary>(() => binding('load.add').load_add(path, JSON.stringify(request)));
}

/** Removes the concentrated load entry of a node and returns the updated summary. */
export function loadRemove(path: string, node: string): DocumentSummary {
    return decode<DocumentSummary>(() => binding('load.remove').load_remove(path, JSON.stringify({ node })));
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
/** Solves a document with tension-only (cable) truss members. */
export function solve(path: string, options: SolveOptions & { analysis: 'tensionOnly' }): TensionOnlySolution;
/** Solves a document, optionally writing the JSON report to `options.out`. */
export function solve(
    path: string,
    options: SolveOptions = {},
): Solution | ModalSolution | BucklingSolution | TensionOnlySolution {
    const request =
        options.out === undefined &&
        options.analysis === undefined &&
        options.modes === undefined &&
        options.iterations === undefined &&
        options.tolerance === undefined
            ? undefined
            : JSON.stringify({
                  out: options.out,
                  analysis: options.analysis,
                  modes: options.modes,
                  iterations: options.iterations,
                  tolerance: options.tolerance,
              });
    return decode<Solution | ModalSolution | BucklingSolution | TensionOnlySolution>(() =>
        request === undefined ? binding('solve').solve(path) : binding('solve').solve(path, request),
    );
}

/** Solves steady-state heat conduction and optionally writes the JSON report. */
export function heat(path: string, options: HeatOptions = {}): HeatSolution {
    return decode<HeatSolution>(() =>
        options.out === undefined
            ? binding('heat').heat(path)
            : binding('heat').heat(path, JSON.stringify({ out: options.out })),
    );
}

/** Solves a model with tension-only (cable) truss members. */
export function tensionOnly(path: string, options: TensionOnlyOptions = {}): TensionOnlySolution {
    const request =
        options.iterations === undefined && options.tolerance === undefined && options.out === undefined
            ? undefined
            : JSON.stringify({ iterations: options.iterations, tolerance: options.tolerance, out: options.out });
    return decode<TensionOnlySolution>(() =>
        request === undefined
            ? binding('tension.only').tension_only(path)
            : binding('tension.only').tension_only(path, request),
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

/** Returns the solved displacement of one node. */
export function nodeDisplacement(path: string, id: string): NodeDisplacement {
    return decode<NodeDisplacement>(() =>
        binding('node.displacement').node_displacement(path, JSON.stringify({ id })),
    );
}

/** Returns the solved member result of one element, including its end forces. */
export function elementResult(path: string, id: string): ElementResult {
    return decode<ElementResult>(() => binding('element.result').element_result(path, JSON.stringify({ id })));
}

/** Returns the support reaction of one node. */
export function nodeReaction(path: string, node: string): SupportReaction {
    return decode<SupportReaction>(() => binding('reaction.node').node_reaction(path, JSON.stringify({ node })));
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

/** Averages element-end stresses at shared nodes and reports the extremes. */
export function nodalStresses(path: string, options: NodalStressOptions = {}): NodalStressSolution {
    const request = options.stations === undefined && options.out === undefined
        ? undefined
        : JSON.stringify({ stations: options.stations, out: options.out });
    return decode<NodalStressSolution>(() =>
        request === undefined
            ? binding('stress.nodal').nodal_stresses(path)
            : binding('stress.nodal').nodal_stresses(path, request),
    );
}

/** Summarises the unfactored static case and every stored load combination. */
export function loadCases(path: string, options: LoadCaseOptions = {}): LoadCaseSummary {
    const request = options.out === undefined ? undefined : JSON.stringify({ out: options.out });
    return decode<LoadCaseSummary>(() =>
        request === undefined
            ? binding('load.cases').load_cases(path)
            : binding('load.cases').load_cases(path, request),
    );
}

/** Solves a response-spectrum analysis by modal superposition (SRSS or CQC). */
export function responseSpectrum(path: string, options: ResponseSpectrumOptions): ResponseSpectrumSolution {
    if (!Array.isArray(options.spectrum) || options.spectrum.length === 0) {
        throw new OpenCAEError('session/argument-invalid', '`spectrum` must contain at least one point', {
            details: { feature: 'spectrum' },
        });
    }
    return decode<ResponseSpectrumSolution>(() =>
        binding('spectrum.solve').response_spectrum(
            path,
            JSON.stringify({
                modes: options.modes,
                axis: options.axis,
                combination: options.combination,
                damping: options.damping,
                spectrum: options.spectrum,
                out: options.out,
            }),
        ),
    );
}

/** Solves the steady-state harmonic response over a frequency sweep. */
export function frequencyResponse(path: string, options: FrequencyResponseOptions): FrequencyResponseSolution {
    if (!(options.damping > 0)) {
        throw new OpenCAEError('session/argument-invalid', '`damping` must be a positive ratio', {
            details: { feature: 'frequency.response' },
        });
    }
    return decode<FrequencyResponseSolution>(() =>
        binding('frequency.response').frequency_response(
            path,
            JSON.stringify({
                modes: options.modes,
                axis: options.axis,
                damping: options.damping,
                startHz: options.startHz,
                endHz: options.endHz,
                points: options.points,
                out: options.out,
            }),
        ),
    );
}

/** Solves construction stages by activating and deactivating elements. */
export function constructionStages(
    path: string,
    stages: readonly ConstructionStage[],
    options: ConstructionStageOptions = {},
): ConstructionStageSolution {
    if (stages.length === 0) {
        throw new OpenCAEError('session/argument-invalid', '`stages` must contain at least one stage', {
            details: { feature: 'stage.solve' },
        });
    }
    return decode<ConstructionStageSolution>(() =>
        binding('stage.solve').construction_stages(path, JSON.stringify({ stages, out: options.out })),
    );
}

/** Solves a geometric-nonlinear truss model with a Newton iteration. */
export function geometricNonlinear(path: string, options: NonlinearOptions = {}): NonlinearSolution {
    return decode<NonlinearSolution>(() =>
        binding('nonlinear.truss').geometric_nonlinear(
            path,
            JSON.stringify({
                increments: options.increments,
                iterations: options.iterations,
                tolerance: options.tolerance,
                out: options.out,
            }),
        ),
    );
}

/** Exports the assembled global stiffness and lumped mass matrices. */
export function matrices(path: string, options: MatrixOptions = {}): SystemMatrices {
    const request = options.out === undefined ? undefined : JSON.stringify({ out: options.out });
    return decode<SystemMatrices>(() =>
        request === undefined ? binding('matrix.export').matrices(path) : binding('matrix.export').matrices(path, request),
    );
}

/** Exports a solved document. */
export function exportDocument(
    path: string,
    format: ExportFormat,
    target: string,
    options: ExportOptions = {},
): ExportReport {
    const request = options.scale === undefined ? undefined : JSON.stringify({ scale: options.scale });
    return decode<ExportReport>(() =>
        request === undefined
            ? binding('export').export(path, format, target)
            : binding('export').export(path, format, target, request),
    );
}

/** Solves a document and returns the full report. */
export function report(path: string): Report {
    return decode<Report>(() => binding('report').report(path));
}
