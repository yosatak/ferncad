/**
 * WASM ブリッジレイヤー
 *
 * ferncad-wasm の初期化と呼び出しをラップする。
 */

import init, {
  evaluate as wasmEvaluate,
  export_stl as wasmExportStl,
  check_syntax as wasmCheckSyntax,
} from '../pkg/ferncad_wasm.js';

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

/** ソースコードをSTLバイナリとしてエクスポートする */
export function exportStl(source: string): Uint8Array | { error: string } {
  try {
    return wasmExportStl(source);
  } catch (e) {
    return { error: String(e) };
  }
}

/** 構文チェックのみ行う */
export function checkSyntax(source: string): string {
  return wasmCheckSyntax(source);
}
