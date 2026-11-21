/**
 * Native addon discovery.
 *
 * The addon only maps values: the document, persistence, validation, solving
 * and export live in the shared Rust `cae-model` crate. An absent addon is a
 * declarative state, so calls report `capability/not-implemented` instead of
 * inventing a result.
 */

import { createRequire } from 'node:module';

const requireNative = createRequire(import.meta.url);

/** Orthogonal exports of the built addon. Every call returns a JSON string. */
export interface NativeBinding {
    version(): string;
    capabilities(): string;
    create(path: string, optionsJson: string): string;
    open(path: string): string;
    save(path: string, target?: string): string;
    node_add(path: string, requestJson: string): string;
    element_add(path: string, requestJson: string): string;
    material_set(path: string, requestJson: string): string;
    support_add(path: string, requestJson: string): string;
    load_add(path: string, requestJson: string): string;
    gravity_set(path: string, requestJson: string): string;
    validate(path: string): string;
    solve(path: string, requestJson?: string): string;
    internal_forces(path: string, requestJson?: string): string;
    export(path: string, format: string, target: string): string;
    report(path: string): string;
}

let cached: NativeBinding | null | undefined;

/** Loads the addon once, or returns `undefined` when it is not installed. */
export function nativeBinding(): NativeBinding | undefined {
    if (cached === undefined) {
        cached = load();
    }
    return cached ?? undefined;
}

function load(): NativeBinding | null {
    for (const candidate of candidates()) {
        try {
            return requireNative(candidate) as NativeBinding;
        } catch (error) {
            if (candidate === explicitPath()) {
                throw error;
            }
            if (!isModuleNotFound(error, candidate)) {
                throw error;
            }
        }
    }
    return null;
}

function candidates(): string[] {
    const explicit = explicitPath();
    if (explicit) {
        return [explicit];
    }
    return [`@wae/open-cae-${process.platform}-${process.arch}`];
}

function explicitPath(): string | undefined {
    const value = process.env.OPEN_CAE_NATIVE;
    return value !== undefined && value.length > 0 ? value : undefined;
}

function isModuleNotFound(error: unknown, candidate: string): boolean {
    if (typeof error !== 'object' || error === null) {
        return false;
    }
    const code = (error as { code?: unknown }).code;
    // Node prints the resolved path, which uses the platform separator on
    // Windows. Normalize it so a linked platform package without a built addon
    // is still treated as "not installed" instead of a fatal error.
    const message = (error as Error).message.replace(/\\/g, '/');
    return code === 'MODULE_NOT_FOUND' && message.includes(candidate);
}
