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
export type Analysis = 'static' | 'modal' | 'buckling' | 'tensionOnly';

/** Disclosure level of one capability entry. */
export type SupportLevel = 'supported' | 'unsupported';

/** Severity of a validation issue. */
export type Severity = 'error' | 'warning' | 'notice';

/** Export formats `export()` can write. */
export type ExportFormat = 'csv' | 'vtk' | 'md' | 'html' | 'svg';

/** Options of `export()`. */
export interface ExportOptions {
    /**
     * Scale factor applied to nodal displacements when drawing the deformed
     * shape of an SVG export. Must be finite and positive; defaults to `1`.
     */
    readonly scale?: number;
}

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

/** Parametric cross-section geometry. Dimensions use the document units. */
export type Section =
    | { readonly shape: 'rectangle'; readonly width: number; readonly height: number }
    | { readonly shape: 'circle'; readonly diameter: number }
    | {
          readonly shape: 'i';
          readonly depth: number;
          readonly flangeWidth: number;
          readonly flangeThickness: number;
          readonly webThickness: number;
      }
    | {
          readonly shape: 'box';
          readonly width: number;
          readonly height: number;
          readonly wall: number;
      }
    | {
          readonly shape: 'tube';
          readonly outerDiameter: number;
          readonly innerDiameter: number;
      }
    | {
          readonly shape: 'channel';
          readonly depth: number;
          readonly flangeWidth: number;
          readonly flangeThickness: number;
          readonly webThickness: number;
      };

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
    /** Permissible stress for utilisation checks, in the document stress unit. */
    readonly allowableStress?: number;
    /** Coefficient of thermal expansion in `1/K`; required by thermal loads. */
    readonly alpha?: number;
    /**
     * Thermal conductivity `k` in `W/(m·K)`; required only by `heat()`.
     *
     * Units are never converted, so a millimetre document must supply the
     * length-consistent conductivity.
     */
    readonly conductivity?: number;
    /** Parametric section geometry, when declared. */
    readonly section?: Section;
}

/** One built-in material preset, expressed in the requested units. */
export interface MaterialPreset {
    readonly name: string;
    readonly youngsModulus: number;
    readonly density: number;
    readonly allowableStress: number | null;
}

/** One supported parametric section shape. */
export interface SectionShape {
    readonly shape: 'rectangle' | 'circle' | 'i' | 'box' | 'tube' | 'channel';
    readonly parameters: readonly string[];
}

/** Built-in material and section library. */
export interface MaterialLibrary {
    readonly units: Units;
    readonly materials: readonly MaterialPreset[];
    readonly sections: readonly SectionShape[];
}

/** Derived properties of one parametric section. */
export interface SectionReport {
    /** Shape identifier, matching the `shape` tag of the section. */
    readonly shape: Section['shape'];
    /** Unit system the dimensions were interpreted in. */
    readonly units: Units;
    /** Cross-sectional area. */
    readonly area: number;
    /** Second moment of area about the strong (bending) axis. */
    readonly inertiaStrong: number;
    /** Second moment of area about the weak axis. */
    readonly inertiaWeak: number;
    /** Saint-Venant torsion constant. */
    readonly torsionConstant: number;
    /** Distance from the neutral axis to the extreme fibre. */
    readonly extremeFiber: number;
    /** Elastic section modulus `I_strong / c`. */
    readonly sectionModulus: number;
    /** Radius of gyration about the strong axis, `sqrt(I_strong / A)`. */
    readonly radiusOfGyrationStrong: number;
    /** Radius of gyration about the weak axis, `sqrt(I_weak / A)`. */
    readonly radiusOfGyrationWeak: number;
}

/** One polygon vertex as an `[x, y]` pair in document units. */
export type PolygonPoint = readonly [number, number];

/**
 * Derived properties of a closed polygon section.
 *
 * The second moments are taken about centroidal axes. Vertices may be given in
 * either winding order and the closing vertex may be omitted; the engine
 * normalises the outline to a counter-clockwise winding so `ixy` follows one
 * fixed sign convention.
 */
