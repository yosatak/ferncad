/**
 * ferncad web application entry point
 *
 * Connects the editor, 3D viewer, WASM engine, file explorer, and project storage.
 * Supports multi-file projects with `require` for user-defined modules.
 */

import {
  createEditor,
  getCode,
  highlightSpan,
  clearHighlightSpan,
  getCursorOffset,
  setEditorContent,
  getEditorState,
  setEditorState,
} from './editor';
import { Viewer } from './viewer';
import type { EditorView } from 'codemirror';
import {
  initWasm,
  evaluatePartsWithFiles,
  exportStlWithFiles,
  exportStepWithFiles,
  extractParamsWithFiles,
  checkSyntaxWithFiles,
  getCompletionsWithFiles,
  type PartsResult,
} from './wasm-bridge';
import { updateSliders, generateOverrides, clearOverrides } from './sliders';
import { VirtualFileSystem } from './vfs';
import { FileExplorer } from './file-explorer';
import { TabBar } from './tabs';
import {
  initProjectDB,
  migrateFromLocalStorage,
  saveProject,
  loadProject,
  listProjects,
  deleteProject,
  saveSession,
  loadSession,
  createNewProject,
  type Project,
} from './project-storage';
import { ChatPanel } from './chat-panel';
import { LLMBridge, MODEL_OPTIONS, DEFAULT_MODEL } from './llm-bridge';
import { buildMessages, extractCode, type ChatEntry } from './fern-prompt';

// ── Global state ────────────────────────────────────────────────────

const vfs = new VirtualFileSystem();
let currentProject: Project | null = null;
let activeFile = 'main.fern';
let editorRef: EditorView | null = null;
let tabBar: TabBar;
let fileExplorer: FileExplorer;
let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;

// ── LLM state ──────────────────────────────────────────────────────

const llmBridge = new LLMBridge();
let chatPanel: ChatPanel;
let chatHistory: ChatEntry[] = [];
let llmModelId = DEFAULT_MODEL;
let llmInitialized = false;
let currentRequestId: string | null = null;

// ── Helpers ─────────────────────────────────────────────────────────

function setStatus(message: string, type: 'info' | 'error' | 'success' = 'info'): void {
  const statusBar = document.getElementById('status-bar')!;
  statusBar.textContent = message;
  statusBar.className = type;
}

