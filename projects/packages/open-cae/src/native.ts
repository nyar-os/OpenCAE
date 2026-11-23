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
    node_update(path: string, requestJson: string): string;
    node_mass(path: string, requestJson: string): string;
    node_temperature(path: string, requestJson: string): string;
    node_heat(path: string, requestJson: string): string;
    node_merge(path: string, requestJson: string): string;
    node_remove(path: string, requestJson: string): string;
    element_add(path: string, requestJson: string): string;
    element_update(path: string, requestJson: string): string;
    element_split(path: string, requestJson: string): string;
    element_remove(path: string, requestJson: string): string;
    element_release(path: string, requestJson: string): string;
    element_tension_only(path: string, requestJson: string): string;
    element_gradient(path: string, requestJson: string): string;
    element_span(path: string, requestJson: string): string;
    tension_only(path: string, requestJson?: string): string;
    material_set(path: string, requestJson: string): string;
    material_preset(path: string, requestJson: string): string;
    library(requestJson?: string): string;
    section_properties(requestJson: string): string;
    section_polygon(requestJson: string): string;
    section_i(requestJson: string): string;
    section_tube(requestJson: string): string;
    section_channel(requestJson: string): string;
    section_angle(requestJson: string): string;
    section_t(requestJson: string): string;
    section_ellipse(requestJson: string): string;
    section_triangle(requestJson: string): string;
    section_z(requestJson: string): string;
    section_principal(requestJson: string): string;
    support_add(path: string, requestJson: string): string;
    support_remove(path: string, requestJson: string): string;
    load_add(path: string, requestJson: string): string;
    load_remove(path: string, requestJson: string): string;
    gravity_set(path: string, requestJson: string): string;
    ambient_temperature_set(path: string, requestJson: string): string;
    combination_add(path: string, requestJson: string): string;
    combination_remove(path: string, requestJson: string): string;
    pattern_add(path: string, requestJson: string): string;
    pattern_remove(path: string, requestJson: string): string;
    solve_combinations(path: string, requestJson?: string): string;
    solve_buckling_combinations(path: string, requestJson?: string): string;
    validate(path: string): string;
    solve(path: string, requestJson?: string): string;
    heat(path: string, requestJson?: string): string;
    reactions(path: string, requestJson?: string): string;
    node_displacement(path: string, requestJson: string): string;
    element_result(path: string, requestJson: string): string;
    node_reaction(path: string, requestJson: string): string;
    internal_forces(path: string, requestJson?: string): string;
    influence_line(path: string, requestJson: string): string;
    influence_line_train(path: string, requestJson: string): string;
    stresses(path: string, requestJson?: string): string;
    deflections(path: string, requestJson?: string): string;
    nodal_stresses(path: string, requestJson?: string): string;
    load_cases(path: string, requestJson?: string): string;
    matrices(path: string, requestJson?: string): string;
    response_spectrum(path: string, requestJson: string): string;
    frequency_response(path: string, requestJson: string): string;
    construction_stages(path: string, requestJson: string): string;
    geometric_nonlinear(path: string, requestJson?: string): string;
    export(path: string, format: string, target: string, optionsJson?: string): string;
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