export interface PolygonSectionReport {
    /** Shape identifier, always `"polygon"`. */
    readonly shape: 'polygon';
    /** Unit system the vertices were interpreted in. */
    readonly units: Units;
    /** Cross-sectional area. */
    readonly area: number;
    /** Centroid `x` coordinate. */
    readonly centroidX: number;
    /** Centroid `y` coordinate. */
    readonly centroidY: number;
    /** Second moment of area about the centroidal `x` axis, `∫ y² dA`. */
    readonly ix: number;
    /** Second moment of area about the centroidal `y` axis, `∫ x² dA`. */
    readonly iy: number;
    /** Product of inertia about the centroidal axes, `∫ x·y dA`. */
    readonly ixy: number;
    /** Polar moment of area about the centroid, `J = Ix + Iy`. */
    readonly polarMoment: number;
    /** Radius of gyration about the centroidal `x` axis, `sqrt(Ix / A)`. */
    readonly radiusOfGyrationX: number;
    /** Radius of gyration about the centroidal `y` axis, `sqrt(Iy / A)`. */
    readonly radiusOfGyrationY: number;
}

/**
 * Derived properties of a parametric standard section (I, rectangular tube,
 * channel, equal-thickness L, T, solid ellipse or equal-thickness Z).
 *
 * Coordinates use the lower-left corner of the bounding rectangle: `x` runs
 * across the width and `y` along the depth, and the second moments are taken
 * about centroidal axes. `polarMoment` keeps the polygon report key but carries
 * the Saint-Venant torsion constant `J`, which equals `Ix + Iy` only for a
 * circular section.
 */
export interface StandardSectionReport {
    /** Shape identifier: `"i"`, `"tube"`, `"channel"`, `"angle"`, `"t"`, `"ellipse"`, `"triangle"` or `"z"`. */
    readonly shape: 'i' | 'tube' | 'channel' | 'angle' | 't' | 'ellipse' | 'triangle' | 'z';
    /** Unit system the dimensions were interpreted in. */
    readonly units: Units;
    /** Cross-sectional area. */
    readonly area: number;
    /** Centroid `x` coordinate from the lower-left corner. */
    readonly centroidX: number;
    /** Centroid `y` coordinate from the lower-left corner. */
    readonly centroidY: number;
    /** Second moment of area about the centroidal `x` axis. */
    readonly ix: number;
    /** Second moment of area about the centroidal `y` axis. */
    readonly iy: number;
    /** Product of inertia about the centroidal axes. */
    readonly ixy: number;
    /** Saint-Venant torsion constant `J` (thin-walled approximation). */
    readonly polarMoment: number;
    /** Radius of gyration about the centroidal `x` axis, `sqrt(Ix / A)`. */
    readonly radiusOfGyrationX: number;
    /** Radius of gyration about the centroidal `y` axis, `sqrt(Iy / A)`. */
    readonly radiusOfGyrationY: number;
}

/**
 * Geometry of a doubly symmetric I-section.
 *
 * `height` is the total depth `h`, `width` the flange width `b`,
 * `webThickness` the web thickness `tw` and `flangeThickness` the flange
 * thickness `tf`; `0 < tf < h/2` and `0 < tw < b` are required.
 */
export interface ISectionGeometry {
    readonly height: number;
    readonly width: number;
    readonly webThickness: number;
    readonly flangeThickness: number;
}

/**
 * Geometry of a hollow rectangular tube: total `height` `h`, total `width`
 * `b` and wall `thickness` `t`, with `0 < t < min(b, h)/2`.
 */
export interface RectangularTubeGeometry {
    readonly height: number;
    readonly width: number;
    readonly thickness: number;
}

/**
 * Geometry of an open channel whose opening faces right.
 *
 * `height` is the total depth `h`, `width` the flange width `b`,
 * `webThickness` the web thickness `tw` and `flangeThickness` the flange
 * thickness `tf`; `0 < tf < h/2` and `0 < tw < b` are required.
 */
export interface ChannelSectionGeometry {
    readonly height: number;
    readonly width: number;
    readonly webThickness: number;
    readonly flangeThickness: number;
}

/**
 * Geometry of an equal-thickness L (angle) section: total `height` `h`,
 * horizontal leg `width` `w` and leg `thickness` `t`, with
 * `0 < t < min(h, w)`. The `t × t` corner belongs to both legs and is counted
 * once.
 */
export interface AngleSectionGeometry {
    readonly height: number;
    readonly width: number;
    readonly thickness: number;
}

/**
 * Geometry of a T-section whose flange is at the top: total `height` `h`,
 * flange `width` `b`, `webThickness` `tw` and `flangeThickness` `tf`, with
 * `0 < tf < h` and `0 < tw < b`.
 */
export interface TSectionGeometry {
    readonly height: number;
    readonly width: number;
    readonly webThickness: number;
    readonly flangeThickness: number;
}

/**
 * Geometry of a solid ellipse: full extent `width` `w` along `x` and full
 * extent `height` `h` along `y`, with `w > 0` and `h > 0`. The semi-axes are
 * `a = w/2` and `b = h/2`, and the centroid is `(a, b)` from the lower-left
 * corner of the bounding rectangle.
 */
