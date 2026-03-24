/**
 * WASM bridge layer
 *
 * Wraps ferncad-wasm initialization and calls.
 */

import init, {
  evaluate as wasmEvaluate,
  evaluate_parts as wasmEvaluateParts,
  export_stl as wasmExportStl,
  check_syntax as wasmCheckSyntax,
} from '../pkg/ferncad_wasm.js';

import type { PartMeshData } from './viewer';

let initialized = false;

/** Initialize the WASM module */
export async function initWasm(): Promise<void> {
  if (initialized) return;
  await init();
  initialized = true;
}

/** Mesh data from evaluation */
export interface MeshData {
  positions: Float32Array;
  normals: Float32Array;
}

/** Evaluate source code and get mesh data */
export function evaluateCode(source: string): MeshData | { error: string } {
  try {
    const combined: Float32Array = wasmEvaluate(source);
    const half = combined.length / 2;
    const positions = combined.slice(0, half);
    const normals = combined.slice(half);
    return { positions, normals };
  } catch (e) {
    return { error: String(e) };
  }
}

/** Per-part mesh data result */
export interface PartsResult {
  parts: PartMeshData[];
}

/** Evaluate source code and get per-part mesh data */
export function evaluateParts(source: string): PartsResult | { error: string } {
  try {
    const json = wasmEvaluateParts(source);
    const data = JSON.parse(json);
    const parts: PartMeshData[] = data.parts.map((p: {
      name: string;
      color: [number, number, number];
      positions: number[];
      normals: number[];
    }) => ({
      name: p.name,
      color: p.color as [number, number, number],
      positions: new Float32Array(p.positions),
      normals: new Float32Array(p.normals),
    }));
    return { parts };
  } catch (e) {
    return { error: String(e) };
  }
}

/** Export source code as binary STL */
export function exportStl(source: string): Uint8Array | { error: string } {
  try {
    return wasmExportStl(source);
  } catch (e) {
    return { error: String(e) };
  }
}

/** Check syntax only */
export function checkSyntax(source: string): string {
  return wasmCheckSyntax(source);
}
