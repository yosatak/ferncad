/**
 * WASM ブリッジレイヤー
 *
 * ferncad-wasm の初期化と呼び出しをラップする。
 */

import init, {
  evaluate as wasmEvaluate,
  evaluate_parts as wasmEvaluateParts,
  export_stl as wasmExportStl,
  export_step as wasmExportStep,
  check_syntax as wasmCheckSyntax,
} from '../pkg/ferncad_wasm.js';

import type { PartMeshData } from './viewer';

let initialized = false;

/** WASM モジュールを初期化する */
export async function initWasm(): Promise<void> {
  if (initialized) return;
  await init();
  initialized = true;
}

/** 評価結果のメッシュデータ */
export interface MeshData {
  positions: Float32Array;
  normals: Float32Array;
}

/** ソースコードを評価してメッシュデータを取得する */
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

/** パーツ別メッシュデータ */
export interface PartsResult {
  parts: PartMeshData[];
}

/** ソースコードを評価してパーツ別メッシュデータを取得する */
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

/** ソースコードをSTLバイナリとしてエクスポートする */
export function exportStl(source: string): Uint8Array | { error: string } {
  try {
    return wasmExportStl(source);
  } catch (e) {
    return { error: String(e) };
  }
}

/** ソースコードをSTEPバイナリとしてエクスポートする */
export function exportStep(source: string): Uint8Array | { error: string } {
  try {
    return wasmExportStep(source);
  } catch (e) {
    return { error: String(e) };
  }
}

/** 構文チェックのみ行う */
export function checkSyntax(source: string): string {
  return wasmCheckSyntax(source);
}