export interface EllipseSectionGeometry {
    readonly width: number;
    readonly height: number;
}

/**
 * Geometry of a solid equilateral triangle: side `a`, with `a > 0`. The base
 * lies along the bottom edge of the bounding box and the apex is at the top
 * centre, so the bounding box is `a` wide and `a·√3/2` tall with the origin at
 * the lower-left corner.
 */
export interface TriangleSectionGeometry {
    readonly side: number;
}

/**
 * Geometry of an equal-thickness Z-section: total `height` `h`, overall
 * `width` `w` and uniform `thickness` `t`, with `0 < t < w` and `2t < h`. The
 * origin is the lower-left corner of the bounding rectangle, the web is
 * centred and the top and bottom flanges extend to opposite edges.
 */
export interface ZSectionGeometry {
    readonly height: number;
    readonly width: number;
    readonly thickness: number;
}

/** Request of `applyMaterialPreset()`. */
export interface MaterialPresetRequest {
    /** Built-in preset name, for example `steel` or `aluminum`. */
    readonly preset: string;
    /** Area override; derived from the section when omitted. */
    readonly area?: number;
    /** Parametric section to store with the material. */
    readonly section?: Section;
    /** Permissible-stress override. */
    readonly allowableStress?: number;
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

/** Reaction at one supported node. */
export interface SupportReaction {
    readonly node: string;
    readonly fx: number;
    readonly fy: number;
    readonly fz: number;
    /** Reaction moment about Z. */
    readonly mz: number;
}

/** Translational resultant `(fx, fy, fz)`. */
export interface ForceResultant {
    readonly fx: number;
    readonly fy: number;
    readonly fz: number;
}

/**
 * Support reactions and the global force equilibrium check.
 * `unbalanced = reaction + applied` and `balanced` is true when every
 * component is within `tolerance`.
 */
export interface ReactionSummary {
    readonly supports: readonly SupportReaction[];
    readonly reaction: ForceResultant;
    readonly applied: ForceResultant;
    readonly unbalanced: ForceResultant;
    readonly tolerance: number;
    readonly balanced: boolean;
}

/** Options of `reactions()`. */
export interface ReactionOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** Complete linear-static solution. */
export interface Solution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    readonly nodes: readonly NodeDisplacement[];
    readonly reactions: readonly NodeReaction[];
    readonly reactionSummary: ReactionSummary;
    readonly elements: readonly ElementResult[];
    readonly strainEnergy: number;
    readonly maxDisplacement: NodeDisplacement;
    readonly maxAxialForce: MaxAxialForce;
    readonly maxMoment: MaxMoment;
}

/** Solved temperature of one node in a heat-conduction result. */
export interface HeatTemperature {
    readonly id: string;
    readonly temperature: number;
    /** `true` when the temperature was fixed by a Dirichlet boundary. */
    readonly fixed: boolean;
}

/** Surface convection of one member in a heat-conduction result. */
export interface HeatConvection {
    /** Convection coefficient `h` in `W/(m²·K)`. */
    readonly coefficient: number;
    /** Exposed perimeter in the document length unit. */
    readonly perimeter: number;
    /** Ambient temperature in K or °C. */
    readonly ambientTemperature: number;
    /**
     * `h·P·L·(T_avg − Ta)`, positive out of the member.
     */
    readonly heat: number;
}

/** Steady-state axial heat flow of one member. */
export interface HeatElementFlux {
    readonly id: string;
    readonly from: string;
    readonly to: string;
    readonly length: number;
    readonly conductivity: number;
    readonly area: number;
    readonly temperatureFrom: number;
    readonly temperatureTo: number;
    /**
     * `q = k·A·(T_from − T_to)/L`, in watts, positive from `from` to `to`.
     */
    readonly flux: number;
    /** Surface convection; omitted for conduction-only members. */
    readonly convection?: HeatConvection;
}

/** Heat supplied by one fixed-temperature boundary; positive enters the model. */
export interface HeatReaction {
    readonly node: string;
    readonly heat: number;
}

/**
 * Result of a steady-state heat-conduction analysis.
 *
 * This report carries no displacement, strain or force fields. It contains
 * only temperatures and axial heat flows, so it is never a structural result.
 */
export interface HeatSolution {
    readonly format: string;
    readonly version: number;
    /** Always `heat`. */
    readonly analysis: string;
    readonly units: string;
    readonly temperatures: readonly HeatTemperature[];
    readonly elementFlux: readonly HeatElementFlux[];
    readonly reactions: readonly HeatReaction[];
    /** Largest absolute free-degree-of-freedom balance residual. */
    readonly residual: number;
    readonly ok: boolean;
}