function updateAssemblyTree(parts: PartsResult['parts']): void {
  const tree = document.getElementById('assembly-tree')!;
  if (parts.length <= 1 && parts[0]?.name === 'shape') {
    tree.innerHTML = '<div class="tree-item">Single shape</div>';
    return;
  }
  tree.innerHTML = parts
    .map((p) => {
      const r = Math.round(p.color[0] * 255);
      const g = Math.round(p.color[1] * 255);
      const b = Math.round(p.color[2] * 255);
      const tris = p.positions.length / 9;
      return `<div class="tree-item">
      <span class="tree-color" style="background:rgb(${r},${g},${b})"></span>
      <span class="tree-name">${p.name}</span>
      <span class="tree-info">${tris} tri</span>
    </div>`;
    })
    .join('');
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

/** Schedule auto-save of current project to IndexedDB (debounced 1s) */
function scheduleAutoSave(): void {
  if (autoSaveTimer) clearTimeout(autoSaveTimer);
  autoSaveTimer = setTimeout(async () => {
    if (!currentProject) return;
    // Sync VFS content to project
    currentProject.files = vfs.listFiles().map((name) => ({
      name,
      content: vfs.getFile(name) ?? '',
    }));
    currentProject.activeFile = activeFile;
    try {
      await saveProject(currentProject);
      await saveSession(currentProject.id);
    } catch {
      // Silently ignore save errors
    }
  }, 1000);
}

// ── Evaluation ──────────────────────────────────────────────────────

function runEvaluation(): void {
  const mainSource = vfs.getFile('main.fern') ?? '';
  if (!mainSource.trim()) {
    viewer.clearMesh();
    setStatus('Empty code', 'info');
    updateAssemblyTree([]);
    return;
  }

  const files = vfs.toFileMap();
  const result = evaluatePartsWithFiles('main.fern', files);

  if ('error' in result) {
    setStatus(result.error, 'error');
    viewer.clearMesh();
    updateAssemblyTree([]);
    return;
  }

  const { parts } = result;
  if (parts.length === 0 || parts.every((p) => p.positions.length === 0)) {
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
    partCount > 1 ? `${partCount} parts, ${totalTris} triangles` : `${totalTris} triangles`,
    'success',
  );
}

// ── Viewer (created early) ──────────────────────────────────────────

const canvas = document.getElementById('viewer-canvas') as HTMLCanvasElement;
const viewer = new Viewer(canvas);

// ── File switching ──────────────────────────────────────────────────

function switchToFile(filename: string): void {
  if (!editorRef) return;
  if (filename === activeFile) return;
  if (!vfs.hasFile(filename)) return;

  // Save current editor state to tab
  tabBar.saveEditorState(activeFile, getEditorState(editorRef));

  activeFile = filename;

  // Restore or load content
  const savedState = tabBar.getEditorState(filename);
  if (savedState) {
    setEditorState(editorRef, savedState);
  } else {
    setEditorContent(editorRef, vfs.getFile(filename) ?? '');
  }

  tabBar.openTab(filename);
  tabBar.setActiveTab(filename);
  fileExplorer.setActiveFile(filename);
}

// ── Main ────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  setStatus('Initializing...');

  try {
    await Promise.all([initWasm(), initProjectDB()]);
  } catch (e) {
    setStatus(`Init error: ${e}`, 'error');
    return;
  }

  // Migrate from localStorage if needed
  const migratedId = await migrateFromLocalStorage();

  // Load session or create new project
  const sessionProjectId = migratedId ?? (await loadSession());
  if (sessionProjectId) {
    currentProject = await loadProject(sessionProjectId);
  }
  if (!currentProject) {
    currentProject = createNewProject('Untitled');
    await saveProject(currentProject);
    await saveSession(currentProject.id);
  }

  // Populate VFS from project
  vfs.loadFiles(currentProject.files);
  activeFile = currentProject.activeFile || 'main.fern';
  if (!vfs.hasFile(activeFile)) {
    activeFile = vfs.listFiles()[0] ?? 'main.fern';
  }

  // ── Tab bar ─────────────────────────────────────────────────────
  const tabBarContainer = document.getElementById('tab-bar')!;
  tabBar = new TabBar({
    container: tabBarContainer,
    onTabSelect: (filename) => switchToFile(filename),
    onTabClose: (filename) => {
      tabBar.closeTab(filename);
      // If all tabs closed, open main.fern
      if (tabBar.getOpenTabs().length === 0) {
        tabBar.openTab('main.fern');
        switchToFile('main.fern');
      }
    },
  });

  // ── File explorer ───────────────────────────────────────────────
  const explorerContainer = document.getElementById('file-explorer')!;
  fileExplorer = new FileExplorer({
    container: explorerContainer,
    vfs,
    onFileSelect: (filename) => {
      tabBar.openTab(filename);
      switchToFile(filename);
    },
    onFileCreate: (filename) => {
      vfs.setFile(filename, '');
      tabBar.openTab(filename);
      switchToFile(filename);
      scheduleAutoSave();
    },
    onFileDelete: (filename) => {
      tabBar.closeTab(filename);
      vfs.deleteFile(filename);
      // Switch to another file if we deleted the active one
      if (activeFile === filename) {
        const remaining = vfs.listFiles();
        const next = remaining[0] ?? 'main.fern';
        tabBar.openTab(next);
        switchToFile(next);
      }
      scheduleAutoSave();
    },
    onFileRename: (oldName, newName) => {
      vfs.renameFile(oldName, newName);
      tabBar.renameTab(oldName, newName);
      if (activeFile === oldName) {
        activeFile = newName;
      }
      fileExplorer.setActiveFile(activeFile);
      scheduleAutoSave();
    },
  });
  fileExplorer.setActiveFile(activeFile);
  fileExplorer.render();

  // ── Editor → Viewer cursor sync (debounced) ────────────────────
  let cursorDebounce: ReturnType<typeof setTimeout> | null = null;
  function onCursorChange(): void {
    if (!editorRef) return;
    if (cursorDebounce) clearTimeout(cursorDebounce);
    cursorDebounce = setTimeout(() => {
      if (!editorRef) return;
      const offset = getCursorOffset(editorRef);
      const partName = viewer.findPartByOffset(offset);
      if (partName) {
        viewer.setHighlight(partName);
        const span = viewer.getPartSpan(partName);
        if (span) {
          highlightSpan(editorRef, span.start, span.end);
        }
      } else {
        viewer.clearHighlight();
        clearHighlightSpan(editorRef);
      }
    }, 80);
  }

  // Viewer → Editor: clicking a shape highlights the source code
  viewer.onPartClick = (span) => {
    if (editorRef && span.end > 0) {
      highlightSpan(editorRef, span.start, span.end);
    }
  };

  // ── Create editor ──────────────────────────────────────────────
  const editorContainer = document.getElementById('editor-container')!;
  const editor = createEditor(
    editorContainer,
    (code) => {
      // Save to VFS on every change
      vfs.setFile(activeFile, code);
      clearOverrides();
      refreshSliders();
      runEvaluation();
      scheduleAutoSave();
    },
    vfs.getFile(activeFile) ?? '',
    onCursorChange,
  );
  editorRef = editor;

  // Open initial tab
  tabBar.openTab(activeFile);
  tabBar.setActiveTab(activeFile);

  // Slider panel
  const paramPanel = document.getElementById('param-panel')!;

  function refreshSliders(): void {
    const files = vfs.toFileMap();
    const parts = extractParamsWithFiles('main.fern', files);
    const allParams = parts.flatMap((p) => p.params);
    updateSliders(paramPanel, allParams, (overrides) => {
      const prefix = generateOverrides(overrides);
      // Temporarily prepend overrides to main.fern content for evaluation
      const mainSource = vfs.getFile('main.fern') ?? '';
      const files = vfs.toFileMap();
      files['main.fern'] = prefix + mainSource;
      const result = evaluatePartsWithFiles('main.fern', files);
      if (!('error' in result)) {
        viewer.updateParts(result.parts);
        updateAssemblyTree(result.parts);
      }
    });
  }

  // Initial evaluation
  runEvaluation();
  refreshSliders();
  setStatus('Ready', 'success');

  // ── Toolbar buttons ────────────────────────────────────────────

  document.getElementById('btn-evaluate')!.addEventListener('click', () => {
    clearOverrides();
    refreshSliders();
    runEvaluation();
  });

  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === 'Enter') {
      e.preventDefault();
      runEvaluation();
    }
  });

  document.getElementById('btn-export-stl')!.addEventListener('click', () => {
    const files = vfs.toFileMap();
    const result = exportStlWithFiles('main.fern', files);
    if ('error' in result) {
      setStatus(`STL error: ${result.error}`, 'error');
      return;
    }
    downloadBlob(result as BlobPart, 'ferncad-export.stl', 'application/octet-stream');
    setStatus('STL downloaded', 'success');
  });

  document.getElementById('btn-export-step')!.addEventListener('click', () => {
    const files = vfs.toFileMap();
    const result = exportStepWithFiles('main.fern', files);
    if ('error' in result) {
      setStatus(`STEP error: ${result.error}`, 'error');
      return;
    }
    downloadBlob(result as BlobPart, 'ferncad-export.step', 'application/step');
    setStatus('STEP downloaded', 'success');
  });

  // New project
  document.getElementById('btn-new-project')!.addEventListener('click', async () => {
    const name = prompt('Project name:', 'Untitled');
    if (!name) return;
    currentProject = createNewProject(name);
    await saveProject(currentProject);
    await saveSession(currentProject.id);
    vfs.loadFiles(currentProject.files);
    activeFile = 'main.fern';
    setEditorContent(editor, vfs.getFile(activeFile) ?? '');
    // Reset tabs
    for (const tab of tabBar.getOpenTabs()) {
      tabBar.closeTab(tab);
    }
    tabBar.openTab(activeFile);
    tabBar.setActiveTab(activeFile);
    fileExplorer.setActiveFile(activeFile);
    fileExplorer.render();
    runEvaluation();
    refreshSliders();
    setStatus(`New project: ${name}`, 'success');
  });

  // Save project
  document.getElementById('btn-save')!.addEventListener('click', async () => {
    if (!currentProject) return;
    const name = prompt('Project name:', currentProject.name);
    if (!name) return;
    currentProject.name = name;
    currentProject.files = vfs.listFiles().map((n) => ({
      name: n,
      content: vfs.getFile(n) ?? '',
    }));
    currentProject.activeFile = activeFile;
    await saveProject(currentProject);
    await saveSession(currentProject.id);
    setStatus(`Saved: ${name}`, 'success');
  });

  // ── Resize handle (editor ↔ viewer) ─────────────────────────────
  {
    const resizeHandle = document.getElementById('resize-handle')!;
    const editorPanel = document.getElementById('editor-panel')!;
    const viewerPanel = document.getElementById('viewer-panel')!;
    const workspace = document.getElementById('workspace')!;

    let dragging = false;

    resizeHandle.addEventListener('mousedown', (e) => {
      e.preventDefault();
      dragging = true;
      resizeHandle.classList.add('dragging');
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    });

    document.addEventListener('mousemove', (e) => {
      if (!dragging) return;
      const workspaceRect = workspace.getBoundingClientRect();
      const explorerWidth = document.getElementById('file-explorer')!.getBoundingClientRect().width;
      const handleWidth = resizeHandle.getBoundingClientRect().width;
      const availableWidth = workspaceRect.width - explorerWidth - handleWidth;
      const editorWidth = e.clientX - workspaceRect.left - explorerWidth - handleWidth / 2;
      const clampedWidth = Math.max(200, Math.min(editorWidth, availableWidth - 200));

      editorPanel.style.flex = 'none';
      editorPanel.style.width = `${clampedWidth}px`;
      viewerPanel.style.flex = '1';
    });

    document.addEventListener('mouseup', () => {
      if (!dragging) return;
      dragging = false;
      resizeHandle.classList.remove('dragging');
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    });
  }

  // ── AI assistant ──────────────────────────────────────────────────

  const viewerPanel = document.getElementById('viewer-panel')!;
  chatPanel = new ChatPanel({
    container: viewerPanel,
    onSend: handleChatSend,
    onApplyCode: handleApplyCode,
    onClose: () => {
      document.getElementById('btn-ai')!.classList.remove('chat-active');
    },
    onModelChange: (modelId) => {
      llmModelId = modelId;
      llmInitialized = false;
      chatPanel.setStatus('Model changed — click Send to load');
      chatPanel.setInputEnabled(true);
    },
  });
  chatPanel.setModelOptions(
    MODEL_OPTIONS.map((m) => ({ id: m.id, label: m.label })),
    llmModelId,
  );

  document.getElementById('btn-ai')!.addEventListener('click', () => {
    chatPanel.toggle();
    document.getElementById('btn-ai')!.classList.toggle('chat-active', chatPanel.isVisible());
    if (chatPanel.isVisible() && !llmInitialized) {
      initLLM();
    }
  });

  // Ctrl+L shortcut
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === 'l') {
      e.preventDefault();
      chatPanel.toggle();
      document.getElementById('btn-ai')!.classList.toggle('chat-active', chatPanel.isVisible());
      if (chatPanel.isVisible() && !llmInitialized) {
        initLLM();
      }
    }
  });

  async function initLLM(): Promise<void> {
    if (llmInitialized) return;
    chatPanel.setStatus('Loading model...');
    chatPanel.setProgress(0, 'Downloading model...');
    chatPanel.setInputEnabled(false);

    try {
      await llmBridge.init(llmModelId, (progress, text) => {
        chatPanel.setProgress(progress, text);
      });
      llmInitialized = true;
      chatPanel.hideProgress();
      chatPanel.setStatus('Ready');
      chatPanel.setInputEnabled(true);
    } catch (e) {
      chatPanel.hideProgress();
      chatPanel.setStatus(`Load error: ${e}`);
      chatPanel.addErrorMessage(`Failed to load model: ${e}`);
    }
  }

  function handleChatSend(message: string): void {
    chatPanel.addUserMessage(message);

    // Initialize model on first send if not yet loaded
    if (!llmInitialized) {
      chatPanel.setInputEnabled(false);
      initLLM().then(() => {
        if (llmInitialized) doGenerate(message);
      });
      return;
    }

    doGenerate(message);
  }

  function doGenerate(message: string): void {
    chatPanel.setInputEnabled(false);
    chatPanel.setStatus('Generating...');

    const editorContent = vfs.getFile(activeFile) ?? '';
    const messages = buildMessages(message, editorContent, chatHistory);
    const bubbleId = chatPanel.addAssistantMessage();

    currentRequestId = llmBridge.generate(messages, {
      onToken: (token) => {
        chatPanel.appendToken(bubbleId, token);
      },
      onDone: (fullText) => {
        currentRequestId = null;
        const code = extractCode(fullText);
        chatPanel.finalizeMessage(bubbleId, code);
        chatPanel.setStatus('Ready');
        chatPanel.setInputEnabled(true);

        // Update chat history
        chatHistory.push({ role: 'user', content: message });
        chatHistory.push({ role: 'assistant', content: fullText });
        // Keep history manageable
        if (chatHistory.length > 20) {
          chatHistory = chatHistory.slice(-20);
        }
      },
      onError: (error) => {
        currentRequestId = null;
        chatPanel.addErrorMessage(`Error: ${error}`);
        chatPanel.setStatus('Ready');
        chatPanel.setInputEnabled(true);
      },
    });
  }

  function handleApplyCode(code: string): void {
    if (!editorRef) return;
    setEditorContent(editorRef, code);
    vfs.setFile(activeFile, code);
    runEvaluation();
    scheduleAutoSave();
    setStatus('Code applied from AI', 'success');
  }

  // Open project
  document.getElementById('btn-load')!.addEventListener('click', async () => {
    const projects = await listProjects();
    if (projects.length === 0) {
      setStatus('No saved projects', 'info');
      return;
    }
    showProjectDialog(projects, async (id) => {
      const project = await loadProject(id);
      if (!project) return;
      currentProject = project;
      await saveSession(project.id);
      vfs.loadFiles(project.files);
      activeFile = project.activeFile || 'main.fern';
      if (!vfs.hasFile(activeFile)) {
        activeFile = vfs.listFiles()[0] ?? 'main.fern';
      }
      setEditorContent(editor, vfs.getFile(activeFile) ?? '');
      // Reset tabs
      for (const tab of tabBar.getOpenTabs()) {
        tabBar.closeTab(tab);
      }
      tabBar.openTab(activeFile);
      tabBar.setActiveTab(activeFile);
      fileExplorer.setActiveFile(activeFile);
      fileExplorer.render();
      runEvaluation();
      refreshSliders();
      setStatus(`Opened: ${project.name}`, 'success');
    });
  });
}

