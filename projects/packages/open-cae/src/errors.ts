/**
 * Stable error surface of `@wae/open-cae`.
 *
 * Codes are contract, not prose: callers compare them, localize around them and
 * never parse the message. A failure therefore keeps its code even when the
 * text is translated.
 */

/** Every stable code this package may report. */
export const errorCodes = [
    'session/disposed',
    'session/argument-invalid',
    'session/no-document',
    'native/unavailable',
    'native/error',
    'file/exists',
    'io/read',
    'io/write',
    'document/parse',
    'document/serialize',
    'document/format',
    'document/version',
    'document/dimension',
    'document/dimension-mismatch',
    'document/assumed-units',
    'document/closed',
    'material/invalid',
    'material/invalid-section',
    'section/invalid-dimensions',
    'material/invalid-alpha',
    'material/missing-alpha',
    'material/invalid-conductivity',
    'heat/no-fixed-temperature',
    'heat/invalid-convection',
    'heat/invalid-perimeter',
    'heat/missing-perimeter',
    'heat/missing-ambient',
    'heat/invalid-ambient',
    'model/no-nodes',
    'model/no-elements',
    'model/mechanism',
    'analysis/invalid-modes',
    'analysis/invalid-stations',
    'analysis/invalid-points',
    'analysis/invalid-axis',
    'analysis/invalid-spectrum',
    'spectrum/invalid-combination',
    'spectrum/invalid-damping',
    'buckling/non-finite',
    'combination/invalid-name',
    'combination/invalid-factor',
    'combination/duplicate-name',
    'combination/duplicate-pattern',
    'combination/unknown-pattern',
    'combination/none',
    'combination/unknown-name',
    'pattern/invalid-name',
    'pattern/duplicate-name',
    'pattern/unknown-pattern',
    'node/duplicate-id',
    'node/unknown-id',
    'node/invalid-coordinate',
    'node/invalid-temperature',
    'node/referenced-by-element',
    'node/referenced-by-support',
    'node/referenced-by-load',
    'element/duplicate-id',
    'element/duplicate',
    'element/degenerate',
    'element/unknown-node',
    'element/zero-length',
    'element/distributed-load-requires-beam',
    'element/invalid-distributed-load',
    'element/invalid-temperature',
    'element/invalid-release',
    'element/release-requires-beam',
    'element/invalid-fraction',
    'element/split-unsupported-load',
    'element/unknown-id',
    'support/unknown-node',
    'support/unknown-support',
    'support/no-freedom',
    'load/unknown-node',
    'load/unknown-load',
    'load/invalid-heat',
    'result/unknown-node',
    'result/unknown-element',
    'result/no-reaction',
    'export/mismatch',
    'export/invalid-model',
    'export/no-solution',
    'export/invalid-scale',
    'cli/unknown-command',
    'cli/missing-argument',
    'cli/missing-option',
    'cli/missing-value',
    'cli/unknown-option',
    'cli/invalid-number',
    'cli/invalid-option',
    'cli/invalid-units',
    'cli/unknown-format',
    'cli/dimension-mismatch',
    'cli/direction-conflict',
    'cli/invalid-spectrum',
    'influence/loads-present',
    'influence/track-not-beam',
    'influence/invalid-position',
    'influence/invalid-component',
    'influence/empty-train',
    'influence/spacing-mismatch',
    'influence/negative-spacing',
    'influence/invalid-axle-load',
    'influence/invalid-step',
    'capability/not-implemented',
] as const;

/** Union of {@link errorCodes}. */
export type OpenCAEErrorCode = (typeof errorCodes)[number];

/** Optional locators a failure can carry so callers can point at the document. */
export interface OpenCAEErrorDetails {
    /** Capability or field involved in the failure, e.g. `create.units`. */
    readonly feature?: string;
    /** Document path involved in the failure. */
    readonly path?: string;
}

/** Options accepted by {@link OpenCAEError}. */
export interface OpenCAEErrorOptions {
    /** Whether retrying with different input or state can succeed. */
    readonly recoverable?: boolean;
    readonly details?: OpenCAEErrorDetails;
    readonly cause?: unknown;
}

/** Every failure thrown or rejected by this package. */
export class OpenCAEError extends Error {
    /** Stable, comparable code. */
    readonly code: OpenCAEErrorCode;
    /** Whether retrying with different input or state can succeed. */
    readonly recoverable: boolean;
    /** Optional locators (`feature`, `path`). */
    readonly details: OpenCAEErrorDetails;

    constructor(code: OpenCAEErrorCode, message: string, options: OpenCAEErrorOptions = {}) {
        super(message, options.cause === undefined ? undefined : { cause: options.cause });
        this.name = 'OpenCAEError';
        this.code = code;
        this.recoverable = options.recoverable ?? false;
        this.details = Object.freeze({ ...options.details });
    }
}

/**
 * Reported when the optional native addon is absent or a declared route is not
 * implemented by the installed build.
 */
export class NotImplementedError extends OpenCAEError {
    constructor(feature: string, message = `\`${feature}\` is not implemented yet`) {
        super('capability/not-implemented', message, { recoverable: false, details: { feature } });
        this.name = 'NotImplementedError';
    }
}

/** Narrows an unknown thrown value to {@link OpenCAEError}. */
export function isOpenCAEError(value: unknown): value is OpenCAEError {
    return value instanceof OpenCAEError;
}
