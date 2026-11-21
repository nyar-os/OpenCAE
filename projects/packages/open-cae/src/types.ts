/**
 * Declared types of `@wae/open-cae`.
 *
 * The shapes describe the real document: truss and beam elements, supports,
 * loads and one global material. Every editing call returns the summary of the
 * document as it was written to disk, so a caller never has to guess whether a
 * change reached the file.
 */

/** Declared unit system. The solver does not convert units. */
export type Units = 'mm' | 'm';

/** Spatial dimension of a document. */
export type Dimension = 2 | 3;

/** Element formulation. Beams are two-dimensional only. */
export type ElementKind = 'truss' | 'beam';

/** Analysis performed by `solve()`. */
export type Analysis = 'static' | 'modal' | 'buckling';

/** Disclosure level of one capability entry. */
export type SupportLevel = 'supported' | 'unsupported';

/** Severity of a validation issue. */
export type Severity = 'error' | 'warning' | 'notice';

/** Export formats `export()` can write. */
export type ExportFormat = 'csv' | 'vtk' | 'md';

/** Session options. Constructing a session opens no file. */
export interface OpenCAEOptions {
    /** BCP-47 tag used to pick the CLI locale. Defaults to `en-US`. */
    readonly locale?: string;
    /** Default unit system of documents created by this session. Defaults to `mm`. */
    readonly units?: Units;
    /** Default spatial dimension of documents created by this session. Defaults to `2`. */
    readonly dimension?: Dimension;
}

/** Options of `OpenCAE.create()`. */
export interface CreateOptions {
    readonly units?: Units;
    readonly dimension?: Dimension;
}

/** Global isotropic linear elastic material and section data. */
export interface Material {
    readonly name: string;
    /** Young's modulus: MPa for `mm`, Pa for `m`. */
    readonly youngsModulus: number;
    /** Cross-sectional area: mm² for `mm`, m² for `m`. */
    readonly area: number;
    /**
     * Mass density used by modal analysis and self-weight. Must be consistent
     * with the declared units, for example `7.85e-9 t/mm³` in `mm`.
     */
    readonly density: number;
}

/** Summary of one document as written to disk. */
export interface DocumentSummary {
    /** Opaque document identity, derived from the path. */
    readonly id: string;
    /** Path of the document file. */
    readonly path: string;
    /** Declared unit system, or `null` when the file does not declare one. */
    readonly units: Units | null;
    readonly dimension: Dimension;
    /** Monotonically increasing edit counter. */
    readonly revision: number;
    readonly nodes: number;
    readonly elements: number;
    readonly supports: number;
    readonly loads: number;
    readonly material: Material;
}

/** Result of `validate()`. */
export interface ValidationReport {
    /** `true` when no issue has `error` severity. */
    readonly ok: boolean;
    readonly issues: readonly Issue[];
}

/** One validation finding with a stable code. */
export interface Issue {
    readonly code: string;
    readonly severity: Severity;
    readonly message: string;
}

/** Nodal displacement. */
export interface NodeDisplacement {
    readonly id: string;
    readonly ux: number;
    readonly uy: number;
    readonly uz: number;
    /** Rotation about Z; non-zero only at beam nodes in two dimensions. */
    readonly rz: number;
    readonly magnitude: number;
}

/** Nodal reaction residual `K·u − F`. */
export interface NodeReaction {
    readonly node: string;
    readonly fx: number;
    readonly fy: number;
    readonly fz: number;
    /** Reaction moment about Z; non-zero only at beam nodes in two dimensions. */
    readonly mz: number;
}

/**
 * Local end forces `f = K·d − f_eq`, i.e. the generalized forces applied to
 * the element ends by the nodes. The separate `axialForce` remains positive in
 * tension.
 */
export interface EndForces {
    readonly n1: number;
    readonly v1: number;
    readonly m1: number;
    readonly n2: number;
    readonly v2: number;
    readonly m2: number;
}

/** Member result. Axial force is positive in tension. */
export interface ElementResult {
    readonly id: string;
    readonly from: string;
    readonly to: string;
    readonly kind: ElementKind;
    readonly length: number;
    readonly axialForce: number;
    readonly stress: number;
    readonly strain: number;
    readonly endForces: EndForces;
    readonly maxAbsMoment: number;
}