/** Show a project selection dialog */
function showProjectDialog(
  projects: { id: string; name: string; timestamp: number }[],
  onSelect: (id: string) => void,
): void {
  document.getElementById('load-dialog')?.remove();

  const dialog = document.createElement('div');
  dialog.id = 'load-dialog';
  dialog.innerHTML = `
    <div class="dialog-overlay"></div>
    <div class="dialog-content">
      <h3>Open Project</h3>
      <div class="dialog-list">
        ${projects
          .map((p) => {
            const date = new Date(p.timestamp).toLocaleString();
            return `<div class="dialog-item" data-id="${p.id}">
            <span class="dialog-name">${p.name}</span>
            <span class="dialog-date">${date}</span>
            <button class="dialog-delete" data-id="${p.id}" title="Delete">x</button>
          </div>`;
          })
          .join('')}
      </div>
      <button class="dialog-close">Cancel</button>
    </div>
  `;
  document.body.appendChild(dialog);

  dialog.querySelectorAll('.dialog-item').forEach((el) => {
    el.addEventListener('click', (e) => {
      const target = e.target as HTMLElement;
      if (target.classList.contains('dialog-delete')) return;
      const id = (el as HTMLElement).dataset.id!;
      dialog.remove();
      onSelect(id);
    });
  });

  dialog.querySelectorAll('.dialog-delete').forEach((el) => {
    el.addEventListener('click', async (e) => {
      e.stopPropagation();
      const id = (el as HTMLElement).dataset.id!;
      const name = (el as HTMLElement).closest('.dialog-item')?.querySelector('.dialog-name')
        ?.textContent;
      if (confirm(`Delete project "${name}"?`)) {
        await deleteProject(id);
        dialog.remove();
        // Re-open the dialog with updated list
        const updated = await listProjects();
        if (updated.length > 0) {
          showProjectDialog(updated, onSelect);
        }
      }
    });
  });

  dialog.querySelector('.dialog-close')!.addEventListener('click', () => dialog.remove());
  dialog.querySelector('.dialog-overlay')!.addEventListener('click', () => dialog.remove());
}

main();
