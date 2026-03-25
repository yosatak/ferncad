/**
 * ferncad Web アプリケーション エントリポイント
 *
 * エディタ、ビューア、WASM を接続する。
 * アセンブリ時はパーツ別メッシュ + ツリー表示に対応。
 */

import { createEditor, getCode } from './editor';
import { Viewer } from './viewer';
import { initWasm, evaluateParts, exportStl, exportStep, type PartsResult } from './wasm-bridge';

/** ステータスバーを更新する */
function setStatus(message: string, type: 'info' | 'error' | 'success' = 'info'): void {
  const statusBar = document.getElementById('status-bar')!;
  statusBar.textContent = message;
  statusBar.className = type;
}

/** アセンブリツリーを更新する */
function updateAssemblyTree(parts: PartsResult['parts']): void {
  const tree = document.getElementById('assembly-tree')!;
  if (parts.length <= 1 && parts[0]?.name === 'shape') {
    tree.innerHTML = '<div class="tree-item">単一形状</div>';
    return;
  }
  tree.innerHTML = parts.map((p, i) => {
    const r = Math.round(p.color[0] * 255);
    const g = Math.round(p.color[1] * 255);
    const b = Math.round(p.color[2] * 255);
    const tris = p.positions.length / 9;
    return `<div class="tree-item">
      <span class="tree-color" style="background:rgb(${r},${g},${b})"></span>
      <span class="tree-name">${p.name}</span>
      <span class="tree-info">${tris} tri</span>
    </div>`;
  }).join('');
}

/** メインの初期化処理 */
async function main(): Promise<void> {
  setStatus('WASM を初期化中...');

  try {
    await initWasm();
    setStatus('準備完了', 'success');
  } catch (e) {
    setStatus(`WASM 初期化エラー: ${e}`, 'error');
    return;
  }

  const canvas = document.getElementById('viewer-canvas') as HTMLCanvasElement;
  const viewer = new Viewer(canvas);

  function runEvaluation(code: string): void {
    if (!code.trim()) {
      viewer.clearMesh();
      setStatus('コードが空です', 'info');
      updateAssemblyTree([]);
      return;
    }

    const result = evaluateParts(code);

    if ('error' in result) {
      setStatus(result.error, 'error');
      viewer.clearMesh();
      updateAssemblyTree([]);
      return;
    }

    const { parts } = result;
    if (parts.length === 0 || parts.every(p => p.positions.length === 0)) {
      setStatus('形状が生成されませんでした', 'info');
      viewer.clearMesh();
      updateAssemblyTree([]);
      return;
    }

    viewer.updateParts(parts);
    updateAssemblyTree(parts);

    const totalTris = parts.reduce((sum, p) => sum + p.positions.length / 9, 0);
    const partCount = parts.length;
    setStatus(
      partCount > 1
        ? `${partCount} パーツ, ${totalTris} 三角形を描画`
        : `${totalTris} 三角形を描画`,
      'success',
    );
  }

  const editorContainer = document.getElementById('editor-container')!;
  const editor = createEditor(editorContainer, runEvaluation);
  runEvaluation(getCode(editor));

  // 評価ボタン
  document.getElementById('btn-evaluate')!.addEventListener('click', () => {
    runEvaluation(getCode(editor));
  });

  // Ctrl+Enter
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === 'Enter') {
      e.preventDefault();
      runEvaluation(getCode(editor));
    }
  });

  // STL エクスポート
  document.getElementById('btn-export-stl')!.addEventListener('click', () => {
    const result = exportStl(getCode(editor));
    if ('error' in result) {
      setStatus(`STL エラー: ${result.error}`, 'error');
      return;
    }
    downloadBlob(result as BlobPart, 'ferncad-export.stl', 'application/octet-stream');
    setStatus('STL をダウンロードしました', 'success');
  });

  // STEP エクスポート
  document.getElementById('btn-export-step')!.addEventListener('click', () => {
    const result = exportStep(getCode(editor));
    if ('error' in result) {
      setStatus(`STEP エラー: ${result.error}`, 'error');
      return;
    }
    downloadBlob(result as BlobPart, 'ferncad-export.step', 'application/octet-stream');
    setStatus('STEP をダウンロードしました', 'success');
  });
}

/** Blob をダウンロードする */
function downloadBlob(data: BlobPart, filename: string, mimeType: string): void {
  const blob = new Blob([data], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

main();
