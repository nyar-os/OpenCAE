# Capabilities

This document is the support claim for OpenCAE. Anything not marked
supported here is not a support claim, even if an API name suggests it.

The machine-readable version is printed by `open-cae capabilities` (and by
`OpenCAE.capabilities()`); it is generated from the same
`CAPABILITIES_JSON` constant the solver uses.

## Capability matrix

| Capability | Status |
|------------|--------|
| 2D truss linear static (`truss2d`) | ✅ supported |
| 3D truss linear static (`truss3d`) | ✅ supported |
| 2D Euler-Bernoulli beam (`beam2d`, including axial force/shear/bending moment, distributed loads, end moments, end releases) | ✅ supported |
| partial-span distributed loads (`distributedLoadFrom`/`distributedLoadTo`, uniform or linear over part of a member) | ✅ supported |
| parametric element splitting (`element split`, `splitElement()`; interior node + two segments) | ✅ supported |
| beam end moment releases (`endRelease`, static condensation of hinges) | ✅ supported |
| self-weight (`selfWeight`, lumped truss distribution + consistent beam loads) | ✅ supported |
| uniform member temperature change (`thermalExpansion`, `α·ΔT` thermal strain + equivalent nodal loads) | ✅ supported |
| through-depth temperature gradient (`thermalGradient`, linear ΔT across the section + equivalent end moments) | ✅ supported |
| steady-state heat conduction (`heat`, truss axial `kA/L` conduction, fixed temperatures + nodal heat flows + optional surface convection) | ✅ supported |
| prescribed displacements (`prescribedDisplacement`, support settlements) | ✅ supported |
| modal analysis (`modal`, lumped mass + Jacobi, signed participation factors and effective modal mass) | ✅ supported |
| concentrated nodal masses (`node mass`, translational lumped mass on top of element mass) | ✅ supported |
| linear buckling (`buckling`, 2D truss/beam and 3D truss geometric stiffness + Jacobi) | ✅ supported |
| buckling under load combinations (`bucklingCombinations`, scaled cases + governing envelope) | ✅ supported |
| internal force diagrams (`internalForces`, sampled N/V/M per element) | ✅ supported |
| beam stress recovery and utilisation (`stresses`) | ✅ supported |
| deflected-shape sampling (`deflections`, local deflection/rotation and deformed coordinates) | ✅ supported |
| influence lines (`influenceLine`, reaction/section ordinates as a unit load moves along a track beam; unloaded model required) | ✅ supported |
| moving load train envelopes (`influenceLineTrain`, axle loads at fixed spacings over the influence line polyline; unloaded model required) | ✅ supported |
| averaged nodal stress and extremes (`nodalStresses`) | ✅ supported |
| load-case summary table (`loadCases`, `base` plus every combination) | ✅ supported |
| load-combination envelope (`combinationEnvelope`, `base` plus every combination; per-component minimum/maximum with the governing combination) | ✅ supported |
| model quantity and DOF summary (`modelSummary`, member length/volume/mass/centroid plus translational, rotational, constrained and free DOF counts) | ✅ supported |
| support reaction summary and equilibrium check (`reactions`) | ✅ supported |
| linear elastic spring supports (`elasticSupports`, translational and rotational stiffness) | ✅ supported |
| inclined/rotated roller supports (`inclinedSupports`, arbitrary constraint direction in 2D) | ✅ supported |
| assembled stiffness and mass matrix export (`matrixExport`) | ✅ supported |
| response-spectrum analysis (`responseSpectrum`, SDOF spectral acceleration input + SRSS or CQC) | ✅ supported |
| harmonic frequency response (`frequencyResponse`, modal superposition sweep, peak frequency/amplitude) | ✅ supported |
| construction stages / element birth–death summaries (`constructionStages`, active element subsets + stage factors) | ✅ supported |
| geometric-nonlinear truss analysis (`geometricNonlinear`, corotational Newton iteration with load increments) | ✅ supported |
| tension-only cable trusses (`tensionOnly`, active-set iteration with slack members removed) | ✅ supported |
| load combinations (`loadCombinations`, load, gravity and temperature factors + envelope; named load pattern factors mixed in) | ✅ supported |
| named load patterns (`loadPatterns`, concentrated nodal load layouts referenced by combinations) | ✅ supported |
| material presets and section library (`materialLibrary`, rectangle/circle/I-section/box/tube/channel properties) | ✅ supported |
| custom polygon section properties (`polygonSection`, closed vertex list; area, centroid, Ix/Iy/Ixy, polar moment and radii of gyration) | ✅ supported |
| parametric standard sections (`standardSection`, I / rectangular tube / channel / L angle / T / solid ellipse / equilateral triangle / equal-thickness Z; area, centroid, Ix/Iy/Ixy, torsion constant and radii of gyration) | ✅ supported |
| principal section axes (`principalSection`, Ix/Iy/Ixy; principal moments, polar moment and major-axis angle) | ✅ supported |
| `.ocae` JSON I/O | ✅ supported |
| CSV (solution tables and the `envelope --csv` table) / legacy VTK / Markdown / HTML report export and deterministic SVG shape export | ✅ supported |
| SVG result contours (`export --contour axial\|shear\|moment\|displacement`, sampled min/max colour bar) | ✅ supported |
| 3D beam (`beam3d`) | ❌ not implemented |
| 3D frame (`frame3d`) | ❌ not implemented |
| material nonlinearity, beam/frame large displacement (`nonlinear`) | ❌ not implemented |
| Abaqus `.inp` import (documented subset; see [Abaqus `.inp` import](#abaqus-inp-import)) | ✅ supported |
| STEP export | ❌ not implemented |
| contact, prestress, dynamic time history | ❌ not implemented |

`beam3d`/`frame3d`/`nonlinear` are rejected at every entry point, while
`buckling`, `prescribedDisplacement`, `endRelease`, `thermalExpansion`,
`thermalGradient`, `heat`, `partialSpanLoad`, `elasticSupports`,
`inclinedSupports`, `matrixExport`, `responseSpectrum`, `frequencyResponse`,
`constructionStages`, `geometricNonlinear`, `tensionOnly`, `deflections`,
`nodalStress`, `loadCases`, `influenceLine`, `influenceLineTrain`,
`loadPatterns`, `loadCombinations`, `combinationEnvelope` and `modelSummary`
are available through the
Rust API, the
native addon and the TypeScript SDK. SVG contours are exposed as
`export_svg_with_contour()` with `ContourQuantity` in Rust, the `contour`
field of the addon export options and `export({ contour })` in the SDK and the
CLI (`export --contour ...`). Load patterns are `add_pattern_load()`
nodal loads grouped by name in Rust, `pattern_add`/`pattern_remove` in the
addon and `addPatternLoad()`/`removePattern()` in the SDK. A combination mixes
them through `add_combination_with_patterns()`/`add_combination_full()` in
Rust, `combination_add` with `thermal` and `patterns` fields in the addon and
`document.addCombination({ thermal, patterns })` in the SDK and the CLI
(`combination add --thermal ... --patterns ...`). Custom polygon section properties
(`polygonSection`) are exposed as `polygon_section_report()` in Rust,
`section_polygon` in the addon and `sectionPolygon()` in the SDK/CLI.
Parametric standard sections (`standardSection`) are exposed as
`i_standard_section_report()`, `tube_standard_section_report()`,
`channel_standard_section_report()`, `angle_section_report()`,
`t_section_report()`, `ellipse_section_report()`, `triangle_section_report()`
and `z_section_report()` in Rust, `section_i`, `section_tube`,
`section_channel`, `section_angle`, `section_t`, `section_ellipse`,
`section_triangle` and `section_z` in the addon and `sectionI()`,
`sectionTube()`, `sectionChannel()`, `sectionAngle()`, `sectionT()`,
`sectionEllipse()`, `sectionTriangle()` and `sectionZ()` in the SDK, with CLI
subcommands `section i`, `section tube`, `section channel`, `section angle`,
`section t`, `section ellipse`, `section triangle` and `section z`.
Principal section axes (`principalSection`) are exposed as
`principal_section_report()` in Rust, `section_principal` in the addon and
`sectionPrincipal()` in the SDK, with CLI subcommand `section principal`.
Parametric element splitting is the `edits`
entry `elementSplit` of the capability matrix and is available at the same
three entry points plus the CLI (`open-cae element split`). In-place element
updates are the `edits` entry `elementUpdate`: `element update` can change the
endpoints, distributed load (uniform, tapered or partial-span), uniform
temperature change, gradient and convection data independently, leaving every
unnamed value untouched, with `--clear-load` and `--clear-temperature` to
remove the corresponding actions.

`thermalExpansion` and `thermalGradient` cover member temperature actions and
feed the structural solver only. `heat` is a separate, uncoupled steady-state
conduction analysis: it reports temperatures and heat flows, never uses
displacements, and supports **conduction along truss/beam axes only** with an
optional **constant surface convection** to a model ambient temperature
(`heatConvection` in the capability matrix); radiation and time stepping remain
unsupported. `geometricNonlinear` is limited to
truss models; the generic `nonlinear` entry (material nonlinearity,
beam/frame large displacement) remains unimplemented. `tensionOnly` is
likewise limited to truss members: it deactivates compressed cables, records
the active set and iterations, and reports a mechanism when the slack topology
cannot carry the load.

HTML report and SVG shape export exist in the Rust API, the native addon and
the TypeScript SDK; the `open-cae` CLI writes `csv`, `vtk`, `md` and `svg`
(see [cli.md](cli.md)). The SVG drawing projects every node onto the global
X–Y plane (Z is ignored, including in three-dimensional documents), fits a
fixed 960 × 720 viewBox to the projected undeformed and deformed geometry,
and draws grey undeformed lines, a blue deformed polyline at the requested
`scale`, node dots, support triangles and load arrows. Like the reports it
contains no timestamps, so repeated exports are byte-identical; the scale must
be finite and positive and a model without nodes is rejected with
`export/no-solution`.

`export --contour <quantity>` (SDK `export({ contour })`, addon `contour`,
Rust `export_svg_with_contour`) replaces the blue deformed polyline with
coloured member segments: `axial`, `shear` and `moment` come from the same
`internalForces` sampling that `--stations` (default 11, at least 2) controls,
and `displacement` uses the nodal displacement magnitudes. The colours map a
blue–white–red diverging scale over the sampled extremes; the colour bar names
the quantity, its unit (`N`, `N·mm`/`N·m`, `mm`/`m`) and the sampled minimum and
maximum, so the drawing and the numbers agree. A contour is SVG-only; every
other format rejects it with `export/invalid-contour`, and an unknown quantity
is rejected with the same code (the CLI reports `cli/invalid-option`).

Model quantities are exposed as `summarize_model()` in Rust, `model_summary` in
the addon and `modelSummary()` in the SDK, with CLI subcommand `model summary`.
The report needs no solve: it sums member length, volume `A·L`, member mass
`density·A·L` (using each element area override), nodal mass and their centroid,
and counts the solver degrees of freedom (two translations per node, three in
three dimensions, plus one rotation at every unreleased 2D beam node; rigid and
inclined supports restrain one each, springs none). Units are never converted,
so a `mm` document reports `mm`, `mm³` and tonnes. A CSV export now also writes
`forces.csv` (one row per sampled station with `position`, `axialForce`,
`shear` and `moment`); `export --stations` controls that sampling and defaults
to 11.

## Numerical methods

1. **Direct stiffness method**: truss elements are assembled from the 3D
   length `L`, direction cosines `c` and axial stiffness `k = EA/L`. 2D beam
   elements use the 6×6 Euler-Bernoulli stiffness in local
   `[u1x, u1y, θ1, u2x, u2y, θ2]` coordinates (`EA/L`, `12EI/L³`, `6EI/L²`,
   `4EI/L`, `2EI/L`) and are assembled as `TᵀKT` with the rotation
   transformation `c = Δx/L, s = Δy/L`.
2. **Degree-of-freedom activation**: a 2D document containing beams uses
   `ux, uy, rz` globally, but `rz` is active only at continuous beam ends (a
   released end contributes no rotational degree of freedom); 2D documents
   without beams use only `ux, uy`; 3D trusses use `ux, uy, uz`. This prevents
   zero-stiffness rotation rows in purely truss parts and at hinges.
3. **Constraint handling**: only active, unconstrained degrees of freedom are
   assembled and the reduced system is solved (row/column elimination first),
   avoiding the ill-conditioning of the large-number penalty method. When a
   support prescribes a non-zero displacement `u_c`, the free right-hand side
   is corrected by `−K_fc·u_c`; the constrained values are then written back
   into the displacement vector, so reactions and element forces follow from
   the same `K·u − F` residual as homogeneous supports.
4. **Linear equation solution**: a self-written Gaussian elimination with
   partial pivoting (no linear algebra crate). A pivot smaller than `1e-12 ×
   the largest matrix magnitude` is treated as singular and returns
   `model/mechanism`.
5. **Distributed loads and self-weight**:
   - The beam distributed load (local `+y`) becomes the consistent nodal
     vector and is then applied through the coordinate transformation. A
     uniform `w` gives `[0, wL/2, wL²/12, 0, wL/2, −wL²/12]`; a linear load
     `q(x) = q1 + (q2 − q1)·x/L` gives
     `[0, L(7q1+3q2)/20, L²(3q1+2q2)/60, 0, L(3q1+7q2)/20, −L²(2q1+3q2)/60]`.
     Both are the exact integrals for Euler-Bernoulli shape functions.
   - Truss self-weight splits `ρALg` equally between the two end nodes
     (lumped-mass style: exact total force balance, but without the
     higher-order terms of a consistent mass matrix).
   - Beam self-weight decomposes gravity into local axial and transverse
     components: the axial component is split equally as `ρAL(g·t)/2` (exact
     for linear shape functions), and the transverse component uses the
     consistent load above (exact for beam shape functions).
6. **Results**:
   - displacement `u = (ux, uy, uz, rz)` (`rz = 0` for pure trusses and 3D
     models);
   - reaction `R = K·u − F`, including `fx, fy, fz` and `mz`;
   - each element reports local end forces
     `endForces = {n1,v1,m1,n2,v2,m2} = K_local·d_local − f_eq` and
     `maxAbsMoment`; the axial force `axialForce` is reported separately with
     tension positive;
   - `solve_internal_forces(model, stations)` (CLI `open-cae forces`) recovers
     section forces by equilibrium from the end forces and the applied
     linear/gravity loads: `N(x) = −n1 − p·x`, `V(x) = v1 + ∫q`,
     `M(x) = −m1 + v1·x + ∫q(x−s)ds`, sampled at equally spaced stations; the
     end stations reproduce the end forces exactly;
   - total strain energy `Σ ½·dᵀKd` is reported as `strainEnergy`.
7. **Modal analysis**: uses the same `K` as the original problem and a
   diagonal lumped mass matrix — `ρAL/2` for translational degrees of freedom
   and `ρIL/2` (rotary inertia) for beam rotational degrees of freedom. It
   forms the symmetric matrix `A = M^(−1/2)·K·M^(−1/2)`, computes eigenvalues
   with symmetric Jacobi iterations (off-diagonal tolerance `1e-12·max|A|`, at
   most 100 sweeps) and frequencies `f = √λ/(2π)`. Zero, negative or
   non-finite eigenvalues are skipped and counted in `skipped`; NaN is never
   emitted. Each mode reports the effective modal mass `(φᵀ·M·r)²` in X/Y/Z,
   and `totalMass` reports the total translational mass so the effective mass
   fraction can be obtained by division; the fixed-free axial bar first mode
   approaches the analytical `8/π²` fraction. A concentrated nodal `mass` adds
   to every translational degree of freedom before the same solve.
8. **Linear buckling**: first solves the same static load case and reads the
   signed axial force `N` of every member. The geometric stiffness is
   assembled from that reference state: beams use the consistent 6×6 `N/L`
   matrix (`6/5`, `L/10`, `2L²/15`, …) transformed to global coordinates, and
   trusses use the transverse projector `(N/L)·(I − n·nᵀ)` at both ends. The
   eigenproblem `(K + λ·Kg)·φ = 0` is reduced to a standard symmetric form with
   a Cholesky factor of `K_ff`, solved by the same Jacobi iteration and
   reported as ascending positive load factors `λ`; non-positive or non-finite
   factors are skipped. Modes are normalized so the largest absolute nodal
   value is one. Two-dimensional beams and two- or three-dimensional trusses
   are assembled. Buckling under load combinations scales each combination's
   loads (and gravity) exactly like the static combination solver and runs
   this analysis per case; the reported factors therefore always multiply that
   combination's load set, and the envelope names the combination with the
   lowest first factor.
9. **Mechanism detection**: every solve performs a numerical rank check on the
   free-degree-of-freedom stiffness matrix (whether `K_ff` has full rank); on
   failure it reports `model/mechanism` and lists as many unconstrained nodes
   as possible.
10. **Load combinations**: each combination clones the model, multiplies every
    nodal load and element distributed load by `loads`, multiplies the gravity
    vector by `gravity` (clearing it when the result is the zero vector),
    multiplies every element temperature change and through-depth gradient by
    `thermal` (default `1`), then adds every referenced named pattern's nodal
    loads multiplied by its factor. The scaled model is solved and the full
    static solution kept. The envelope
    reports the governing nodal displacement, signed axial force/moment maxima
    and per-node/per-element extremes with the combination names that produced
    them. Prescribed displacements and element temperature changes are not
    scaled.
11. **Material and section library**: presets store Young's modulus and
    density in `mm` units and convert by `1e6` (Pa) and `1e12` (kg/m³).
    Sections compute `A`, `I_strong = b h³/12` (rectangle), `I_weak`, the
    extreme-fibre distance `c` and a Saint-Venant torsion constant
    (`J ≈ a b³/3·(1 − 0.63 b/a + 0.052 (b/a)⁵)` for rectangles, `π d⁴/32` for
    circles and a thin-walled sum for I-sections). The elastic section modulus
    is `I_strong / c`; a beam may omit `inertia` and inherit `I_strong` from
    the section.
12. **Beam stress recovery**: at every internal-force station the report
    computes `σ_min/σ_max = N/A ± |M|/S` with `S = I/c`, where `I` is the
    element inertia (or the material-section inertia) and `c` is the
    material-section extreme-fibre distance. Each element reports the largest
    absolute stress, the governing station and, when `allowableStress` is
    declared, `utilisation = |σ|max / allowableStress`; the report also names
    the governing element. Trusses report `N/A` only. This is a sampled check,
    not a fibre-level integration.
13. **Support reaction summary**: after solving, the reaction vector
    `R = K·u − F` is grouped per support (`fx, fy, fz, mz`) and the report
    compares `reaction + applied` with a tolerance that scales with the
    largest resultant. Rigid supports report the residual; spring supports
    report `−k·u`. `applied` includes nodal forces, consistent distributed
    loads and self-weight; `balanced` is the global force-equilibrium check.
    The same summary is included in every static solution and written to
    `reactions.csv` by the CSV exporter.
14. **Elastic supports**: a spring adds its stiffness to the diagonal of the
    assembled matrix and leaves the matching degree of freedom free, so a
    model can be stabilized by springs instead of rigid constraints. Spring
    and rigid constraint never share a degree of freedom (validation rejects
    it), which keeps the reaction split unambiguous. The axial bar
    `P/(EA/L + k)`, the cantilever tip spring `P/(3EI/L³ + k)` and a root
    rotational spring `P L³/(3EI) + P L²/k` are asserted to relative error
    `1e-12`.
15. **Thermal expansion**: a uniform member `temperature` change `ΔT` with the
    material `alpha` adds the equivalent axial load `E·A·α·ΔT·[−1, +1]` in
    local coordinates (axial only). The reported axial force is
    `k·δ − E·A·α·ΔT`, so a free member expands by `α·ΔT·L` without stress and
    a fully restrained member carries `−E·A·α·ΔT`; a free thermal expansion
    stays moment-free. A temperature change without a finite `alpha` is
    rejected with `material/missing-alpha`. A separate `temperatureGradient` is
    the linear difference between the local `+y` and `−y` extreme fibres: it
    produces the free curvature `−α·ΔT/h` and the equivalent end moments
    `±E·I·α·ΔT/h`, so a simply supported beam bends without end moments and a
    fixed-fixed beam carries the constant restraining moment. A gradient
    requires a finite `alpha` and a material section that supplies the depth,
    otherwise it is rejected with `material/missing-alpha` or
    `material/missing-section`. Neither thermal load is scaled by load
    combinations. CLI flags: `element add --dt` for a uniform change and
    `element add --dt-grad` for the gradient.
16. **Beam end releases**: released bending rotations are statically condensed
    before assembly (`K_cond = K − K_er·K_rr⁻¹·K_re`,
    `f_cond = f − K_er·K_rr⁻¹·f_r`) so the assembled element carries no moment
    at the hinge. The internal released rotation is recovered from
    `K_rr·θ_r = f_r − K_re·d_e` before end forces are reported, which keeps
    `m = 0` at the released end. Releasing both ends of a member gives a
    moment-free (simply supported) member; a node whose beam ends are all
    released has no rotational degree of freedom.
17. **Deflected shape**: `solve_deflection(model, stations)` (CLI
    `open-cae deflections`) transforms the nodal displacements into the member
    frame, restores released hinge rotations, and evaluates the cubic Hermite
    interpolation of `(v1, θ1, v2, θ2)` plus the exact fixed-end particular
    solution of the linearly varying transverse load (`EI·v'''' = q(x)`, a
    quartic/quintic polynomial). The axial displacement adds the quadratic term
    of a uniform axial load. Each station reports `axial`, `transverse`,
    `rotation` and the deformed global `x/y/z`, so simply supported, cantilever
    and triangular-load cases reproduce the closed-form deflections exactly.
18. **Nodal stress summary**: `solve_nodal_stress(model, stations)` (CLI
    `open-cae nodal-stresses`) reuses the per-element stress recovery, routes
    the first and last station to the `from` and `to` nodes, and reports the
    arithmetic mean of the connected axial stresses, the minimum/maximum
    extreme-fibre stresses of the connected ends, the largest absolute value
    with its governing element, and the utilisation against `allowableStress`.
    It is a screening view of the sampled stress field, not a fibre-level
    integration.
19. **Load-case summary**: `summarize_load_cases(model)` (CLI
    `open-cae load-cases`) always solves the unfactored `base` case and then
    each stored combination on the same `loads`/`gravity` scaled model used by
    combination analysis. Each row reports the applied resultant, the largest
    displacement and its node, the extreme axial force and bending moment with
    their elements, the strain energy and the equilibrium flag, giving a
    compact table for reports without superseding the per-element diagrams.
20. **Load-combination envelope**: `solve_envelope(model)` (CLI
    `open-cae envelope`) solves the unfactored `base` case and every stored
    combination through the same combination path as `solve_combinations()`,
    including the `loads`/`gravity`/`thermal`/pattern scaling and its stable
    error codes, and returns one row per node (`ux`, `uy`, and `uz` in 3D),
    element (`axial`, `shear`, `moment`) and support (`fx`, `fy`, and `fz` in
    3D) with the signed minimum and maximum and the combination that produced
    each extreme. Shear and moment use the signed end force with the larger
    absolute value, so a truss reports zero shear and moment. A document
    without combinations reports the base case only; `combinationCount` counts
    declared combinations while `cases` always lists `base` first. Values stay
    in the document unit system, ties keep the earlier case, and a failing
    combination returns its solver code instead of a partial envelope.
    `envelope_csv(report)` (CLI `envelope --csv`, SDK `envelope({ csv })`)
    renders that same report as UTF-8 CSV with LF line endings and the header
    `section,id,component,min,max,minCombination,maxCombination`: one row per
    node, element and reaction component in report order, JSON number
    formatting, RFC 4180 quoting for identifiers and combination names, and no
    unit conversion. The CSV is a rendering of the report only; it is not a
    second export format for the other result types, and a failing case writes
    neither the JSON nor the CSV file.
21. **Partial-span loads**: `distributed_load_from`/`distributed_load_to`
    limit a beam transverse load to the fractions `[a, b]` of the length, with
    the intensity varying linearly from the start to the end of that segment
    (CLI `element add --w-from` / `--w-to`). The consistent nodal vector is
    integrated with exact three-point Gauss-Legendre quadrature of the cubic
    Hermite shape functions. Internal-force sampling adds the segment
    resultant and its first moment piecewise, and the deflection sampler
    superposes its own fixed-end particular solution, so a simply supported
    beam loaded over its middle half reproduces `R = |q|L/4`,
    `M_mid = 3|q|L²/32` and `δ_mid = 19|q|L⁴/(2048EI)`.
22. **Inclined supports**: a `direction: [nx, ny]` support rotates the node's
    two translational degrees of freedom into the local frame
    `Q = [[nx, −ny], [ny, nx]]` with `K ← Qᵀ·K·Q` and `F ← Qᵀ·F`, constrains
    the local first component (`u·n = 0`) and rotates displacements, reactions
    and exported matrices back with `Q` (or `Qᵀ·K·Q` in reverse for the
    stiffness). A 45° bar with a roller perpendicular to its axis therefore
    extends by `PL/EA` along the bar, carries no roller reaction, and its
    exported global stiffness is the standard `k·[[c², cs], …]` bar matrix.
23. **Matrix export**: `solve_matrices(model)` assembles the same
    active-degree-of-freedom system the solver uses, exposes the dense global
    `K` (with the inclined-support rotation undone) and the diagonal lumped
    `M`, and labels every row with its node and component (`ux`, `uy`, `uz` or
    `rz`) plus whether a support constrains it. A bar's axial `EA/L` terms, a
    cantilever's `12EI/L³`, `6EI/L²`, `4EI/L` and `2EI/L` terms and the
    `ρAL/2` / `ρIL/2` masses are asserted directly.
24. **Response spectrum**: `solve_response_spectrum(model, modes, axis, spectrum,
    combination, damping)` reuses the mass-normalised modal solution, reads
    `Sa(T)` by linear interpolation (constant beyond the first and last point),
    converts it to `Sd = Sa/ω²`, scales each mode by the signed participation
    factor `Γ` in the requested direction and combines the nodal contribution
    vectors component by component. `combination = "srss"` (the default) takes
    `sqrt(Σ_r R_r²)` per component. `combination = "cqc"` uses the complete
    quadratic combination with one modal damping ratio `ζ` in `(0, 1]` and the
    Der Kiureghian coefficient `ρ_ij = 8ζ²(1+r)r^(3/2) / ((1+r²)² + 4ζ²r(1+r)²)`
    with `r = ω_j/ω_i` and `ρ_ij = 1` when `i = j`; each component is
    `sqrt(Σ_i Σ_j ρ_ij R_i R_j)`. A single free translational degree of freedom
    therefore peaks at the static-equivalent `Sd = Sa·m/k` under both rules, so
    one-mode CQC equals SRSS. An unknown combination reports
    `spectrum/invalid-combination`; CQC without a finite damping ratio in
    `(0, 1]` reports `spectrum/invalid-damping`.
25. **Harmonic frequency response**: `solve_frequency_response(model, modes,
    axis, damping, start_hz, end_hz, points)` treats the document's static
    load vector as the harmonic excitation `f(t) = f·sin(ωt)` of the requested
    direction. With the mass-normalised shapes of the modal solver the modal
    force is `p_r = φ_rᵀ·f` and the steady-state complex amplitude is
    `Σ_r φ_r·p_r / (ω_r² − ω² + 2iζω_rω)`; nodal amplitudes are the modulus of
    the summed translations. The response is read at `points` equally spaced
    frequencies, the peak is the largest sampled amplitude, and every node's
    peak frequency/amplitude is reported. The damping ratio is modal and must
    be positive (zero damping is unbounded at resonance), prescribed non-zero
    displacements and inclined supports are rejected, and the report names the
    peak as a **sampled** value, so a finer sweep resolves it better. A single
    free degree of freedom reproduces the static `F/k` at 0 Hz and the resonant
    `(F/k)/(2ζ)` exactly.
26. **Construction stages**: `solve_construction_stages(model, stages)` clones
    the model per stage, keeps only the elements listed in that stage (omitting
    the list keeps every element), scales nodal/element loads and gravity with
    `loads`/`gravity` exactly like a load combination, validates and solves the
    active submodel, and reports the applied resultant, largest displacement
    and node, extreme axial force/moment and elements, strain energy and
    equilibrium flag. Stage names must be unique and non-empty, active element
    identifiers must exist and `elements: []` is rejected. It is a
    stage-by-stage linear analysis of the active topology: internal forces are
    **not** locked in from earlier stages, so erection screens are supported
    but stress-history/creep-transfer analysis is not.
27. **Geometric-nonlinear truss**: `solve_nonlinear(model, increments,
    max_iterations, tolerance)` supports large displacement of truss members
    only (a beam element returns `capability/not-implemented`). Each bar is
    treated as a corotational bar with axial force `N = E·A·(L − L₀)/L₀` from
    the current chord, and the exact tangent
    `K = E·A/L₀·n·nᵀ + N/L·(I − n·nᵀ)` is assembled. The load and self-weight
    are split into equal increments; each increment runs Newton until
    `‖r_free‖ ≤ tolerance·max(1, ‖λf‖)`, and the report contains the total
    iteration count, final residual and displacements. This formulation
    includes geometric stiffening/softening (a shallow two-bar truss settles
    onto its nonlinear equilibrium), but excludes material plasticity, member
    buckling, beam bending and inclined supports.
28. **Tension-only cables**: `solve_tension_only(model, iterations,
    tolerance)` starts with every cable active, solves, removes compressed
    cables, reassembles stiffness, self-weight and thermal loads without them,
    and repeats until the active set stops changing or the iteration budget is
    spent. The report contains `iterations`, `converged`, each cable's `active`
    state and axial force, and the final `solution`. A slack set that leaves a
    mechanism fails with `model/mechanism`.
29. **Steady-state heat conduction**: `solve_heat(model)` (CLI `open-cae heat`,
    SDK `document.heat()`) assembles one scalar temperature degree of freedom
    per node with the 1D member matrix `k·A/L·[[1, −1], [−1, 1]]`, where `k` is
    the material `conductivity` (`W/(m·K)`) and `A` is the element area
    override or global area. Nodes with a finite `temperature` are Dirichlet
    boundaries, other nodes are free, and a node `heat` flow (W, positive into
    the node) loads the right-hand side. The same reduced dense solve as the
    structural path is used. Element flux is `q = k·A·(T_from − T_to)/L`,
    positive from `from` to `to`; a fixed node reports the boundary reaction
    `K·T − Q`, so `Σreaction + Σapplied heat = 0` for conduction-only models.

    A member may additionally declare a surface convection coefficient `h`
    (W/(m²·K)) and an exposed `perimeter` `P`; the model then supplies one
    `ambientTemperature` `Ta`. The element matrix gains
    `h·P·L/6·[[2, 1], [1, 2]]` and the right-hand side gains
    `h·P·L·Ta/2·[1, 1]`, which is the consistent linear-element discretisation
    of `h·P·(T − Ta)` along the member. The element report gains a
    `convection` object `{coefficient, perimeter, ambientTemperature, heat}`
    with `heat = h·P·L·(T_avg − Ta)` and `T_avg = (T_from + T_to)/2`, positive
    out of the member; the object is omitted for conduction-only members, so
    their JSON keeps the previous byte-for-byte shape. Summing the nodal
    balances of a convection element gives exactly `h·P·L·(T_avg − Ta)`, hence
    `Σfixed-temperature reaction + Σapplied nodal heat = Σconvective heat`:
    without applied nodal heat the reaction equals the convective loss. A
    negative or non-finite `h`, a non-positive or non-finite perimeter, a
    convection member without a perimeter, and convection without a finite
    ambient temperature report `heat/invalid-convection`,
    `heat/invalid-perimeter`, `heat/missing-perimeter` and
    `heat/missing-ambient`; a non-finite ambient reports `heat/invalid-ambient`.
    Convection fields are ignored by every structural solve.

    The report is
    `{format, version, analysis: "heat", units, temperatures, elementFlux,
    reactions, residual, ok}` in model node/element order, with no timestamp.
    Missing conductivity, non-finite heat, no fixed temperature, or an
    unconstrained component report `material/invalid-conductivity`,
    `load/invalid-heat`, `heat/no-fixed-temperature` or the same code for the
    singular component. Units are never converted, so a millimetre document
    must supply a length-consistent conductivity and perimeter.

30. **Parametric element splitting**: `split_element(id, at)` (CLI
    `open-cae element split --id <id> --at <fraction>`, SDK
    `splitElement()`) inserts an interior node at
    `from + at · (to − from)`. The fraction must be finite and strictly inside
    `(0, 1)`. An existing node at exactly those coordinates is reused; otherwise
    a node is created with the deterministic id `<element>-mid` (suffixed with
    `-2`, `-3`, ... when taken). The first segment keeps the original element id
    and the second is appended as `<element>-b` (same suffix rule). Both
    segments inherit the material overrides, kind, inertia, uniform distributed
    load, uniform temperature, surface convection/perimeter and tension-only
    flag; end releases move to the
    outer ends so the interior node stays continuous. Only a uniform
    distributed load (no `distributedLoadEnd` or an equal start/end intensity)
    can be divided losslessly: the two segments carry the same intensity and the
    total resultant is unchanged. Partial-span loads, linearly varying
    distributed loads and through-depth temperature gradients report
    `element/split-unsupported-load`. Nodal loads, supports and masses are
    untouched, and the edit bumps `revision` exactly once.
31. **Polygon section properties**: `polygon_section_report(vertices, units)`
    (CLI `open-cae section polygon --points "x,y;x,y;..."`, SDK
    `sectionPolygon()`) integrates a closed polygon with Green's theorem:
    `A = ½Σ(xᵢyᵢ₊₁ − xᵢ₊₁yᵢ)`, the centroid
    `(1/(6A))Σ(xᵢ + xᵢ₊₁)(xᵢyᵢ₊₁ − xᵢ₊₁yᵢ)` (and the matching `y` sum), the
    origin second moments
    `Ix = (1/12)Σ(yᵢ² + yᵢyᵢ₊₁ + yᵢ₊₁²)(xᵢyᵢ₊₁ − xᵢ₊₁yᵢ)`,
    `Iy = (1/12)Σ(xᵢ² + xᵢxᵢ₊₁ + xᵢ₊₁²)(xᵢyᵢ₊₁ − xᵢ₊₁yᵢ)` and
    `Ixy = (1/24)Σ(xᵢyᵢ₊₁ + 2xᵢyᵢ + 2xᵢ₊₁yᵢ₊₁ + xᵢ₊₁yᵢ)(xᵢyᵢ₊₁ − xᵢ₊₁yᵢ)`,
    followed by the parallel-axis transfer to the centroid, the polar moment
    `J = Ix + Iy` and the radii of gyration `√(Ix/A)`, `√(Iy/A)`. The closing
    vertex may be repeated; it is removed automatically. The outline is
    normalised to a counter-clockwise winding, so clockwise and
    counter-clockwise input give identical results and `Ixy` has a single sign
    convention. Fewer than three vertices report
    `section/polygon-too-few-vertices`, a zero/near-zero area or repeated
    vertices report `section/polygon-degenerate`, crossing edges report
    `section/polygon-self-intersecting` and non-finite coordinates report
    `section/invalid-polygon`. This is a standalone section calculator: the
    properties are not stored in documents and are not consumed by the beam
    solver (which still uses the global parametric `Section`).
32. **Standard section properties**: `i_standard_section_report(h, b, tw, tf)`,
    `tube_standard_section_report(h, b, t)`,
    `channel_standard_section_report(h, b, tw, tf)`, `angle_section_report(h,
    w, t)`, `t_section_report(h, b, tw, tf)`, `ellipse_section_report(w, h)`
    and `triangle_section_report(a)` (CLI `open-cae section i`,
    `section tube`, `section channel`, `section angle`, `section t`,
    `section ellipse` and `section triangle`, SDK `sectionI()`,
    `sectionTube()`, `sectionChannel()`, `sectionAngle()`, `sectionT()`,
    `sectionEllipse()` and `sectionTriangle()`) compute area, centroid,
    centroidal second moments, torsion constant and radii of gyration directly
    from the dimensions. The coordinates put the origin at the lower-left
    corner of the bounding rectangle, so `x` runs across the width and `y`
    along the depth.
    - I-section: `A = 2btf + (h − 2tf)tw`,
      `Ix = (b·h³ − (b − tw)(h − 2tf)³)/12`,
      `Iy = (2tf·b³ + (h − 2tf)tw³)/12`, `Ixy = 0`, centroid `(b/2, h/2)` and
      the thin-walled open `J = (2b·tf³ + (h − 2tf)tw³)/3`.
    - Rectangular tube: `A = bh − (b − 2t)(h − 2t)`,
      `Ix = (b·h³ − (b − 2t)(h − 2t)³)/12`,
      `Iy = (h·b³ − (h − 2t)(b − 2t)³)/12`, `Ixy = 0`,
      `J = 2t(b − t)²(h − t)²/(b + h − 2t)`. This is the *rectangular* tube and
      is distinct from the circular hollow `Section::Tube`.
    - Channel (opening right, web on the left edge): the same area and `Ix` as
      the I-section, `Ix` and `Iy` from the parallel-axis theorem about the
      centroid `x̄ = (2b·tf·(b/2) + (h − 2tf)tw·(tw/2))/A` (so `x̄ ≠ b/2`), and
      the same open-section `J`.
    - Equal-thickness L (angle) with the outer corner at the origin: a `w × t`
      horizontal leg plus a `t × (h − t)` vertical leg, so
      `A = t(w + h − t)` counts the `t × t` corner once. The centroid follows
      from the first moments about the origin, `Ix`, `Iy` and `Ixy` add the
      origin second moments and transfer them to the centroid, and
      `J = (w·t³ + (h − t)·t³)/3`. `Ixy` is generally non-zero and follows the
      polygon sign convention `∫ x·y dA` about the centroid.
    - T-section with the flange at the top and the web centred: `A = b·tf +
      tw(h − tf)`, the centroid `y` is measured from the bottom edge,
      `Ix = (b·h³ − (b − tw)(h − tf)³)/3 − A·ȳ²` (bounding rectangle minus the
      two bottom corner rectangles about the base, then transferred to the
      centroid), `Iy = tf·b³/12 + (h − tf)·tw³/12`, `Ixy = 0` and
      `J = (b·tf³ + (h − tf)·tw³)/3`.
    - Solid ellipse with full width `w` and full height `h`: the semi-axes are
      `a = w/2` and `b = h/2`, so `A = π·a·b`, `Ix = π·a·b³/4`,
      `Iy = π·a³·b/4`, `Ixy = 0`, the centroid is `(a, b)` and the radii of
      gyration are `√(Ix/A) = b/2` and `√(Iy/A) = a/2`. The torsion constant
      is the exact solid-ellipse value `J = π·a³·b³/(a² + b²)`; it equals
      `Ix + Iy` only when `a = b`.
    - Equilateral triangle with side `a`: the base lies along the bottom edge of
      the bounding box and the apex is at the top centre, so the bounding box is
      `a` wide and `h = a·√3/2` tall, the centroid is `(a/2, h/3)`,
      `A = √3/4·a²`, `Ix = Iy = √3·a⁴/96`, `Ixy = 0` by symmetry and the radii
      of gyration are `√(I/A) = a/√24` in both directions. The torsion constant
      is the exact Saint-Venant value `J = √3·a⁴/80`, reported as `polarMoment`
      like the other standard sections; it is not `Ix + Iy`.
    - Equal-thickness Z-section with the origin at the lower-left corner of the
      bounding rectangle: the web is centred (`x = w/2 − t/2 … w/2 + t/2`), the
      top flange runs from the web to the right edge at `y = h − t … h` and the
      bottom flange from the web to the left edge at `y = 0 … t`:

      ```text
                  ┌────────────┐   y = h
                  │            │
                  │            │
                  │       ┌────┘   y = h − t
                  │  web  │
            ┌─────┘       │        y = t
            │             │
            └─────────────┘        y = 0
          x = 0                x = w
      ```

      The three rectangles do not overlap, so `A = t(w + h − t)` and the outline
      is point symmetric about `(w/2, h/2)`. With the flange length
      `b = w/2 + t/2`,
      `Ix = 2(b·t³/12 + b·t·((h − t)/2)²) + t(h − 2t)³/12`,
      `Iy = 2(t·b³/12 + b·t·((w − t)/4)²) + (h − 2t)·t³/12` and
      `Ixy = b·t·(w − t)(h − t)/4`, which is generally non-zero and follows the
      polygon sign convention. `J` uses the thin-walled open approximation with
      the corner counted once, `J = (w + h − t)·t³/3`, identical whether the
      flange net lengths (excluding the web) plus the full web, or the full
      flanges plus the web between them, are summed.
    The report uses the field names of the polygon report (`centroidX`,
    `centroidY`, `ix`, `iy`, `ixy`, `polarMoment`, `radiusOfGyrationX`,
    `radiusOfGyrationY`); `polarMoment` holds `J`, which is not `Ix + Iy` for
    these sections. Dimensions must satisfy `0 < tf < h/2` and `0 < tw < b`
    (I and channel), `0 < t < min(b, h)/2` (tube), `0 < t < min(h, w)` (angle),
    `0 < tf < h` and `0 < tw < b` (T), positive finite `w` and `h` (ellipse),
    positive finite `a` (triangle), or `0 < t < w` and `2t < h` (Z); violations
    report `section/invalid-dimensions`. Results are deterministic and, like the
    polygon calculator, are not stored in documents or consumed by the beam
    solver.
33. **Principal section axes**: `principal_section_report(ix, iy, ixy, units)`
    (CLI `open-cae section principal --ix <Ix> --iy <Iy> --ixy <Ixy>`, SDK
    `sectionPrincipal()`) treats `Ix`, `Iy` and `Ixy` as a centroidal
    second-moment matrix and reports its principal values. With
    `mean = (Ix + Iy)/2` and `r = √(((Ix − Iy)/2)² + Ixy²)`, the major and minor
    principal moments are `I1 = mean + r` and `I2 = mean − r`, the polar moment
    is `Ix + Iy`, and the major-axis angle is `½·atan2(2·Ixy, Ix − Iy)` in
    degrees, normalised to the half-open interval `(-90, 90]` (add `180` when
    the raw angle is `≤ −90`). The angle is counter-clockwise from `+x`. The
    report echoes `ix`, `iy`, `ixy` and `units`, which labels the unit system
    and does not scale the values (they are already second moments in
    `units⁴`). `Ix` and `Iy` must be positive and finite and the matrix must be
    positive semidefinite, `Ix·Iy − Ixy² ≥ −1e-12·max(Ix·Iy, 1)`; zero,
    negative, non-finite or impossible combinations report
    `section/invalid-second-moments`. The calculator is standalone and its
    output is neither stored in documents nor consumed by the solver.
34. **Influence lines**: `solve_influence_line(model, track, quantity,
    direction, points)` (CLI `open-cae influence`, SDK
    `document.influenceLine`) moves a unit load along a track beam element.
    `points` equally spaced load positions including both track ends are
    sampled; each position clones the model, splits the track element at
    strictly interior positions, applies the unit load in `direction` (`x` or
    `y`, default `y`) at the load point and solves the linear-static response.
    The reported quantity is either a reaction component (`fx`/`fy`/`mz`) at a
    supported node or a section force component (`axial`/`shear`/`moment`) at a
    fixed fraction `position` of an element, defaulting to the track element.
    The model must carry no other load — nodal, distributed, thermal, gravity
    or prescribed support displacements report `influence/loads-present` —
    because the influence line is defined as the response to the moving unit
    load alone. A non-beam track reports `influence/track-not-beam`, a section
    position outside `[0, 1]` reports `influence/invalid-position`, and
    `shear`/`moment` on a truss section element reports
    `influence/invalid-component`. When the section position coincides with an
    interior split point, the from-side limit of the section force is reported.
    Each sample carries the load-position fraction and the ordinate; the report
    adds the `min`, `max` and largest-magnitude `governing` extremes. Simply
    supported and cantilever beams reproduce the closed-form ordinates, e.g.
    the midspan bending-moment line of a simply supported beam peaks at
    `−L/4`.
35. **Moving load trains**: `solve_influence_line_train(model, request)`
    (CLI `open-cae influence-train`, SDK `document.influenceLineTrain`)
    envelopes an axle-load train — `axles` forces kept at fixed `spacings`,
    first entry leading — over the influence line of item 34. The line is
    sampled once as a piecewise-linear polyline and reused for every train
    position; the structure is never re-solved. Candidate positions are a
    uniform grid (`step`, a fraction of the track length, default `0.01`)
    plus the exact positions where any axle crosses an influence-line
    vertex, where the envelope of a piecewise-linear ordinate attains its
    extremes. Axles outside the track are excluded from the sum, so the
    train may enter and leave the track partially, and each extreme reports
    the leading-axle coordinate and the number of axles on the track. Empty
    `axles` report `influence/empty-train`, a wrong `spacings` count reports
    `influence/spacing-mismatch`, negative or non-finite spacings report
    `influence/negative-spacing`, non-finite axle loads report
    `influence/invalid-axle-load` and an out-of-range `step` reports
    `influence/invalid-step`; the underlying line keeps the validation of
    item 34.

## Validation and tests
```shell
cargo fmt -p cae-model -p open-cae-napi
cargo test -p cae-model -p open-cae-napi --offline
cargo clippy -p cae-model -p open-cae-napi --offline --all-targets -- -D warnings
pnpm --filter @wae/open-cae run typecheck
```

`projects/crates/cae-model/tests/cae_model.rs` and `tests/session.rs` cover
the existing truss, 3D model, mechanism, validation, export and session paths,
plus:

- single-bar axial tension: `u = PL/(EA)`, `σ = P/A`, `ε = σ/E`;
- symmetric two-bar truss, 3D vertical single bar, 3D tetrahedral symmetric
  structure;
- under-constrained mechanism detection, fully constrained reaction
  equilibrium, backward-compatible loading of old documents;
- validation warnings for `node/unconnected` (a node that no element
  references) and `node/duplicate-coordinates` (two nodes at the same point);
  both warnings never block solving;
- unit-scale warnings: a density in the wrong system reports
  `units/density-scale` and a gravity magnitude that looks like the other
  system (9.81 in a `mm` document, 9810 in an `m` document) reports
  `units/gravity-scale`; neither converts the value or blocks solving;
- reference-integrity notices: a load pattern no combination references
  reports `pattern/unused`, and a combination that scales no active action
  (loads, gravity, patterns, temperature) reports `combination/empty`;
- CSV/VTK/Markdown/HTML content and `.ocae` round trips.

`tests/svg_export.rs`: a two-bar truss and a three-dimensional bar, checked for
`<svg`, `<line`, `<polyline`, the title, the requested scale, support and load
markers, the documented XY projection note, byte-identical repeated exports,
and the stable `export/no-solution`, `export/invalid-scale`,
`export/invalid-contour` and `export/mismatch` errors. The contour cases draw
the fixed-end moment `P·L = 1e6 N·mm` of a cantilever, the uniform 1000 N axial
force of a tension bar and the `FL/(EA) = 0.047619 mm` extension, and check the
legend values and colours.

`tests/deflection.rs`: simply supported uniform load (`5wL⁴/384EI`),
cantilever tip load (`PL³/3EI` and `PL²/2EI`), cantilever uniform load
(`wL⁴/8EI`), triangular load (`11wL⁴/120EI`), a straight truss sample and the
session JSON export.

`tests/nodal_stress.rs`: a cantilever end-stress average, a shared node that
combines two element ends, a truss nodal stress without utilisation and the
session JSON export.

`tests/load_cases.rs`: load/gravity factor scaling of the `base` row and
combinations, a gravity-dominated case, a model without combinations and the
session JSON export.

`tests/model_summary.rs`: a straight bar checked for length `L`, volume `A·L`,
member mass `ρAL`, nodal mass, mass centroid, and the translation/constraint
counts; beam rotations disappear as ends are released; a 3D bar reports three
translations per node; invalid density, area, nodal mass, coordinates and
unknown node references report their stable codes.

`tests/csv_forces.rs`: a simply supported beam under `q = -1 N/mm` exports
`forces.csv` whose first sampled shear is `-qL/2` and midspan moment is
`-qL²/8`, the `stations` option changes only the force table, and a station
count below 2 reports `analysis/invalid-stations`.

`tests/partial_load.rs`: a simply supported beam loaded over its middle half —
reactions `|q|L/4`, the piecewise shear and moments (`3|q|L²/32` at midspan),
the sampled ends against the element end forces, the midspan deflection
`19|q|L⁴/(2048EI)`, a full-span equivalence check and the load-span validation
errors.

`tests/node_merge.rs`: endpoint re-pointing, component-wise load and spring
merging, OR-ed support flags, and the rejected cases (collapsed element,
differing settlements, inclined direction against a rigid support, unknown or
identical ids).

`tests/entity_removal.rs`: node removal rejects an element reference, reports
support and load references, rejects unknown ids, bumps the revision and
persists the ordered removal.

`tests/entity_updates.rs`: in-place node movement and element endpoint
reconnection keep identifiers and references so a solved model can be edited
without rebuilding its topology.

`tests/element_split.rs`: a split axial bar keeps `PL/EA` and the same axial
force, a split cantilever keeps `PL³/(3EI)` and the fixed-end moment, an
existing node at the split point is reused, attributes and outer releases are
copied, a uniform load keeps the total reaction, invalid fractions, unknown
elements and partial-span/linear/gradient loads are rejected, and the session
split persists across a reopen with a manual undo/redo path.

`tests/result_queries.rs`: `nodeDisplacement`, `elementResult` and
`nodeReaction` match the analytic bar solution, and unknown identifiers report
stable codes.

`projects/crates/cae-model/tests/beam_modal.rs` asserts analytical solutions:

- cantilever beam with an end concentrated force: end deflection `PL³/(3EI)`,
  fixed-end reaction moment `PL` (relative error `1e-9`);
- cantilever beam with a uniform distributed load: end deflection `wL⁴/(8EI)`
  (`1e-9`);
- cantilever beam with a triangular load of peak `q0` at the fixed end: end
  deflection `q0L⁴/(30EI)` and fixed-end reaction moment `q0L²/6` (`1e-9`);
- simply supported beam with a midspan concentrated force: midspan deflection
  `PL³/(48EI)` and maximum bending moment `PL/4` (`1e-9`);
- fixed-free axial bar (10 elements): first frequency within `< 3%` of
  `(1/(4L))√(E/ρ)`, and its first-mode effective mass fraction within `< 5%`
  of the analytical `8/π²`;
- single free translational degree of freedom: effective mass equals the free
  nodal mass `ρAL/2`, its signed participation factor equals `√(ρAL/2)` and
  `totalMass` equals `ρAL` (`1e-12`);
- every reported mode satisfies `participationFactor² = effectiveMass` in all
  directions, with the cumulative translational mass fraction staying below
  one (`1e-12`);
- cantilever beam (40 elements): first bending frequency within `< 5%` of
  `1.875104²·√(EI/(ρAL⁴))/(2π)`;
- the same cantilever with a concentrated tip mass: the first frequency drops
  below the distributed-mass result and `totalMass` grows by exactly the added
  mass (`1e-12`); setting the mass back to `0` restores the original frequency
  exactly;
- nodal masses reject negative, NaN, infinite and unknown-node requests
  (`node/invalid-mass`, `node/unknown-id`), and `validate()` reports an invalid
  mass edited directly into the document;
- pinned-pinned beam column (20 elements): first linear buckling factor within
  `0.1%` of the Euler load `P_cr = π²EI/L²`;
- three-dimensional tetrahedron truss: the two lateral buckling factors match
  the closed-form geometric-stiffness eigenvalue
  `E·A·R²/(N·(R²+2H²))` (relative error `1e-8`);
- vertical bar self-weight: sum of reactions equals total weight (relative
  error `1e-9`);
- internal force sampling: cantilever `V(0)=wL`, `M(0)=−wL²/2`, `M(L)=0`;
  simply supported midspan `M=PL/4`, `V=P/2`; sampled end values reproduce the
  reported element end forces; truss axial force is constant and shear/moment
  are zero; fewer than two stations are rejected (`1e-9`);
- rejection/behaviour tests for 3D beams, truss distributed loads,
  zero-density modal analysis, three-dimensional buckling, and byte-identical
  Markdown **and** HTML report determinism.

`tests/prescribed.rs` asserts prescribed-displacement behaviour:

- axial bar with a prescribed end elongation: `N = EAδ/L`, `σ = Eδ/L` and
  equal/opposite end reactions (`1e-12`);
- propped cantilever with a `δ` roller settlement: tip rotation `−1.5δ/L`,
  roller reaction `−3EIδ/L³`, fixed reaction `+3EIδ/L³`, fixed-end moment
  `3EIδ/L²` (`1e-12`);
- rejection of empty prescriptions, unknown nodes, `uz` in 2D and `rz` in 3D,
  plus the `support/prescribed-unconstrained` validation finding;
- `.ocae` round trips that preserve the `prescribed` block.

`tests/combinations.rs` asserts load-combination behaviour:

- dead (`gravity` only) and live (`loads` only) combinations reproduce the
  self-weight `wL⁴/(8EI)` and tip-load `PL³/(3EI)` cantilever deflections
  (`1e-9`);
- the `D+L` solution is exactly the linear superposition of the dead and live
  solutions, including the fixed-end reaction moment (`1e-12`);
- the envelope selects `D+L`, reports the governing node/element and matches
  the individual solutions;
- a restrained thermal bar reports `N = −EAαΔT` at `thermal = 1`, exactly half
  that force at `thermal = 0.5` and zero at `thermal = 0`, and the factor
  round-trips through `.ocae` while a document without the field defaults to
  `1`;
- empty lists (`combination/none`), non-finite factors
  (`combination/invalid-factor`), empty and duplicate names are rejected,
  combinations round-trip through `.ocae`, and the session writes the JSON
  report.

`tests/load_patterns.rs` asserts named load patterns:

- two patterns mixed with `factor 1` and `factor 2` reproduce the equivalent
  superposed nodal load case (`1e-9`), and pattern loads add to the applied
  `--loads` case exactly like the reference model;
- the solved combination and the `.ocae` round trip preserve the pattern list,
  and `remove_pattern_load()` keeps an empty pattern while
  `remove_pattern()` deletes it;
- pattern edits reuse the plain-load codes (`load/unknown-node`,
  `document/dimension-mismatch`, `pattern/invalid-name`), and validation
  reports `pattern/duplicate-name`, `combination/duplicate-pattern` and
  `combination/unknown-pattern`.

`tests/library.rs` asserts the material and section library:

- rectangular, circular, I-section, box, tube and channel area, strong/weak
  inertia, torsion constant, extreme fibre and section modulus match the
  closed-form formulas (`1e-12`);
- a polygon rectangle reproduces `A = bh`, `Ix = bh³/12`, `Iy = hb³/12`,
  `J = Ix + Iy` and the analytic radii of gyration, and a right triangle
  reproduces `A = bh/2`, `Ix = bh³/36`, `Iy = hb³/36` and
  `Ixy = −b²h²/72` about the centroid (`1e-12`);
- clockwise, counter-clockwise and explicitly closed vertex lists produce
  identical polygon reports, while fewer than three vertices, collinear
  (zero-area) lists, self-intersecting outlines and non-finite coordinates
  report the stable `section/polygon-*` codes;
- the standard I, rectangular-tube, channel, L (angle) and T calculators
  reproduce `Ix = (b·h³ − (b − tw)(h − 2tf)³)/12`, the tube
  hollow-subtraction formulas, the parallel-axis channel `Iy`, the
  centroid positions (including the non-zero channel `x̄ = 1681/67 mm` for
  `200 × 80 × 10 × 6`) and the open-section torsion constants (`1e-12`), and
  agree with the parametric channel section;
- the L section reproduces `A = t(w + h − t)`, `A = 2400`, `x̄ = 23.75`,
  `ȳ = 48.75`, `Ix = 5 576 250`, `Iy = 2 026 250`, `Ixy = −1 968 750` and
  `J = 80 000` for `h = 150, w = 100, t = 10`, and its closed polygon agrees
  with all four second moments (`1e-12`);
- the T section reproduces `A = b·tf + tw(h − tf)`, `ȳ = 5015/67 mm` from the
  bottom for `100 × 80 × 6 × 10`, a rectangle-decomposed `Ix`, the symmetric
  `Iy`, `Ixy = 0` and `J`, and its closed polygon agrees within `1e-12`;
- the solid ellipse reproduces `A = πab`, `Ix = πab³/4`, `Iy = πa³b/4`,
  `Ixy = 0`, the centroid `(a, b)`, the radii `b/2` and `a/2` and the exact
  torsion constant `J = πa³b³/(a² + b²)`, agrees with a 256-edge inscribed
  ellipse polygon on `area`, `Ix` and `Iy` within `1e-3`, and reports
  `section/invalid-dimensions` for zero, negative and non-finite dimensions;
- the equilateral triangle reproduces `A = √3/4·a²`, the centroid
  `(a/2, a·√3/6)`, `Ix = Iy = √3·a⁴/96`, `Ixy = 0`, the exact torsion constant
  `J = √3·a⁴/80` and the radii `a/√24` for `a = 100` (`1e-12`), its closed
  outline polygon agrees with area, centroid and all three second moments
  within `1e-12`, and zero, negative, NaN or infinite `a` report
  `section/invalid-dimensions`;
- the equal-thickness Z section reproduces `A = t(w + h − t) = 2900`,
  `x̄ = 50`, `ȳ = 100`, `Ix = 14 796 666.67`, `Iy = 849 166.67`,
  `Ixy = 2 351 250` and `J = 290 000/3` for `200 × 100 × 10`, and its closed
  polygon agrees with area, centroid and all three second moments within
  `1e-12`, while zero or oversized `t` report `section/invalid-dimensions`;
- invalid flange, web, leg and wall thicknesses report
  `section/invalid-dimensions`, and repeated calls return identical reports
  with the documented CLI/SDK field names;
- the principal-axes calculator reproduces `I1 = 100`, `I2 = 50` and angle `0`
  for `(100, 50, 0)`, `120`/`80` and angle `45°` for `(100, 100, 20)`, `100`/`50`
  and angle `90°` for `(50, 100, 0)`, angle `−45°` for a negative `Ixy`, and
  normalises the `−90°` boundary to `+90°`; it reports `Ix + Iy` as the polar
  moment, rejects zero/negative `Ix` or `Iy`, impossible `Ixy`
  (`Ix·Iy < Ixy²`) and non-finite input with `section/invalid-second-moments`,
  and repeated calls return identical reports with the documented CLI/SDK field
  names;
- presets convert between `mm` (`210000 MPa`, `7.85e-9 t/mm³`) and `m`
  (`2.1e11 Pa`, `7850 kg/m³`);
- a cantilever beam that omits `inertia` uses the stored section and
  reproduces `PL³/(3EI)` (`1e-9`);
- a preset plus section round-trips through the document, validation accepts
  the model and the session solves it;
- invalid section dimensions, non-positive allowable stress and a beam without
  inertia or section are rejected with stable codes.

`tests/stress.rs` asserts beam stress recovery:

- a cantilever tip load reproduces `|σ| = PL/S` at the fixed end with
  `S = I/c` (`1e-9`), a zero free-end moment and the expected utilisation;
- an axial force shifts the extreme-fibre stresses to `N/A ± PL/S`;
- truss elements report `N/A` without a section and no utilisation;
- a beam without a material section is rejected with
  `material/missing-section`, and fewer than two stations with
  `analysis/invalid-stations`;
- the session writes a valid `open-cae-stress-report` JSON file.

`tests/buckling_combinations.rs` asserts buckling under several load
combinations:

- a pinned-pinned column under `P` and `2P` reports Euler factors
  `π²EI/(L²·P)` and `π²EI/(2L²·P)`, whose ratio is exactly two (`1e-3`);
- the envelope selects `2P` and repeats its first factor (`1e-12`);
- a zero-load combination produces no positive factor, is skipped and cannot
  win the envelope;
- an empty combination list (`combination/none`) and `modes = 0`
  (`analysis/invalid-modes`) are rejected;
- the session writes a valid `open-cae-buckling-combination-report` JSON file.

`tests/springs.rs` asserts linear elastic spring supports:

- an axial bar with an end spring reproduces `u = P/(EA/L + k)`, the fixed
  reaction `−EA·u/L`, the spring force `−k·u` and a balanced reaction summary
  (`1e-12`);
- a cantilever with a tip vertical spring reproduces
  `δ = P/(3EI/L³ + k)`, a fixed-end reaction `P − kδ`, the spring force `kδ`
  and a balanced summary (`1e-12`);
- a cantilever whose root rotation is held by a rotational spring reproduces
  `θ = −PL/kr` and `δ = PL³/(3EI) + PL²/kr`, with the spring moment `PL`
  (`1e-12`);
- negative, zero and unknown-node springs are rejected; `kz` in 2D, `kr` at a
  truss-only node and a spring plus rigid flag on the same component produce
  stable validation codes;
- a spring round-trips through `.ocae`, reopens through the session and
  solves.

`tests/reactions.rs` asserts the reaction summary:

- a cantilever tip load gives `fy = +P`, `mz = +PL`, `applied.fy = −P` and
  zero unbalanced force (`1e-9`);
- the session writes a JSON report with `balanced: true` and the support node;
- CSV export writes `reactions.csv` with one header row and one row per
  support (`1e-9`);
- static solutions keep the `reactionSummary` field.

`tests/inclined_support.rs`, `tests/matrices.rs` and
`tests/response_spectrum.rs` assert the new capabilities:

- a 45° bar with an inclined roller perpendicular to its axis extends by
  `PL/EA` and carries no roller reaction (`1e-9`), and a roller along Y matches
  the global `dy` support;
- the exported matrix round-trips through the inclined frame: the global bar
  stiffness `k·[[c², cs], …]`, the constrained local row and a bar's `EA/L`,
  `12EI/L³` and `4EI/L` terms plus the lumped `ρAL/2`/`ρIL/2` masses
  (`1e-12`);
- a single free translational degree of freedom peaks at `Sd = Sa·m/k` with
  `Γ = √m` (`1e-9`), one-mode CQC reproduces SRSS exactly, the spectrum is
  linearly interpolated, and empty/negative period spectra and unavailable axes
  are rejected;
- a two-mode bar frame with closed-form frequencies and mode vectors reproduces
  the two-mode CQC combination `sqrt(Σ_i Σ_j ρ_ij R_i R_j)` (`1e-8`), a
  damping ratio of `1e-9` converges to the SRSS result (`1e-6`), and an unknown
  combination or a missing/out-of-range CQC damping ratio reports
  `spectrum/invalid-combination` / `spectrum/invalid-damping`.

`tests/frequency_response.rs`, `tests/construction_stages.rs` and
`tests/nonlinear.rs` assert the newer capabilities:

- the single-degree-of-freedom bar reproduces the static `F/k` at 0 Hz and the
  resonant amplitude `(F/k)/(2ζ)` at the natural frequency (`1e-9`), with the
  peak frequency equal to the analytic `f_n`; zero modes, fewer than two
  points, an empty range, zero damping and an unavailable axis are rejected;
- activation of only the first of two beam elements gives the cantilever
  `PL³/3EI`, activating both gives the propped-cantilever `7P(2L)³/768EI`,
  doubling the stage factor doubles the displacement and quadruples the strain
  energy (`1e-9`); empty, duplicate, unnamed, non-finite and unknown-element
  stages are rejected;
- a bar under axial load reproduces `PL/EA`, and a shallow two-bar truss
  settles onto the scalar nonlinear equilibrium (`|u_y| = v`, bisection
  reference, `1e-6`) with a small final residual; beam models, zero
  increments/iterations and a non-positive tolerance are rejected.

`tests/heat.rs` asserts steady-state conduction:

- a single bar with `k = 50 W/(m·K)`, `A = 1e-4 m²`, `L = 1 m` and
  `ΔT = 100 K` reports `q = 0.5 W` and reactions `±0.5 W` (`1e-12`);
- two equal series bars halve the end-to-end `ΔT` at the free interface and
  report the same flux in both members;
- a nodal heat load on an otherwise adiabatic free end produces
  `T = T_fixed − Q·L/(kA)`, and its boundary reaction balances the load;
- an adiabatic free node reaches the fixed temperature with zero flux; missing
  conductivity, non-finite heat, no fixed temperature and empty models report
  the stable codes;
- adding node temperature/heat fields leaves the structural solution
  bit-for-bit unchanged, zero heat/absent temperature are omitted from JSON,
  and two heat solves serialize identically without timestamps.

`tests/heat_convection.rs` asserts surface convection:

- a two-node bar with one fixed end and a free end matches the hand-assembled
  2×2 convection system (`T_free`, flux, reaction and `h·P·L·(T_avg − Ta)`) to
  `1e-12`;
- a multi-element fin converges to the analytic insulated-tip
  `Ta + (T0 − Ta)·cosh(m(L − x))/cosh(mL)` solution under refinement
  (2 → 10 → 40 elements, final relative error below `2e-3`);
- the fixed-node reaction equals the summed convective loss, so the signed
  balance `reaction − loss` vanishes;
- missing ambient, missing perimeter, negative convection, zero perimeter and
  non-finite ambient report `heat/missing-ambient`, `heat/missing-perimeter`,
  `heat/invalid-convection`, `heat/invalid-perimeter` and `heat/invalid-ambient`,
  and the setters reject invalid values;
- without convection fields the document, the heat report and the structural
  solution are unchanged, and `ambientTemperature` alone does not affect heat
  results;
- `convection`, `perimeter` and `ambientTemperature` survive a save/reopen and
  reproduce the same heat solution.

`tests/tension_only.rs` asserts the cable active set:

- a tensioned cable stays active with `N = 500 N` in a two-bar collinear
  benchmark (`E=210000 MPa`, `A=100 mm²`, `L=1000 mm`);
- reversing the load makes the compressed cable slack after two solves
  (`N = 0`, the remaining bar carries `1000 N` in tension); reversing again
  reactivates it;
- a slack member that leaves a single-bar mechanism fails with
  `model/mechanism`;
- beam elements reject the flag with `element/tension-only-requires-truss`.

For the single-bar benchmark (L=1000 mm, E=210000 MPa, A=100 mm², P=1000 N)
the analytical displacement is `0.0476190476 mm` and the stress is `10 MPa`;
the tests assert against the analytical expressions with a relative error
threshold of `1e-9`.

## Abaqus `.inp` import

`open-cae import <input.inp> <output.ocae>` (SDK `OpenCAE.importAbaqus()`,
native `import_abaqus`) reads a documented subset into a **new** document and
prints a report. Its `losses` array lists every recognised but unrepresented
feature with `code`, `keyword`, `line` and `detail`, ordered by first
occurrence and deduplicated; the output is deterministic. Units are never
converted: `--units mm|m` (default `mm`) only labels the input numbers because
Abaqus inputs do not declare a unit system.

Supported:

- `*Node` with 2 or 3 coordinates. The dimension is inferred: any non-zero `z`
  produces a 3D document, otherwise 2D.
- `*Element` with `T3D2` (truss), `B31` (2D beam), and the solid types `C3D4`,
  `C3D8` and `C3D8R`. Solid elements cannot be represented by this product and
  are skipped with one `import/abaqus-solid-element` loss per block; any other
  element type is an error.
- `*Nset`/`*Elset` (including `generate` ranges) and the `elset=` option of
  `*Element`, `*Solid Section`, `*Beam Section` and `*Truss Section`.
- `*Material` with isotropic `*Elastic` (E; Poisson's ratio is reported as an
  ignored property). Multiple materials are allowed: the first supplies the
  global material and each section supplies the per-element E and area.
- `*Truss Section` (area), `*Beam Section` (`RECT` width/height or `CIRC`
  radius, from which area and inertia are derived) and `*Solid Section` (no
  area; a truss using one fails with `import/abaqus-element-without-section`).
- `*Boundary` with numeric DOFs 1..6 or the `ENCASTRE`/`PINNED` presets, an
  optional prescribed value, and node-id or `*Nset` targets. DOFs 4/5 are
  always losses, DOF 3 is a loss in a 2D document, and DOF 6 applies only at
  nodes connected to a beam.
- `*Cload` with DOFs 1, 2, 3 (3D) and 6 (beam nodes); other DOFs are
  `import/abaqus-unsupported-load` losses.
- `*Dload` with the beam-local `PY` label on beams, mapped to the uniform
  distributed load; other labels and truss targets are losses.

`*Part`, `*End Part`, `*Assembly` and `*End Assembly` are structural no-ops.
`*Instance` is a loss because transforms are not applied, and `*Include` fails
with `import/abaqus-unsupported-include` instead of reading another file.
Every other keyword or option becomes `import/abaqus-unsupported-keyword` or
`import/abaqus-ignored-option`. Errors are
`import/abaqus-malformed-line`, `import/abaqus-unknown-node`,
`import/abaqus-unknown-element`, `import/abaqus-unknown-material`,
`import/abaqus-duplicate-node`, `import/abaqus-duplicate-element`,
`import/abaqus-unsupported-element-type`,
`import/abaqus-unsupported-beam-section`, `import/abaqus-unsupported-beam`,
`import/abaqus-element-without-section` and `import/abaqus-no-nodes`; every
message names the one-based input line.

## Limitations

- Beams are **2D Euler-Bernoulli only** (no shear deformation, no
  axial-bending coupling); bending-moment **end releases are supported** as
  condensed hinges. **3D beams/frames (beam3d / frame3d) are not implemented**
  and return `capability/not-implemented`.
- Static analysis is **linear elastic, small deformation**, with one
  deliberate exception: `geometricNonlinear` runs a large-displacement Newton
  iteration for **truss models only**. Material nonlinearity, beam/frame large
  displacement, plasticity, dynamic time history, contact and prestress are not
  supported. Prescribed displacements are supported as a single static
  kinematic boundary condition, not as a time-dependent or multi-step loading
  history.
- Construction stages solve each active topology **independently**: loads and
  gravity are scaled per stage, but internal forces are not locked into
  members that carry them onwards, so a true staged stress-history/creep
  analysis is not modelled.
- The harmonic frequency response uses **modal superposition with one damping
  ratio for every mode** and reports peaks read from the sampled frequencies;
  it is not a direct complex solve, and zero damping, non-zero prescribed
  displacements and inclined supports are rejected.
- Buckling is a **linearized eigenvalue analysis** (Euler buckling) based on
  the static axial forces of one load case. It does not track post-buckling
  behaviour, imperfection sensitivity, plasticity or load-history effects.
  Three-dimensional beams remain unsupported, so three-dimensional buckling is
  limited to truss models.
- Loads are limited to **nodal concentrated forces/moments**, **element
  transverse loads that are uniform or linear along the member in the local
  `+y` direction** (optionally limited to part of the span), **uniform member
  temperature changes** (axial expansion only), a **through-depth
  temperature gradient** (thermal bending), **fixed nodal temperatures**,
  **concentrated nodal heat flows** and a **constant surface convection with an
  ambient temperature** (steady conduction only). Heat transfer beyond axial
  conduction plus constant linear-element convection — radiation,
  temperature-dependent `h`/`k`, transient/time stepping, contact conductance
  and coupled thermo-mechanical fields — is not supported,
  and arbitrary curved distributed loads and loads in a global direction are
  also not supported. A load combination scales element temperature actions
  through its `thermal` factor, but fixed nodal temperatures are heat-analysis
  boundary conditions and are never part of a structural combination.
- Named load patterns hold **concentrated nodal forces and moments only**. They
  can be mixed into any combination by name and factor, but element distributed
  loads, temperatures, gravity and supports are not pattern members, so two
  different element-load or thermal layouts still cannot be mixed by a
  combination.
- In-place edits (`node update`, `element update`) move nodes and reconnect
  **existing truss/beam endpoints**, and `element update` can also replace the
  distributed load, uniform temperature change, gradient and convection data of
  one member. They never rebuild geometry, remesh or rewire a whole network, and
  a move that would collapse an element to zero length is rejected. Editing a
  node keeps every element, support and load reference to it.
- `element split` inserts **one interior node** and only divides members whose
  loads can be represented exactly by two segments. Partial-span loads,
  linearly varying distributed loads and temperature gradients are rejected
  with `element/split-unsupported-load`, and the operation does not split,
  redistribute or renumber supports, nodal loads, masses or unrelated members.
  It is a single atomic edit with one revision increment and no undo stack.
- `node merge` is a topology cleanup: the `from` node is removed and the `into`
  node keeps its coordinates and id, even when the two nodes were not
  coincident. Loads and springs add component-wise; two different prescribed
  displacements or inclined directions, or an inclined direction combined with
  any other constraint frame, reject the whole merge
  (`support/merge-conflict`). It does not renumber nodes or snap coordinates.
- The material and section are **global**: all elements share one material and
  one parametric section. Per-element `E`/`A` overrides still exist, but there
  is no per-element section library yet. Custom polygon, standard-section and
  principal-axes properties are standalone calculators: they are neither
  stored in `.ocae` documents nor fed into the beam solver as inertia or
  extreme-fibre distances.
- Spring supports are **linear elastic and two-way**: no
  tension-only/compression-only (gap) behaviour, no friction, no plasticity,
  and no nonlinear force-displacement law is modelled.
- Tension-only behaviour applies to **truss elements only**. The active-set
  iteration removes a cable when its final axial force is compressive and does
  not partially release, add slack/pretension, couple with geometric
  nonlinearity, or model beam/cable bending. The iteration accepts
  deactivation only; a previously slack member is reactivated only by a new
  solve with a different load state.
- Inclined supports constrain **one translational direction** in
  two-dimensional documents; a prescribed settlement along that direction, a
  second independent normal at the same node, and three-dimensional inclined
  constraints are not modelled.
- Beam stress recovery is **sampled** at the requested stations and uses the
  global material section. Shear stress distributions, stress concentrations
  and fibre-level plasticity are not modelled.
- Modal analysis uses a **lumped mass matrix** and ignores constrained degrees
  of freedom; frequencies and mode shapes are finite element approximations.
  Results contain only the lowest few modes. Response-spectrum analysis offers
  SRSS and CQC, but CQC uses **one modal damping ratio for every mode** rather
  than a per-mode damping table. There is no rotational/rocking excitation and
  no ductility or inelastic spectrum modification. Harmonic response inherits
  the same mass matrix and shape approximation.
- Self-weight is applied as **lumped forces** (trusses) or **consistent loads**
  (beam transverse direction); this is exact in equilibrium and approximate in
  displacement, and does not replace a consistent mass matrix.
- The solver is **dense**: memory `O(n²)` and time `O(n³)`, suitable for small
  models but not for large engineering models.
- Mechanism detection is based on numerical pivot thresholds; extremely
  ill-conditioned but theoretically stable structures may be misclassified.
- CSV/VTK numerical units follow the document declaration and the exporter
  performs no conversion; VTK uses the legacy ASCII format. Markdown and HTML
  reports contain no timestamps, so repeated exports are byte-identical. CSV
  node columns are `id,x,y,z,ux,uy,uz` (z/uz are 0 in 2D), `reactions.csv`
  columns are `node,fx,fy,fz,mz`, and `forces.csv` columns are
  `element,from,to,kind,position,axialForce,shear,moment` with 11 stations per
  member by default (`export --stations` changes the count).
- SVG contours are **sampled**: each straight coloured segment takes the
  governing signed value of its two stations, so a peak between stations is
  resolved only as finely as `--stations` (`--stations 21` draws twenty
  segments per member). The drawing still projects onto the global X–Y plane,
  so a contour of a Z-direction quantity is not geometrically visible, and the
  colour scale is always relative to the extremes of the exported model, not a
  fixed engineering scale.
- Other external formats (STEP, Nastran) are neither imported nor exported.