/** Options of `heat()`. */
export interface HeatOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** Active state of one tension-only truss in a tension-only solution. */
export interface TensionOnlyElement {
    readonly id: string;
    /** `true` when the cable contributed stiffness in the final active set. */
    readonly active: boolean;
    /** Axial force in the final solution; zero for a slack member. */
    readonly axialForce: number;
}

/**
 * Result of a tension-only (cable) analysis.
 *
 * The active-set iteration deactivates compressed tension-only trusses and
 * re-solves the remaining structure. `iterations` counts static solves;
 * `converged` is `false` when the active set still changed at the iteration
 * limit.
 */
export interface TensionOnlySolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    readonly iterations: number;
    readonly converged: boolean;
    readonly solution: Solution;
    readonly elements: readonly TensionOnlyElement[];
}

/** Options of `tensionOnly()`. */
export interface TensionOnlyOptions {
    /** Maximum number of active-set solves; defaults to 20. */
    readonly iterations?: number;
    /** Relative axial-force threshold below which a cable is slack; defaults to `1e-9`. */
    readonly tolerance?: number;
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
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

/** Section stress recovered at one station. */
export interface StationStress {
    readonly position: number;
    readonly axialForce: number;
    readonly moment: number;
    /** Axial stress `N/A`. */
    readonly axialStress: number;
    /** Signed bending stress `M/S` at the extreme fibre. */
    readonly bendingStress: number;
    /** Smaller extreme-fibre stress `N/A − |M|/S`. */
    readonly minStress: number;
    /** Larger extreme-fibre stress `N/A + |M|/S`. */
    readonly maxStress: number;
}

/** Recovered stresses of one element. */
export interface ElementStress {
    readonly id: string;
    readonly from: string;
    readonly to: string;
    readonly kind: ElementKind;
    readonly length: number;
    readonly area: number;
    /** Elastic section modulus `I/c`; zero for trusses. */
    readonly sectionModulus: number;
    readonly maxAbsStress: number;
    readonly governingStation: number;
    readonly governingPosition: number;
    /** `maxAbsStress / allowableStress` when the material defines one. */
    readonly utilization: number | null;
    readonly stations: readonly StationStress[];
}

/** Governing element and station of a stress report. */
export interface StressGoverning {
    readonly element: string;
    readonly position: number;
    readonly maxAbsStress: number;
    readonly utilization: number | null;
}

/** Result of `stresses()`. */
export interface StressSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    readonly stations: number;
    readonly allowableStress: number | null;
    readonly governing: StressGoverning;
    readonly elements: readonly ElementStress[];
}

/** Options of `stresses()`. */
export interface StressOptions {
    /** Number of stations per element including both ends; defaults to 11. */
    readonly stations?: number;
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** Deflected-shape sample at one station along an element. */
export interface DeflectionStation {
    /** Distance from the `from` node along the member. */
    readonly position: number;
    /** Local axial displacement, positive from `from` to `to`. */
    readonly axial: number;
    /** Local transverse deflection, positive along the local `+y` axis. */
    readonly transverse: number;
    /** Section rotation about the local z axis in radians. */
    readonly rotation: number;
    /** Deformed global X coordinate of the centreline sample. */
    readonly x: number;
    /** Deformed global Y coordinate of the centreline sample. */
    readonly y: number;
    /** Deformed global Z coordinate of the centreline sample. */
    readonly z: number;
}

/** Deflected shape of one element. */
export interface ElementDeflection {
    readonly id: string;
    readonly from: string;
    readonly to: string;
    readonly kind: ElementKind;
    readonly length: number;
    /** Largest absolute transverse deflection among the stations. */
    readonly maxAbsTransverse: number;
    readonly stations: readonly DeflectionStation[];
}

/** Governing transverse deflection of a deflected-shape report. */
export interface DeflectionGoverning {
    readonly element: string;
    readonly position: number;
    /** Signed local transverse deflection at that position. */
    readonly transverse: number;
}

/** Result of `deflections()`. */
export interface DeflectionSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    readonly stations: number;
    readonly governing: DeflectionGoverning;
    readonly elements: readonly ElementDeflection[];
}

