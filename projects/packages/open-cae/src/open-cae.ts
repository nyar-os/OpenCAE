/**
 * `new OpenCAE(options?)` analysis session.
 *
 * The constructor is synchronous and opens nothing: no network, no disk scan,
 * no window, no file. Native initialisation happens inside the async methods, so
 * a failure is always awaited and always carries a stable code.
 *
 * `create()` and `open()` return a {@link CaeDocument}; the document exposes
 * editing, validation, solving, export and save through the shared native
 * session.
 */

import { CaeDocument } from './document.js';
import { OpenCAEError } from './errors.js';
import * as service from './service.js';
import type {
    AngleSectionGeometry,
    ChannelSectionGeometry,
    CreateOptions,
    Dimension,
    EllipseSectionGeometry,
    ISectionGeometry,
    MaterialLibrary,
    OpenCAECapabilities,
    OpenCAEOptions,
    PolygonPoint,
    PolygonSectionReport,
    RectangularTubeGeometry,
    Section,
    SectionReport,
    StandardSectionReport,
    TSectionGeometry,
    TriangleSectionGeometry,
    Units,
    ZSectionGeometry,
} from './types.js';

/** Analysis session. One instance owns the documents it opens. */
export class OpenCAE {
    /** BCP-47 tag used to pick the CLI locale. */
    readonly locale: string;
    /** Default unit system of documents created by this session. */
    readonly units: Units;
    /** Default spatial dimension of documents created by this session. */
    readonly dimension: Dimension;
    private readonly documents = new Set<CaeDocument>();
    private closed = false;

    constructor(options: OpenCAEOptions = {}) {
        if (options.units !== undefined && options.units !== 'mm' && options.units !== 'm') {
            throw new OpenCAEError('session/argument-invalid', '`units` must be `mm` or `m`', {
                details: { feature: 'session.units' },
            });
        }
        if (options.dimension !== undefined && options.dimension !== 2 && options.dimension !== 3) {
            throw new OpenCAEError('session/argument-invalid', '`dimension` must be 2 or 3', {
                details: { feature: 'session.dimension' },
            });
        }
        this.locale = options.locale ?? 'en-US';
        this.units = options.units ?? 'mm';
        this.dimension = options.dimension ?? 2;
    }

    /** Whether {@link dispose} has run. */
    get disposed(): boolean {
        return this.closed;
    }

    /**
     * Solver and I/O disclosure for the installed engine.
     * Read this instead of treating a method name as a capability.
     */
    async capabilities(): Promise<OpenCAECapabilities> {
        this.assertActive();
        return service.capabilities();
    }

    /**
     * Built-in material presets and supported section shapes.
     * `units` defaults to the session unit system.
     */
    async library(units: Units = this.units): Promise<MaterialLibrary> {
        this.assertActive();
        assertUnits(units);
        return service.library(units);
    }

