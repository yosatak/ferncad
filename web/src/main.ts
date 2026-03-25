/**
 * ferncad Web アプリケーション エントリポイント
 *
 * エディタ、ビューア、WASM を接続する。
 */

import { createEditor, getCode } from './editor';
import { Viewer } from './viewer';
import { initWasm, evaluateCode, exportStl, type MeshData } from './wasm-bridge';

/** ステータスバーを更新する */
function setStatus(message: string, type: 'info' | 'error' | 'success' = 'info'): void {
  const statusBar = document.getElementById('status-bar')!;
  statusBar.textContent = message;
  statusBar.className = type;
}

/** メインの初期化処理 */
async function main(): Promise<void> {
  setStatus('WASM を初期化中...');

  // WASM 初期化
  try {
    await initWasm();
    setStatus('準備完了', 'success');
  } catch (e) {
    setStatus(`WASM 初期化エラー: ${e}`, 'error');
    return;
  }

  // ビューア初期化
  const canvas = document.getElementById('viewer-canvas') as HTMLCanvasElement;
  const viewer = new Viewer(canvas);

  // 評価関数
  function runEvaluation(code: string): void {
    if (!code.trim()) {
      viewer.clearMesh();
      setStatus('コードが空です', 'info');
      return;
    }

    const result = evaluateCode(code);

    if ('error' in result) {
      setStatus(result.error, 'error');
      viewer.clearMesh();
      return;
    }

    const meshData = result as MeshData;
    if (meshData.positions.length === 0) {
      setStatus('形状が生成されませんでした', 'info');
      viewer.clearMesh();
      return;
    }

    const numTriangles = meshData.positions.length / 9;
    viewer.updateMesh(meshData.positions, meshData.normals);
    setStatus(
      `${numTriangles} 三角形を描画`,
      'success',
    );
  }

  // エディタ初期化（コード変更時にデバウンス評価）
  const editorContainer = document.getElementById('editor-container')!;
  const editor = createEditor(editorContainer, (code) => {
    runEvaluation(code);
  });

  // 初回評価
  runEvaluation(getCode(editor));

  // 評価ボタン
  document.getElementById('btn-evaluate')!.addEventListener('click', () => {
    runEvaluation(getCode(editor));
  });

  // Ctrl+Enter で評価
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === 'Enter') {
      e.preventDefault();
      runEvaluation(getCode(editor));
    }
  });

  // STL エクスポートボタン
  document.getElementById('btn-export-stl')!.addEventListener('click', () => {
    const code = getCode(editor);
    const result = exportStl(code);

    if ('error' in result) {
      setStatus(`STL エクスポートエラー: ${result.error}`, 'error');
      return;
    }

    const blob = new Blob([result as BlobPart], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'ferncad-export.stl';
    a.click();
    URL.revokeObjectURL(url);
    setStatus('STL をダウンロードしました', 'success');
  });
}

main();