/** Options of `deflections()`. */
export interface DeflectionOptions {
    /** Number of stations per element including both ends; defaults to 11. */
    readonly stations?: number;
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/**
 * Averaged stress state at one node. Element-end stresses from every
 * connected member are averaged at the node, while `minStress` and
 * `maxStress` keep the extremes of the connected ends.
 */
export interface NodeStress {
    readonly node: string;
    /** Number of connected element ends that contributed. */
    readonly connectedElements: number;
    /** Arithmetic mean of the axial stresses at the connected ends. */
    readonly meanAxialStress: number;
    /** Smallest extreme-fibre stress among the connected ends. */
    readonly minStress: number;
    /** Largest extreme-fibre stress among the connected ends. */
    readonly maxStress: number;
    /** Largest absolute extreme-fibre stress among the connected ends. */
    readonly maxAbsStress: number;
    /** Element that produced `maxAbsStress`. */
    readonly governingElement: string;
}

/** Governing node of an averaged nodal-stress report. */
export interface NodalStressGoverning {
    readonly node: string;
    readonly element: string;
    readonly maxAbsStress: number;
    readonly utilization: number | null;
}

/** Result of `nodalStresses()`. */
export interface NodalStressSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Number of stations sampled per element before averaging. */
    readonly stations: number;
    readonly allowableStress: number | null;
    readonly governing: NodalStressGoverning;
    readonly nodes: readonly NodeStress[];
}

/** Options of `nodalStresses()`. */
export interface NodalStressOptions {
    /** Number of stations per element including both ends; defaults to 11. */
    readonly stations?: number;
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** One row of a load-case summary. */
export interface LoadCaseResult {
    /** Case name; the unfactored static solve is always called `base`. */
    readonly name: string;
    readonly loads: number;
    readonly gravity: number;
    /** Sum of the applied external loads of this case. */
    readonly applied: ForceResultant;
    /** Largest nodal displacement magnitude. */
    readonly maxDisplacement: number;
    readonly maxDisplacementNode: string;
    /** Signed axial force with the largest magnitude. */
    readonly maxAxialForce: number;
    readonly maxAxialElement: string;
    /** Largest absolute bending moment. */
    readonly maxAbsMoment: number;
    readonly maxMomentElement: string;
    readonly strainEnergy: number;
    /** `true` when `reaction + applied` is within tolerance. */
    readonly balanced: boolean;
}

/** Result of `loadCases()`. */
export interface LoadCaseSummary {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Unfactored case followed by every stored load combination. */
    readonly cases: readonly LoadCaseResult[];
}

/** Options of `loadCases()`. */
export interface LoadCaseOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** One active degree of freedom of an assembled system. */
export interface MatrixDof {
    readonly node: string;
    /** Component name: `ux`, `uy`, `uz` or `rz`. */
    readonly component: 'ux' | 'uy' | 'uz' | 'rz';
    /** `true` when a support constrains this degree of freedom. */
    readonly constrained: boolean;
}

/** Assembled global stiffness and mass matrices in active degree-of-freedom order. */
export interface SystemMatrices {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Number of active degrees of freedom. */
    readonly size: number;
    /** Degree-of-freedom labels in matrix row/column order. */
    readonly dofs: readonly MatrixDof[];
    /** Dense global stiffness matrix in `dofs` order. */
    readonly stiffness: readonly (readonly number[])[];
    /** Dense global mass matrix; the lumped mass model is diagonal. */
    readonly mass: readonly (readonly number[])[];
}

/** Options of `matrices()`. */
export interface MatrixOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** One point of a response spectrum. */
export interface SpectrumPoint {
    /** Natural period in seconds; must be finite and non-negative. */
    readonly period: number;
    /** Spectral acceleration in document length per second squared. */
    readonly acceleration: number;
}

/** Modal combination rule of a response-spectrum analysis. */
export type SpectrumCombination = 'srss' | 'cqc';

/** One mode of a response-spectrum analysis. */
export interface SpectrumMode {
    readonly frequencyHz: number;
    readonly period: number;
    /** Spectral acceleration read from the input spectrum at `period`. */
    readonly spectralAcceleration: number;
    /** Signed participation factor in the analysed direction. */
    readonly participationFactor: number;
    /** Spectral displacement `Sa / ω²`. */
    readonly spectralDisplacement: number;
    /** Modal displacement amplitude `Γ · Sd`. */
    readonly scale: number;
}

/** Result of `responseSpectrum()`. */
export interface ResponseSpectrumSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Analysed global direction (`x`, `y` or `z`). */
    readonly axis: string;
    /** Modal combination that was applied. */
    readonly combination: SpectrumCombination;
    /** Modal damping ratio used by CQC; absent for SRSS. */
    readonly damping?: number;
    /** Requested number of modes. */
    readonly requested: number;
    /** Modes used in the combination, in ascending frequency order. */
    readonly modes: readonly SpectrumMode[];
    /** Combined nodal displacements after the modal combination. */
    readonly nodes: readonly NodeDisplacement[];
    /** Nodal displacement with the largest combined magnitude. */
    readonly maxDisplacement: NodeDisplacement;
}