    /**
     * Derived properties of one parametric section: area, inertias, torsion
     * constant, extreme fibre, section modulus and radii of gyration.
     * `units` labels the unit system the dimensions were supplied in.
     */
    async sectionProperties(section: Section, units: Units = this.units): Promise<SectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionProperties(section, units);
    }

    /**
     * Derived properties of a closed polygon section: area, centroid, second
     * moments about centroidal axes, polar moment and radii of gyration.
     * `points` are `[x, y]` pairs in `units`; the closing vertex may be omitted
     * and either winding order is accepted.
     */
    async sectionPolygon(points: readonly PolygonPoint[], units: Units = this.units): Promise<PolygonSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionPolygon(points, units);
    }

    /**
     * Derived properties of a doubly symmetric I-section: area, centroid,
     * second moments, torsion constant and radii of gyration. Requires
     * `0 < flangeThickness < height/2` and `0 < webThickness < width`.
     */
    async sectionI(geometry: ISectionGeometry, units: Units = this.units): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionI(geometry, units);
    }

    /**
     * Derived properties of a hollow rectangular tube. Requires
     * `0 < thickness < min(width, height)/2`.
     */
    async sectionTube(
        geometry: RectangularTubeGeometry,
        units: Units = this.units,
    ): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionTube(geometry, units);
    }

    /**
     * Derived properties of an open channel whose opening faces right. The
     * centroid `x` is measured from the web face, so it is not `width/2`.
     */
    async sectionChannel(
        geometry: ChannelSectionGeometry,
        units: Units = this.units,
    ): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionChannel(geometry, units);
    }

    /**
     * Derived properties of an equal-thickness L (angle) section. The `t × t`
     * corner is counted once, so `area = t(width + height − t)`. Requires
     * `0 < thickness < min(height, width)`.
     */
    async sectionAngle(
        geometry: AngleSectionGeometry,
        units: Units = this.units,
    ): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionAngle(geometry, units);
    }

    /**
     * Derived properties of a T-section whose flange is at the top. The
     * centroid `y` is measured from the bottom edge. Requires
     * `0 < flangeThickness < height` and `0 < webThickness < width`.
     */
    async sectionT(
        geometry: TSectionGeometry,
        units: Units = this.units,
    ): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionT(geometry, units);
    }

    /**
     * Derived properties of a solid ellipse. The semi-axes are `width/2` and
     * `height/2`, the centroid is `(width/2, height/2)` and `Ixy = 0`.
     * `polarMoment` carries the torsion constant `πa³b³/(a² + b²)`, not
     * `Ix + Iy`. Requires positive, finite `width` and `height`.
     */
    async sectionEllipse(
        geometry: EllipseSectionGeometry,
        units: Units = this.units,
    ): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionEllipse(geometry, units);
    }

    /**
     * Derived properties of a solid equilateral triangle. The base lies along
     * the bottom edge of the bounding box and the apex is at the top centre,
     * so the centroid is `(side/2, side·√3/6)` and `Ixy = 0`.
     * `polarMoment` carries the exact Saint-Venant torsion constant
     * `√3·side⁴/80`, not `Ix + Iy`. Requires a positive, finite `side`.
     */
    async sectionTriangle(
        geometry: TriangleSectionGeometry,
        units: Units = this.units,
    ): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionTriangle(geometry, units);
    }

    /**
     * Derived properties of an equal-thickness Z-section. The web is centred
     * and the top and bottom flanges extend to opposite edges, so the section
     * is point symmetric with `Ixy` generally non-zero. `polarMoment` carries
     * the thin-walled torsion constant `(width + height − thickness)·t³/3`.
     * Requires `0 < thickness < width` and `2·thickness < height`.
     */
    async sectionZ(
        geometry: ZSectionGeometry,
        units: Units = this.units,
    ): Promise<StandardSectionReport> {
        this.assertActive();
        assertUnits(units);
        return service.sectionZ(geometry, units);
    }

    /** Creates an empty document at `path` and returns it. */
    async create(path: string, options: CreateOptions = {}): Promise<CaeDocument> {
        this.assertActive();
        assertPath(path);
        assertUnits(options.units);
        assertDimension(options.dimension);
        const summary = service.create(path, {
            units: options.units ?? this.units,
            dimension: options.dimension ?? this.dimension,
        });
        const document = new CaeDocument(summary);
        this.documents.add(document);
        return document;
    }

    /** Opens the document at `path` and returns it. */
    async open(path: string): Promise<CaeDocument> {
        this.assertActive();
        assertPath(path);
        const document = new CaeDocument(service.open(path));
        this.documents.add(document);
        return document;
    }

    /** Closes every document the session still owns. Idempotent. */
    async dispose(): Promise<void> {
        if (this.closed) {
            return;
        }
        this.closed = true;
        for (const document of [...this.documents]) {
            document.close();
        }
        this.documents.clear();
    }

    private assertActive(): void {
        if (this.closed) {
            throw new OpenCAEError('session/disposed', 'the OpenCAE session has been disposed; create a new instance');
        }
    }
}

function assertPath(path: string): void {
    if (typeof path !== 'string' || path.length === 0) {
        throw new OpenCAEError('session/argument-invalid', '`path` must be a non-empty string', {
            details: { feature: 'session.path' },
        });
    }
}

function assertUnits(units: Units | undefined): void {
    if (units !== undefined && units !== 'mm' && units !== 'm') {
        throw new OpenCAEError('session/argument-invalid', '`units` must be `mm` or `m`', {
            details: { feature: 'create.units' },
        });
    }
}

function assertDimension(dimension: Dimension | undefined): void {
    if (dimension !== undefined && dimension !== 2 && dimension !== 3) {
        throw new OpenCAEError('session/argument-invalid', '`dimension` must be 2 or 3', {
            details: { feature: 'create.dimension' },
        });
    }
}
