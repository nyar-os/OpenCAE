/**
 * Path-backed document.
 *
 * `OpenCAE.create()` and `OpenCAE.open()` return one of these. Every editing
 * method forwards to the native session, which writes the file atomically, and
 * stores the summary it returns so `id`, `units`, `dimension` and `revision`
 * always describe what is on disk.
 */

import { OpenCAEError } from './errors.js';
import * as service from './service.js';
import type {
    BucklingCombinationOptions,
    BucklingCombinationSolution,
    BucklingSolution,
    CombinationAddRequest,
    CombinationSolveOptions,
    CombinationSolution,
    DeflectionOptions,
    DeflectionSolution,
    Dimension,
    DocumentSummary,
    ElementAddRequest,
    ElementReleaseRequest,
    ExportFormat,
    ExportReport,
    GravitySetRequest,
    InternalForceOptions,
    InternalForceSolution,
    LoadAddRequest,
    Material,
    MaterialPresetRequest,
    MaterialSetRequest,
    ModalSolution,
    NodeAddRequest,
    NodalStressOptions,
    NodalStressSolution,
    ReactionOptions,
    ReactionSummary,
    Report,
    Solution,
    SolveOptions,
    StressOptions,
    StressSolution,
    SupportAddRequest,
    Units,
    ValidationReport,
} from './types.js';

/** One open document. */
export class CaeDocument {
    private state: DocumentSummary;
    private closed = false;

    constructor(summary: DocumentSummary) {
        this.state = summary;
    }

    /** Document identity, derived from the path. */
    get id(): string {
        return this.state.id;
    }

    /** Path of the document file. */
    get path(): string {
        return this.state.path;
    }

    /** Committed revision. */
    get revision(): number {
        return this.state.revision;
    }

    /** Declared unit system, or `null` when the file does not declare one. */
    get units(): Units | null {
        return this.state.units;
    }

    /** Spatial dimension. */
    get dimension(): Dimension {
        return this.state.dimension;
    }

    /** Global material and section data. */
    get material(): Material {
        return this.state.material;
    }

    /** Number of nodes. */
    get nodes(): number {
        return this.state.nodes;
    }

    /** Number of truss and beam elements. */
    get elements(): number {
        return this.state.elements;
    }

    /** Number of support entries. */
    get supports(): number {
        return this.state.supports;
    }

    /** Number of concentrated load entries. */
    get loads(): number {
        return this.state.loads;
    }

    /** Whether {@link close} has run. */
    get isClosed(): boolean {
        return this.closed;
    }

    /** Latest summary returned by the native session. */
    summary(): DocumentSummary {
        return this.state;
    }

