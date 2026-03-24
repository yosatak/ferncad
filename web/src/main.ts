/**
 * ferncad web application entry point
 *
 * Connects the editor, 3D viewer, and WASM engine.
 * Supports both single shapes and multi-part assemblies.
 */

import { createEditor, getCode } from './editor';
import { Viewer } from './viewer';
import { initWasm, evaluateParts, exportStl, type PartsResult } from './wasm-bridge';

/** Update the status bar */
function setStatus(message: string, type: 'info' | 'error' | 'success' = 'info'): void {
  const statusBar = document.getElementById('status-bar')!;
  statusBar.textContent = message;
  statusBar.className = type;
}

/** Update the assembly tree panel */
function updateAssemblyTree(parts: PartsResult['parts']): void {
  const tree = document.getElementById('assembly-tree')!;
  if (parts.length <= 1 && parts[0]?.name === 'shape') {
    tree.innerHTML = '<div class="tree-item">Single shape</div>';
    return;
  }
  tree.innerHTML = parts.map((p) => {
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

async function main(): Promise<void> {
  setStatus('Initializing WASM...');

  try {
    await initWasm();
    setStatus('Ready', 'success');
  } catch (e) {
    setStatus(`WASM init error: ${e}`, 'error');
    return;
  }

  const canvas = document.getElementById('viewer-canvas') as HTMLCanvasElement;
  const viewer = new Viewer(canvas);

  function runEvaluation(code: string): void {
    if (!code.trim()) {
      viewer.clearMesh();
      setStatus('Empty code', 'info');
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
      setStatus('No shape generated', 'info');
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
        ? `${partCount} parts, ${totalTris} triangles`
        : `${totalTris} triangles`,
      'success',
    );
  }

  const editorContainer = document.getElementById('editor-container')!;
  const editor = createEditor(editorContainer, runEvaluation);
  runEvaluation(getCode(editor));

  document.getElementById('btn-evaluate')!.addEventListener('click', () => {
    runEvaluation(getCode(editor));
  });

  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === 'Enter') {
      e.preventDefault();
      runEvaluation(getCode(editor));
    }
  });

  document.getElementById('btn-export-stl')!.addEventListener('click', () => {
    const result = exportStl(getCode(editor));
    if ('error' in result) {
      setStatus(`STL error: ${result.error}`, 'error');
      return;
    }
    downloadBlob(result as BlobPart, 'ferncad-export.stl', 'application/octet-stream');
    setStatus('STL downloaded', 'success');
  });
}

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