/** Options of `responseSpectrum()`. */
export interface ResponseSpectrumOptions {
    /** Number of modes; defaults to 5. */
    readonly modes?: number;
    /** Global direction of the excitation; defaults to `x`. */
    readonly axis?: 'x' | 'y' | 'z';
    /** Modal combination rule; defaults to `srss`. */
    readonly combination?: SpectrumCombination;
    /**
     * One modal damping ratio in `(0, 1]`; required by `cqc` and ignored by
     * `srss`.
     */
    readonly damping?: number;
    /** Spectral acceleration points; values between points are interpolated linearly. */
    readonly spectrum: readonly SpectrumPoint[];
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** Steady-state response at one sampled excitation frequency. */
export interface FrequencyResponsePoint {
    /** Excitation frequency in hertz. */
    readonly frequencyHz: number;
    /** Largest translational displacement amplitude over all nodes. */
    readonly maxAmplitude: number;
    /** Node that carries `maxAmplitude`. */
    readonly governingNode: string;
}

/** Peak steady-state response of one node over the sampled range. */
export interface NodeFrequencyResponse {
    readonly node: string;
    /** Sampled frequency that produced `maxAmplitude`. */
    readonly peakFrequencyHz: number;
    readonly maxAmplitude: number;
}

/** Result of `frequencyResponse()`. */
export interface FrequencyResponseSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Analysed global direction (`x`, `y` or `z`). */
    readonly axis: string;
    /** Requested number of modes. */
    readonly requested: number;
    /** Modal damping ratio applied to every mode. */
    readonly dampingRatio: number;
    readonly startHz: number;
    readonly endHz: number;
    /** Number of equally spaced sampled frequencies, including both ends. */
    readonly points: number;
    /** Steady-state amplitude at every sampled frequency. */
    readonly sweep: readonly FrequencyResponsePoint[];
    /** Sampled frequency with the largest amplitude. */
    readonly peak: FrequencyResponsePoint;
    /** Peak amplitude of every node over the sampled range. */
    readonly nodes: readonly NodeFrequencyResponse[];
}

/** Options of `frequencyResponse()`. */
export interface FrequencyResponseOptions {
    /** Number of modes; defaults to 5. */
    readonly modes?: number;
    /** Global direction of the harmonic excitation; defaults to `x`. */
    readonly axis?: 'x' | 'y' | 'z';
    /** Modal damping ratio applied to every mode; must be positive. */
    readonly damping: number;
    /** First sampled frequency in hertz. */
    readonly startHz: number;
    /** Last sampled frequency in hertz; must be greater than `startHz`. */
    readonly endHz: number;
    /** Number of equally spaced samples; defaults to 101. */
    readonly points?: number;
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** One construction stage: the active element subset and load factors. */
export interface ConstructionStage {
    /** Unique, non-empty stage name. */
    readonly name: string;
    /** Active element identifiers; omitted or `undefined` keeps every element. */
    readonly elements?: readonly string[];
    /** Factor applied to nodal and element applied loads; defaults to 1. */
    readonly loads?: number;
    /** Factor applied to the gravity vector; defaults to 0. */
    readonly gravity?: number;
}

/** Summary of one solved construction stage. */
export interface ConstructionStageResult {
    readonly name: string;
    /** Number of active elements in the stage. */
    readonly activeElements: number;
    readonly loads: number;
    readonly gravity: number;
    /** Sum of the applied external loads of the stage. */
    readonly applied: ForceResultant;
    readonly maxDisplacement: number;
    readonly maxDisplacementNode: string;
    readonly maxAxialForce: number;
    readonly maxAxialElement: string;
    readonly maxAbsMoment: number;
    readonly maxMomentElement: string;
    readonly strainEnergy: number;
    /** `true` when `reaction + applied` is within tolerance. */
    readonly balanced: boolean;
}

/** Result of `constructionStages()`. */
export interface ConstructionStageSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    readonly stages: readonly ConstructionStageResult[];
}

/** Options of `constructionStages()`. */
export interface ConstructionStageOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
}

/** Result of `geometricNonlinear()`. */
export interface NonlinearSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Number of equal load increments used by the path-following loop. */
    readonly increments: number;
    /** Total Newton iterations over all increments. */
    readonly iterations: number;
    /** Requested relative convergence tolerance. */
    readonly tolerance: number;
    /** Free residual norm of the final accepted state. */
    readonly residualNorm: number;
    readonly nodes: readonly NodeDisplacement[];
    /** Nodal displacement with the largest magnitude. */
    readonly maxDisplacement: NodeDisplacement;
}

