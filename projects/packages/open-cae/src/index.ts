/**
 * Public surface of `@wae/open-cae`.
 *
 * The Rust session (`cae-model`'s `OpenCae`) is the core: it owns document
 * editing, validation, solving and CSV/VTK export. This package is the typed
 * wrapper and the CLI is a thin assembly over it. `capabilities()` reports what
 * the installed build really supports.
 */

export { CaeDocument } from './document.js';
export { OpenCAE } from './open-cae.js';
export { NotImplementedError, OpenCAEError, errorCodes, isOpenCAEError } from './errors.js';
export type { OpenCAEErrorCode, OpenCAEErrorDetails, OpenCAEErrorOptions } from './errors.js';
export type * from './types.js';
