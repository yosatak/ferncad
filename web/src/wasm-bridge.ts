/**
 * WASM bridge layer
 *
 * Wraps ferncad-wasm initialization and calls.
 */

import init, {
  evaluate as wasmEvaluate,
  evaluate_parts as wasmEvaluateParts,
  export_stl as wasmExportStl,
  export_step as wasmExportStep,
  check_syntax as wasmCheckSyntax,
  extract_params as wasmExtractParams,
  get_completions as wasmGetCompletions,
  evaluate_parts_with_files as wasmEvaluatePartsWithFiles,
  check_syntax_with_files as wasmCheckSyntaxWithFiles,
  export_stl_with_files as wasmExportStlWithFiles,
  export_step_with_files as wasmExportStepWithFiles,
  extract_params_with_files as wasmExtractParamsWithFiles,
  get_completions_with_files as wasmGetCompletionsWithFiles,
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
      spanStart: number;
      spanEnd: number;
      positions: number[];
      normals: number[];
    }) => ({
      name: p.name,
      color: p.color as [number, number, number],
      spanStart: p.spanStart ?? 0,
      spanEnd: p.spanEnd ?? 0,
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

/** Export source code as STEP (exact BREP geometry) */
export function exportStep(source: string): Uint8Array | { error: string } {
  try {
    return wasmExportStep(source);
  } catch (e) {
    return { error: String(e) };
  }
}

/** Part parameter spec */
export interface ParamSpec {
  name: string;
  type: string;
  default: number;
  doc: string;
}

/** Part with parameters */
export interface PartSpec {
  name: string;
  params: ParamSpec[];
}

/** Extract defpart parameter specs from source */
export function extractParams(source: string): PartSpec[] {
  try {
    const json = wasmExtractParams(source);
    const data = JSON.parse(json);
    return data.parts ?? [];
  } catch {
    return [];
  }
}

/** Diagnostic entry from syntax check */
export interface DiagnosticEntry {
  line: number;
  col: number;
  message: string;
  severity: 'error' | 'warning';
}

/** Check syntax and return diagnostics */
export function checkSyntax(source: string): DiagnosticEntry[] {
  try {
    const json = wasmCheckSyntax(source);
    const data = JSON.parse(json);
    return data.diagnostics ?? [];
  } catch {
    return [];
  }
}

/** Completion item from the Rust completion engine */
export interface CompletionItemData {
  label: string;
  kind: string;
  detail: string;
  doc: string;
  example: string;
  boost: number;
}

/** Get completions for the given source at cursor position */
export function getCompletions(source: string, cursorPos: number): CompletionItemData[] {
  try {
    const json = wasmGetCompletions(source, cursorPos);
    return JSON.parse(json);
  } catch {
    return [];
  }
}

// ── Multi-file API ──────────────────────────────────────────────────

/** Evaluate with project files context and get per-part mesh data */
export function evaluatePartsWithFiles(
  mainFile: string,
  files: Record<string, string>,
): PartsResult | { error: string } {
  try {
    const json = wasmEvaluatePartsWithFiles(mainFile, JSON.stringify(files));
    const data = JSON.parse(json);
    const parts: PartMeshData[] = data.parts.map(
      (p: {
        name: string;
        color: [number, number, number];
        spanStart: number;
        spanEnd: number;
        positions: number[];
        normals: number[];
      }) => ({
        name: p.name,
        color: p.color as [number, number, number],
        spanStart: p.spanStart ?? 0,
        spanEnd: p.spanEnd ?? 0,
        positions: new Float32Array(p.positions),
        normals: new Float32Array(p.normals),
      }),
    );
    return { parts };
  } catch (e) {
    return { error: String(e) };
  }
}

/** Check syntax with project files context */
export function checkSyntaxWithFiles(
  source: string,
  files: Record<string, string>,
): DiagnosticEntry[] {
  try {
    const json = wasmCheckSyntaxWithFiles(source, JSON.stringify(files));
    const data = JSON.parse(json);
    return data.diagnostics ?? [];
  } catch {
    return [];
  }
}

/** Export STL with project files context */
export function exportStlWithFiles(
  mainFile: string,
  files: Record<string, string>,
): Uint8Array | { error: string } {
  try {
    return wasmExportStlWithFiles(mainFile, JSON.stringify(files));
  } catch (e) {
    return { error: String(e) };
  }
}

/** Export STEP with project files context */
export function exportStepWithFiles(
  mainFile: string,
  files: Record<string, string>,
): Uint8Array | { error: string } {
  try {
    return wasmExportStepWithFiles(mainFile, JSON.stringify(files));
  } catch (e) {
    return { error: String(e) };
  }
}

/** Extract params with project files context */
export function extractParamsWithFiles(
  mainFile: string,
  files: Record<string, string>,
): PartSpec[] {
  try {
    const json = wasmExtractParamsWithFiles(mainFile, JSON.stringify(files));
    const data = JSON.parse(json);
    return data.parts ?? [];
  } catch {
    return [];
  }
}

/** Get completions with project files context */
export function getCompletionsWithFiles(
  source: string,
  cursorPos: number,
  files: Record<string, string>,
): CompletionItemData[] {
  try {
    const json = wasmGetCompletionsWithFiles(source, cursorPos, JSON.stringify(files));
    return JSON.parse(json);
  } catch {
    return [];
  }
}