/** Element carrying the largest absolute axial force. */
export interface MaxAxialForce {
    readonly element: string;
    readonly axialForce: number;
}

/** Element carrying the largest absolute bending moment. */
export interface MaxMoment {
    readonly element: string;
    readonly moment: number;
}

/** Complete linear-static solution. */
export interface Solution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    readonly nodes: readonly NodeDisplacement[];
    readonly reactions: readonly NodeReaction[];
    readonly elements: readonly ElementResult[];
    readonly strainEnergy: number;
    readonly maxDisplacement: NodeDisplacement;
    readonly maxAxialForce: MaxAxialForce;
    readonly maxMoment: MaxMoment;
}

/** Internal force result at one station along an element. */
export interface SectionForces {
    /** Distance from the `from` node along the element. */
    readonly position: number;
    /** Axial force, positive in tension. */
    readonly axialForce: number;
    readonly shear: number;
    readonly moment: number;
}

/** Sampled internal forces of one element. */
export interface ElementDiagram {
    readonly id: string;
    readonly from: string;
    readonly to: string;
    readonly kind: ElementKind;
    readonly length: number;
    readonly maxAbsShear: number;
    readonly maxAbsMoment: number;
    readonly stations: readonly SectionForces[];
}

/** Internal force diagrams returned by `internalForces()`. */
export interface InternalForceSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Number of stations sampled per element, including both ends. */
    readonly stations: number;
    readonly elements: readonly ElementDiagram[];
}

/** Options of `internalForces()`. */
export interface InternalForceOptions {
    /** Number of stations per element including both ends; defaults to 11. */
    readonly stations?: number;
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** Linear combination of applied loads and self-weight. */
export interface LoadCombination {
    readonly name: string;
    /** Factor applied to nodal and element applied loads. */
    readonly loads: number;
    /** Factor applied to the gravity vector. */
    readonly gravity: number;
}

/** One solved load combination. */
export interface CombinationCase {
    readonly name: string;
    readonly loads: number;
    readonly gravity: number;
    readonly solution: Solution;
}

/** Governing value of one envelope entry. */
export interface EnvelopeValue {
    readonly combination: string;
    readonly entity: string;
    /** Signed value; the comparison used its absolute magnitude. */
    readonly value: number;
}

/** Displacement envelope of one node. */
export interface NodeEnvelope {
    readonly node: string;
    readonly maxAbsDisplacement: number;
    readonly combination: string;
}

/** Force envelope of one element. */
export interface ElementEnvelope {
    readonly element: string;
    readonly minAxialForce: number;
    readonly maxAxialForce: number;
    readonly maxAbsMoment: number;
    readonly momentCombination: string;
}

/** Envelope of several load combinations. */
export interface CombinationEnvelope {
    readonly maxDisplacement: EnvelopeValue;
    readonly maxAbsAxialForce: EnvelopeValue;
    readonly maxAbsMoment: EnvelopeValue;
    readonly nodes: readonly NodeEnvelope[];
    readonly elements: readonly ElementEnvelope[];
}

/** Result of `solveCombinations()`. */
export interface CombinationSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    readonly combinations: readonly CombinationCase[];
    readonly envelope: CombinationEnvelope;
}

/** Request of `addCombination()`. */
export interface CombinationAddRequest {
    readonly name: string;
    /** Factor applied to nodal and element applied loads; defaults to 1. */
    readonly loads?: number;
    /** Factor applied to the gravity vector; defaults to 0. */
    readonly gravity?: number;
}

/** Options of `solveCombinations()`. */
export interface CombinationSolveOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** Nodal value of one mass-normalized mode shape. */
export interface ModeShape {
    readonly node: string;
    readonly ux: number;
    readonly uy: number;
    readonly uz: number;
    readonly rz: number;
}

/** One free vibration mode. */
export interface Mode {
    readonly frequencyHz: number;
    readonly eigenvalue: number;
    readonly maxAmplitude: number;
    readonly shape: readonly ModeShape[];
}

/** Modal solution returned by `solve({ analysis: 'modal' })`. */
export interface ModalSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Requested number of modes. */
    readonly requested: number;
    /** Zero, negative or non-finite eigenvalues that were skipped. */
    readonly skipped: number;
    readonly modes: readonly Mode[];
}