    /** Adds a node and saves the file. */
    addNode(request: NodeAddRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.nodeAdd(this.path, request));
    }

    /** Adds a truss or beam element and saves the file. */
    addElement(request: ElementAddRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementAdd(this.path, request));
    }

    /**
     * Sets or clears the bending-moment releases of a beam element and saves
     * the file. `{ start: false, end: false }` clears the release.
     */
    setElementRelease(request: ElementReleaseRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementRelease(this.path, request));
    }

    /** Replaces the global material and saves the file. */
    setMaterial(request: MaterialSetRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.materialSet(this.path, request));
    }

    /** Applies a built-in material preset and saves the file. */
    applyMaterialPreset(request: MaterialPresetRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.materialPreset(this.path, request));
    }

    /** Adds or merges a support entry and saves the file. */
    addSupport(request: SupportAddRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.supportAdd(this.path, request));
    }

    /** Adds or updates a concentrated load and saves the file. */
    addLoad(request: LoadAddRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.loadAdd(this.path, request));
    }

    /** Sets the gravity acceleration vector and saves the file. */
    setGravity(request: GravitySetRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.setGravity(this.path, request));
    }

    /** Validates the document without solving it. */
    validate(): ValidationReport {
        this.assertOpen();
        return service.validate(this.path);
    }

    /** Solves the document for a static load case. */
    solve(options?: SolveOptions & { analysis?: 'static' }): Solution;
    /** Solves the document for `options.modes` natural frequencies. */
    solve(options: SolveOptions & { analysis: 'modal' }): ModalSolution;
    /** Solves the document for `options.modes` linear buckling load factors. */
    solve(options: SolveOptions & { analysis: 'buckling' }): BucklingSolution;
    /** Solves the document, optionally writing the JSON report to `out`. */
    solve(options: SolveOptions = {}): Solution | ModalSolution | BucklingSolution {
        this.assertOpen();
        if (options.analysis === 'modal') {
            return service.solve(this.path, { ...options, analysis: 'modal' });
        }
        if (options.analysis === 'buckling') {
            return service.solve(this.path, { ...options, analysis: 'buckling' });
        }
        return service.solve(this.path, options as SolveOptions & { analysis?: 'static' });
    }

    /** Adds or replaces a linear load combination and saves the file. */
    addCombination(request: CombinationAddRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.addCombination(this.path, request));
    }

    /** Removes a load combination and returns the updated summary. */
    removeCombination(name: string): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.removeCombination(this.path, name));
    }

    /** Solves every load combination and returns the solutions plus envelope. */
    solveCombinations(options: CombinationSolveOptions = {}): CombinationSolution {
        this.assertOpen();
        return service.solveCombinations(this.path, options);
    }

    /** Solves linear buckling for every load combination and returns the factors. */
    solveBucklingCombinations(options: BucklingCombinationOptions = {}): BucklingCombinationSolution {
        this.assertOpen();
        return service.solveBucklingCombinations(this.path, options);
    }

    /** Samples internal forces at equally spaced stations along each element. */
    internalForces(options: InternalForceOptions = {}): InternalForceSolution {
        this.assertOpen();
        return service.internalForces(this.path, options);
    }

    /** Returns support reactions and the global force equilibrium check. */
    reactions(options: ReactionOptions = {}): ReactionSummary {
        this.assertOpen();
        return service.reactions(this.path, options);
    }

    /** Recovers extreme-fibre stresses and utilisation per element. */
    stresses(options: StressOptions = {}): StressSolution {
        this.assertOpen();
        return service.stresses(this.path, options);
    }

    /** Samples the deflected centreline of every element. */
    deflections(options: DeflectionOptions = {}): DeflectionSolution {
        this.assertOpen();
        return service.deflections(this.path, options);
    }

    /** Averages element-end stresses at shared nodes and reports the extremes. */
    nodalStresses(options: NodalStressOptions = {}): NodalStressSolution {
        this.assertOpen();
        return service.nodalStresses(this.path, options);
    }

    /** Solves the document and returns the full report. */
    report(): Report {
        this.assertOpen();
        return service.report(this.path);
    }

    /** Exports the solved document as `csv`, `vtk` or `md`. */
    export(format: ExportFormat, target: string): ExportReport {
        this.assertOpen();
        assertTarget(target);
        return service.exportDocument(this.path, format, target);
    }

    /** Writes the document back to its own path, or to `target`. */
    save(target?: string): DocumentSummary {
        this.assertOpen();
        if (target !== undefined) {
            assertTarget(target);
        }
        return this.adopt(service.save(this.path, target));
    }

    /** Releases this document. Idempotent. */
    close(): void {
        this.closed = true;
    }

    private adopt(summary: DocumentSummary): DocumentSummary {
        this.state = summary;
        return summary;
    }

    private assertOpen(): void {
        if (this.closed) {
            throw new OpenCAEError('document/closed', `document \`${this.path}\` has been closed`);
        }
    }
}

function assertTarget(target: string): void {
    if (typeof target !== 'string' || target.length === 0) {
        throw new OpenCAEError('session/argument-invalid', '`target` must be a non-empty string', {
            details: { feature: 'document.target' },
        });
    }
}
