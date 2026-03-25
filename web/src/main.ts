/**
 * ferncad web application entry point
 *
 * Connects the editor, 3D viewer, and WASM engine.
 * Supports both single shapes and multi-part assemblies.
 */

import { createEditor, getCode } from './editor';
import { Viewer } from './viewer';
import { initWasm, evaluateParts, exportStl, exportStep, extractParams, type PartsResult } from './wasm-bridge';
import { saveCurrentCode, loadCurrentCode, saveModel, loadModel, listModels, deleteModel } from './storage';
import { updateSliders, generateOverrides, clearOverrides } from './sliders';

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

  // Slider panel
  const paramPanel = document.getElementById('param-panel')!;

  function runWithSliders(code: string): void {
    const overridePrefix = generateOverrides(new Map());
    runEvaluation(overridePrefix + code);
  }

  function refreshSliders(code: string): void {
    const parts = extractParams(code);
    const allParams = parts.flatMap((p) => p.params);
    updateSliders(paramPanel, allParams, (overrides) => {
      const prefix = generateOverrides(overrides);
      runEvaluation(prefix + getCode(editor));
    });
  }

  // Load saved code or use default
  const savedCode = loadCurrentCode();
  const editorContainer = document.getElementById('editor-container')!;
  const editor = createEditor(editorContainer, (code) => {
    saveCurrentCode(code);
    clearOverrides();
    refreshSliders(code);
    runEvaluation(code);
  }, savedCode ?? undefined);
  const initialCode = getCode(editor);
  runEvaluation(initialCode);
  refreshSliders(initialCode);

  document.getElementById('btn-evaluate')!.addEventListener('click', () => {
    const code = getCode(editor);
    clearOverrides();
    refreshSliders(code);
    runEvaluation(code);
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

  document.getElementById('btn-export-step')!.addEventListener('click', () => {
    const result = exportStep(getCode(editor));
    if ('error' in result) {
      setStatus(`STEP error: ${result.error}`, 'error');
      return;
    }
    downloadBlob(result as BlobPart, 'ferncad-export.step', 'application/step');
    setStatus('STEP downloaded', 'success');
  });

  // Save model
  document.getElementById('btn-save')!.addEventListener('click', () => {
    const name = prompt('Model name:');
    if (!name) return;
    saveModel(name, getCode(editor));
    setStatus(`Saved: ${name}`, 'success');
  });

  // Load model
  document.getElementById('btn-load')!.addEventListener('click', () => {
    const models = listModels();
    if (models.length === 0) {
      setStatus('No saved models', 'info');
      return;
    }
    showLoadDialog(models, (name) => {
      const code = loadModel(name);
      if (code) {
        editor.dispatch({
          changes: { from: 0, to: editor.state.doc.length, insert: code },
        });
        saveCurrentCode(code);
        runEvaluation(code);
        setStatus(`Loaded: ${name}`, 'success');
      }
    });
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

/** Show a simple load dialog */
function showLoadDialog(
  models: { name: string; timestamp: number }[],
  onSelect: (name: string) => void,
): void {
  // Remove existing dialog
  document.getElementById('load-dialog')?.remove();

  const dialog = document.createElement('div');
  dialog.id = 'load-dialog';
  dialog.innerHTML = `
    <div class="dialog-overlay"></div>
    <div class="dialog-content">
      <h3>Load Model</h3>
      <div class="dialog-list">
        ${models.map((m) => {
          const date = new Date(m.timestamp).toLocaleString();
          return `<div class="dialog-item" data-name="${m.name}">
            <span class="dialog-name">${m.name}</span>
            <span class="dialog-date">${date}</span>
            <button class="dialog-delete" data-name="${m.name}" title="Delete">x</button>
          </div>`;
        }).join('')}
      </div>
      <button class="dialog-close">Cancel</button>
    </div>
  `;
  document.body.appendChild(dialog);

  // Item click -> load
  dialog.querySelectorAll('.dialog-item').forEach((el) => {
    el.addEventListener('click', (e) => {
      const target = e.target as HTMLElement;
      if (target.classList.contains('dialog-delete')) return;
      const name = (el as HTMLElement).dataset.name!;
      dialog.remove();
      onSelect(name);
    });
  });

  // Delete button
  dialog.querySelectorAll('.dialog-delete').forEach((el) => {
    el.addEventListener('click', (e) => {
      e.stopPropagation();
      const name = (el as HTMLElement).dataset.name!;
      if (confirm(`Delete "${name}"?`)) {
        deleteModel(name);
        dialog.remove();
      }
    });
  });

  // Close button / overlay click
  dialog.querySelector('.dialog-close')!.addEventListener('click', () => dialog.remove());
  dialog.querySelector('.dialog-overlay')!.addEventListener('click', () => dialog.remove());
}

main();