/** One linear buckling mode. */
export interface BucklingMode {
    /**
     * Load factor `λ` that multiplies the static reference axial forces at the
     * onset of instability. The critical load is `λ` times the static load case.
     */
    readonly factor: number;
    /** Largest absolute nodal shape value after normalization to one. */
    readonly maxAmplitude: number;
    /** Mode shape in model node order. */
    readonly shape: readonly ModeShape[];
}

/** Buckling solution returned by `solve({ analysis: 'buckling' })`. */
export interface BucklingSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Requested number of modes. */
    readonly requested: number;
    /** Eigenpairs skipped because they did not yield a finite positive factor. */
    readonly skipped: number;
    /** Modes in ascending load-factor order. */
    readonly modes: readonly BucklingMode[];
}

/** Full report returned by `report()`. */
export interface Report {
    readonly format: string;
    readonly version: number;
    readonly document: DocumentSummary;
    readonly issues: readonly Issue[];
    readonly solution: Solution;
    /** Lowest five modes when the material density is positive. */
    readonly modal: ModalSolution | null;
}

/** Result of `export()`. */
export interface ExportReport {
    readonly format: ExportFormat;
    /** Target path exactly as the caller supplied it. */
    readonly path: string;
    /** Concrete files created by the export. */
    readonly files: readonly string[];
}

/** Entry of the capability matrix. */
export interface OpenCAECapabilities {
    readonly engine: { readonly package: string; readonly version: string };
    readonly solver: Readonly<Record<string, SupportLevel>>;
    readonly io: Readonly<Record<string, SupportLevel>>;
}

/** Request of `addNode()`. */
export interface NodeAddRequest {
    readonly id: string;
    readonly x: number;
    readonly y: number;
    /** Only valid in three dimensions; defaults to `0`. */
    readonly z?: number;
}

/** Request of `addElement()`. */
export interface ElementAddRequest {
    readonly id: string;
    readonly from: string;
    readonly to: string;
    /** Element formulation; defaults to `truss`. */
    readonly kind?: ElementKind;
    /** Optional per-element Young's modulus override. */
    readonly youngsModulus?: number;
    /** Optional per-element area override. */
    readonly area?: number;
    /** Second moment of area; required and positive for beams. */
    readonly inertia?: number;
    /**
     * Distributed load per unit length at the start node in the element's
     * local `+y` direction. Only valid on beams.
     */
    readonly distributedLoad?: number;
    /**
     * Distributed load per unit length at the end node. Together with
     * `distributedLoad` this is a linearly varying (trapezoidal) load; omitting
     * it keeps the load uniform.
     */
    readonly distributedLoadEnd?: number;
}

/** Request of `setMaterial()`. */
export interface MaterialSetRequest {
    readonly youngsModulus: number;
    readonly area: number;
    /** Mass density, required by modal analysis and self-weight. */
    readonly density?: number;
    readonly name?: string;
}

/**
 * Request of `addSupport()`. At least one constraint flag or prescribed value
 * must be present.
 */
export interface SupportAddRequest {
    readonly node: string;
    readonly dx?: boolean;
    readonly dy?: boolean;
    readonly dz?: boolean;
    /** Rotation about Z; only valid at beam nodes in two dimensions. */
    readonly rz?: boolean;
    /**
     * Enforced displacement along X; implies `dx`. Non-zero values model a
     * support settlement or a driven displacement.
     */
    readonly ux?: number;
    /** Enforced displacement along Y; implies `dy`. */
    readonly uy?: number;
    /** Enforced displacement along Z; implies `dz` and is valid in 3D only. */
    readonly uz?: number;
    /** Enforced rotation about Z in radians; implies `rz`. */
    readonly rotation?: number;
}

/** Request of `addLoad()`. */
export interface LoadAddRequest {
    readonly node: string;
    readonly fx?: number;
    readonly fy?: number;
    readonly fz?: number;
    /** Concentrated moment about Z; only valid at beam nodes. */
    readonly mz?: number;
}

/** Request of `setGravity()`. The zero vector clears gravity. */
export interface GravitySetRequest {
    readonly gx?: number;
    readonly gy?: number;
    readonly gz?: number;
}

/** Options of `solve()`. */
export interface SolveOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
    /** `static` (default), `modal` or `buckling`. */
    readonly analysis?: Analysis;
    /** Number of modes for modal or buckling analysis; defaults to 5. */
    readonly modes?: number;
}
