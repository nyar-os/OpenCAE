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
    AmbientTemperatureRequest,
    BucklingCombinationOptions,
    BucklingCombinationSolution,
    BucklingSolution,
    CombinationAddRequest,
    CombinationSolveOptions,
    CombinationSolution,
    ConstructionStage,
    ConstructionStageOptions,
    ConstructionStageSolution,
    DeflectionOptions,
    DeflectionSolution,
    Dimension,
    DocumentSummary,
    ElementAddRequest,
    ElementLoadSpanRequest,
    ElementReleaseRequest,
    ElementResult,
    ElementSplitReport,
    ElementSplitRequest,
    ElementTemperatureGradientRequest,
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
    InfluenceLineOptions,
    InfluenceLineSolution,
    InfluenceTrainOptions,
    InfluenceTrainSolution,
    InternalForceOptions,
    InternalForceSolution,
    LoadAddRequest,
    LoadCaseOptions,
    LoadCaseSummary,
    Material,
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
    ReactionOptions,
    ReactionSummary,
    Report,
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

    /**
     * Moves a node in place, keeping its identifier and every reference to it,
     * and saves the file. Only the supplied coordinates change.
     */
    updateNode(request: NodeUpdateRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.updateNode(this.path, request));
    }

    /**
     * Sets or clears the concentrated mass of a node and saves the file. `0`
     * clears it; the mass is added to the node's translational degrees of
     * freedom by modal analysis.
     */
    setNodeMass(request: NodeMassRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.setNodeMass(this.path, request));
    }

    /**
     * Sets or clears the fixed temperature of a node and saves the file.
     * `temperature: null` removes the Dirichlet boundary; the value is used only
     * by {@link heat}.
     */
    setNodeTemperature(request: NodeTemperatureRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.setNodeTemperature(this.path, request));
    }

    /**
     * Sets the concentrated nodal heat flow in watts and saves the file.
     * `0` clears it; the value is used only by {@link heat}.
     */
    setNodeHeat(request: NodeHeatRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.setNodeHeat(this.path, request));
    }

    /**
     * Merges one node into another and saves the file. The `into` node keeps its
     * coordinates; element endpoints, supports and loads of `from` move over.
     * Conflicting supports and an element that would collapse are rejected.
     */
    mergeNodes(request: NodeMergeRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.mergeNodes(this.path, request));
    }

    /**
     * Removes a node and saves the file. A node referenced by an element, a
     * support or a load is rejected with `node/referenced-by-*`.
     */
    removeNode(id: string): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.nodeRemove(this.path, id));
    }

    /** Adds a truss or beam element and saves the file. */
    addElement(request: ElementAddRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementAdd(this.path, request));
    }

    /**
     * Updates an element in place and saves the file. Endpoints, distributed
     * load, temperature and convection can change independently; omitted fields
     * keep their current value. `clearLoad`/`clearTemperature` remove the
     * respective actions. The call causes exactly one revision.
     */
    updateElement(request: ElementUpdateRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.updateElement(this.path, request));
    }

    /**
     * Splits an element at a fraction of its length and saves the file. An
     * existing node at the split point is reused, otherwise one is created. Both
     * segments keep the material, section, uniform load and temperature;
     * partial-span, linearly varying and gradient loads are rejected with
     * `element/split-unsupported-load`.
     */
    splitElement(request: ElementSplitRequest): ElementSplitReport {
        this.assertOpen();
        const report = service.splitElement(this.path, request);
        this.adopt(report);
        return report;
    }

    /** Removes an element and saves the file. */
    removeElement(id: string): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementRemove(this.path, id));
    }

    /**
     * Sets or clears the bending-moment releases of a beam element and saves
     * the file. `{ start: false, end: false }` clears the release.
     */
    setElementRelease(request: ElementReleaseRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementRelease(this.path, request));
    }

    /** Sets or clears the tension-only (cable) flag of a truss element. */
    setElementTensionOnly(request: ElementTensionOnlyRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementTensionOnly(this.path, request));
    }

    /**
     * Sets or clears the linear temperature gradient of a beam element and
     * saves the file. `gradient: null` removes it and keeps the uniform
     * temperature.
     */
    setElementTemperatureGradient(request: ElementTemperatureGradientRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementGradient(this.path, request));
    }

    /**
     * Sets or clears the loaded segment of a beam element's distributed load
     * and saves the file. `span: null` restores a load over the whole member.
     */
    setElementLoadSpan(request: ElementLoadSpanRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.elementSpan(this.path, request));
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

    /** Removes the support entry of a node and saves the file. */
    removeSupport(node: string): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.supportRemove(this.path, node));
    }

    /** Adds or updates a concentrated load and saves the file. */
    addLoad(request: LoadAddRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.loadAdd(this.path, request));
    }

    /** Removes the concentrated load entry of a node and saves the file. */
    removeLoad(node: string): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.loadRemove(this.path, node));
    }

    /** Sets the gravity acceleration vector and saves the file. */
    setGravity(request: GravitySetRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.setGravity(this.path, request));
    }

    /**
     * Sets or clears the ambient temperature used by surface convection and
     * saves the file. `temperature: null` removes it; the value is used only by
     * {@link heat}.
     */
    setAmbientTemperature(request: AmbientTemperatureRequest): DocumentSummary {
        this.assertOpen();
        return this.adopt(service.setAmbientTemperature(this.path, request));
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
    /** Solves the document with tension-only (cable) truss members. */
    solve(options: SolveOptions & { analysis: 'tensionOnly' }): TensionOnlySolution;
    /** Solves the document, optionally writing the JSON report to `out`. */
    solve(options: SolveOptions = {}): Solution | ModalSolution | BucklingSolution | TensionOnlySolution {
        this.assertOpen();
        if (options.analysis === 'modal') {
            return service.solve(this.path, { ...options, analysis: 'modal' });
        }
        if (options.analysis === 'buckling') {
            return service.solve(this.path, { ...options, analysis: 'buckling' });
        }
        if (options.analysis === 'tensionOnly') {
            return service.solve(this.path, { ...options, analysis: 'tensionOnly' });
        }
        return service.solve(this.path, options as SolveOptions & { analysis?: 'static' });
    }

    /** Solves the document with tension-only (cable) truss members. */
    tensionOnly(options: TensionOnlyOptions = {}): TensionOnlySolution {
        this.assertOpen();
        return service.tensionOnly(this.path, options);
    }

    /**
     * Solves steady-state axial heat conduction and optionally writes the JSON
     * report. The result contains temperatures and heat flows only; it is not a
     * structural solution.
     */
    heat(options: HeatOptions = {}): HeatSolution {
        this.assertOpen();
        return service.heat(this.path, options);
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

    /** Builds an influence line as a unit load moves along a track beam element. */
    influenceLine(options: InfluenceLineOptions): InfluenceLineSolution {
        this.assertOpen();
        return service.influenceLine(this.path, options);
    }

    /** Builds the moving-load-train envelope of an influence line. */
    influenceLineTrain(options: InfluenceTrainOptions): InfluenceTrainSolution {
        this.assertOpen();
        return service.influenceLineTrain(this.path, options);
    }

    /** Returns support reactions and the global force equilibrium check. */
    reactions(options: ReactionOptions = {}): ReactionSummary {
        this.assertOpen();
        return service.reactions(this.path, options);
    }

    /** Returns the solved displacement of one node by identifier. */
    nodeDisplacement(id: string): NodeDisplacement {
        this.assertOpen();
        return service.nodeDisplacement(this.path, id);
    }

    /** Returns the solved member result of one element, including its end forces. */
    elementResult(id: string): ElementResult {
        this.assertOpen();
        return service.elementResult(this.path, id);
    }

    /** Returns the support reaction of one node by identifier. */
    nodeReaction(node: string): SupportReaction {
        this.assertOpen();
        return service.nodeReaction(this.path, node);
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

    /** Summarises the unfactored static case and every stored load combination. */
    loadCases(options: LoadCaseOptions = {}): LoadCaseSummary {
        this.assertOpen();
        return service.loadCases(this.path, options);
    }

    /** Exports the assembled global stiffness and lumped mass matrices. */
    matrices(options: MatrixOptions = {}): SystemMatrices {
        this.assertOpen();
        return service.matrices(this.path, options);
    }

    /** Solves a response-spectrum analysis with SRSS or CQC modal combination. */
    responseSpectrum(options: ResponseSpectrumOptions): ResponseSpectrumSolution {
        this.assertOpen();
        return service.responseSpectrum(this.path, options);
    }

    /** Solves the steady-state harmonic response over a frequency sweep. */
    frequencyResponse(options: FrequencyResponseOptions): FrequencyResponseSolution {
        this.assertOpen();
        return service.frequencyResponse(this.path, options);
    }

    /** Solves construction stages by activating and deactivating elements. */
    constructionStages(stages: readonly ConstructionStage[], options: ConstructionStageOptions = {}): ConstructionStageSolution {
        this.assertOpen();
        return service.constructionStages(this.path, stages, options);
    }

    /** Solves a geometric-nonlinear truss model with a Newton iteration. */
    geometricNonlinear(options: NonlinearOptions = {}): NonlinearSolution {
        this.assertOpen();
        return service.geometricNonlinear(this.path, options);
    }

    /** Solves the document and returns the full report. */
    report(): Report {
        this.assertOpen();
        return service.report(this.path);
    }

    /**
     * Exports the solved document as `csv`, `vtk`, `md`, `html` or `svg`.
     * `options.scale` multiplies the nodal displacements of an SVG deformed
     * shape and defaults to `1`.
     */
    export(format: ExportFormat, target: string, options: ExportOptions = {}): ExportReport {
        this.assertOpen();
        assertTarget(target);
        return service.exportDocument(this.path, format, target, options);
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