/** Options of `geometricNonlinear()`. */
export interface NonlinearOptions {
    /** Number of equal load increments; defaults to 10. */
    readonly increments?: number;
    /** Newton iterations per increment; defaults to 30. */
    readonly iterations?: number;
    /** Relative residual target; defaults to `1e-10`. */
    readonly tolerance?: number;
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
    /**
     * Signed modal participation factor `φᵀ·M·r` in the global X, Y and Z
     * directions. Its square is `effectiveMass`.
     */
    readonly participationFactor: readonly [number, number, number];
    /**
     * Effective modal mass `(φᵀ·M·r)²` in the global X, Y and Z directions.
     * Divide by `totalMass` for the effective mass fraction.
     */
    readonly effectiveMass: readonly [number, number, number];
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
    /** Total translational mass in the global X, Y and Z directions. */
    readonly totalMass: readonly [number, number, number];
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

/** One solved buckling load combination. */
export interface BucklingCase {
    readonly name: string;
    readonly loads: number;
    readonly gravity: number;
    /** Buckling solution of the scaled model. */
    readonly solution: BucklingSolution;
}

/** Governing combination of a buckling envelope. */
export interface BucklingEnvelope {
    readonly combination: string;
    readonly factor: number;
}

/** Result of `solveBucklingCombinations()`. */
export interface BucklingCombinationSolution {
    readonly format: string;
    readonly version: number;
    readonly units: string;
    /** Requested number of modes per combination. */
    readonly requested: number;
    readonly combinations: readonly BucklingCase[];
    /** Lowest first buckling factor, or `null` when no factor is positive. */
    readonly envelope: BucklingEnvelope | null;
}

/** Options of `solveBucklingCombinations()`. */
export interface BucklingCombinationOptions {
    /** Number of modes per combination; defaults to 5. */
    readonly modes?: number;
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
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
    readonly edits: Readonly<Record<string, SupportLevel>>;
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

/** Request of `updateNode()`. */
export interface NodeUpdateRequest {
    readonly id: string;
    /** New X coordinate; omitted values keep their current coordinate. */
    readonly x?: number;
    /** New Y coordinate. */
    readonly y?: number;
    /** New Z coordinate; only valid in three dimensions. */
    readonly z?: number;
}

/**
 * Request of `setNodeMass()`.
 *
 * The concentrated mass is added to every translational degree of freedom of
 * the node by modal analysis and uses the mass unit consistent with `density`:
 * tonnes (`t`) in the millimetre system and kilograms (`kg`) in the metre
 * system. A value of `0` clears it.
 */
export interface NodeMassRequest {
    readonly id: string;
    /** Concentrated nodal mass; finite and non-negative. */
    readonly mass: number;
}

/** Request of `setNodeTemperature()`; `null` clears the fixed temperature. */
export interface NodeTemperatureRequest {
    readonly id: string;
    /** Fixed temperature in K or °C, or `null` to remove the boundary. */
    readonly temperature: number | null;
}

/** Request of `setNodeHeat()`; `0` clears the heat load. */
export interface NodeHeatRequest {
    readonly id: string;
    /** Concentrated heat flow in watts, positive into the node. */
    readonly heat: number;
}

/** Request of `mergeNodes()`; the `into` node survives with its coordinates. */
export interface NodeMergeRequest {
    /** Node that is removed after its references move to `into`. */
    readonly from: string;
    /** Node that keeps its id and coordinates. */
    readonly into: string;
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
    /**
     * Second moment of area. Required for beams unless the material section
     * provides one; when present it overrides the section value.
     */
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
    /**
     * Start of the loaded segment as a fraction of the element length. Supply
     * it together with `distributedLoadTo`; the load then acts only between the
     * two fractions, with `distributedLoad`/`distributedLoadEnd` as the
     * intensities at the segment ends.
     */
    readonly distributedLoadFrom?: number;
    /** End of the loaded segment as a fraction of the element length. */
    readonly distributedLoadTo?: number;
    /**
     * Uniform temperature change of the member in kelvin (or °C difference).
     * Requires the material `alpha`; a uniform change produces axial strain
     * only and never bends the member.
     */
    readonly temperature?: number;
    /**
     * Linear temperature difference between the local `+y` and `-y` extreme
     * fibres in kelvin (or °C difference). A positive value heats the `+y`
     * face. Requires the material `alpha` and a section that provides the
     * depth; the gradient produces equivalent end moments.
     */
    readonly temperatureGradient?: number;
    /**
     * Surface convection coefficient `h` in `W/(m²·K)` for steady-state heat
     * conduction. Requires `perimeter` and a model ambient temperature.
     */
    readonly convection?: number;
    /**
     * Exposed perimeter of the member surface in the document length unit;
     * required when `convection` is set.
     */
    readonly perimeter?: number;
    /**
     * Marks a truss member as tension-only (a cable). The tension-only analysis
     * removes members that carry compression. Only valid for `truss` elements.
     */
    readonly tensionOnly?: boolean;
}

/** Bending-moment release (hinge) at the ends of a two-dimensional beam. */
export interface EndRelease {
    /** Release the bending moment at the `from` node. */
    readonly start: boolean;
    /** Release the bending moment at the `to` node. */
    readonly end: boolean;
}

/** Request of `setElementRelease()`. Both flags false clears the release. */
export interface ElementReleaseRequest {
    readonly id: string;
    readonly start?: boolean;
    readonly end?: boolean;
}

/** Request of `setElementTensionOnly()`. */
export interface ElementTensionOnlyRequest {
    readonly id: string;
    readonly tensionOnly: boolean;
}

/**
 * Request of `updateElement()`. Only the supplied endpoints change; material,
 * loads, temperature and releases are preserved.
 */
export interface ElementUpdateRequest {
    readonly id: string;
    /** New start node identifier. */
    readonly from?: string;
    /** New end node identifier. */
    readonly to?: string;
    /** Surface convection coefficient `h` in `W/(m²·K)`. */
    readonly convection?: number;
    /** Exposed perimeter in the document length unit. */
    readonly perimeter?: number;
}

/** Request of `splitElement()`. */
export interface ElementSplitRequest {
    readonly id: string;
    /** Fraction of the element length, strictly between `0` and `1`. */
    readonly at: number;
}

/**
 * Result of `splitElement()`: the document summary plus the interior node and
 * both element identifiers.
 */
export interface ElementSplitReport extends DocumentSummary {
    /** Interior node at the split point (created or reused). */
    readonly node: string;
    /** First segment from the original start node; it keeps the element id. */
    readonly first: string;
    /** Second segment towards the original end node. */
    readonly second: string;
    /** `true` when an existing node at the split point was reused. */
    readonly reusedNode: boolean;
}

/** Request of `setMaterial()`. */
export interface MaterialSetRequest {
    readonly youngsModulus: number;
    readonly area: number;
    /** Mass density, required by modal analysis and self-weight. */
    readonly density?: number;
    readonly name?: string;
    /** Permissible stress for utilisation checks. */
    readonly allowableStress?: number;
    /** Coefficient of thermal expansion in `1/K`. */
    readonly alpha?: number;
    /** Thermal conductivity `k` in `W/(m·K)`, used only by `heat()`. */
    readonly conductivity?: number;
    /** Parametric section geometry. */
    readonly section?: Section;
}

/**
 * Request of `addSupport()`. At least one constraint flag, prescribed value or
 * spring stiffness must be present.
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
    /** Linear elastic translational spring stiffness along X (force/length). */
    readonly kx?: number;
    /** Linear elastic translational spring stiffness along Y (force/length). */
    readonly ky?: number;
    /** Linear elastic translational spring stiffness along Z (3D only). */
    readonly kz?: number;
    /** Linear elastic rotational spring stiffness about Z (2D beam nodes only). */
    readonly kr?: number;
    /**
     * Inclined roller constraint direction `[nx, ny]` in the X–Y plane (2D
     * documents only). It constrains displacement along the normalised vector
     * and cannot be combined with `dx`, `dy`, a prescribed displacement or a
     * spring on the same support.
     */
    readonly direction?: readonly [number, number];
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

/** Request of `setAmbientTemperature()`; `null` clears the ambient temperature. */
export interface AmbientTemperatureRequest {
    /** Ambient temperature in K or °C, or `null` to remove it. */
    readonly temperature: number | null;
}

/** Options of `solve()`. */
export interface SolveOptions {
    /** When given, the JSON report is written atomically to this path. */
    readonly out?: string;
    /** `static` (default), `modal`, `buckling` or `tensionOnly`. */
    readonly analysis?: Analysis;
    /** Number of modes for modal or buckling analysis; defaults to 5. */
    readonly modes?: number;
    /** Maximum active-set solves for `tensionOnly`; defaults to 20. */
    readonly iterations?: number;
    /** Relative slack threshold for `tensionOnly`; defaults to `1e-9`. */
    readonly tolerance?: number;
}
