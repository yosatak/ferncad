/**
 * Tab Bar
 *
 * Manages open file tabs above the editor. Stores EditorState per tab
 * so cursor position and undo history are preserved when switching.
 */

import type { EditorState } from '@codemirror/state';

interface Tab {
  filename: string;
  editorState: EditorState | null;
}

export interface TabBarOptions {
  container: HTMLElement;
  onTabSelect: (filename: string) => void;
  onTabClose: (filename: string) => void;
}

export class TabBar {
  private tabs: Tab[] = [];
  private activeTab: string | null = null;
  private container: HTMLElement;
  private onTabSelect: (filename: string) => void;
  private onTabClose: (filename: string) => void;

  constructor(options: TabBarOptions) {
    this.container = options.container;
    this.onTabSelect = options.onTabSelect;
    this.onTabClose = options.onTabClose;
  }

  /** Open a file in a new tab (or activate if already open) */
  openTab(filename: string): void {
    const existing = this.tabs.find((t) => t.filename === filename);
    if (!existing) {
      this.tabs.push({ filename, editorState: null });
    }
    this.activeTab = filename;
    this.render();
  }

  /** Close a tab */
  closeTab(filename: string): void {
    const idx = this.tabs.findIndex((t) => t.filename === filename);
    if (idx < 0) return;
    this.tabs.splice(idx, 1);

    // If we closed the active tab, activate the nearest remaining tab
    if (this.activeTab === filename) {
      if (this.tabs.length > 0) {
        const newIdx = Math.min(idx, this.tabs.length - 1);
        this.activeTab = this.tabs[newIdx].filename;
        this.onTabSelect(this.activeTab);
      } else {
        this.activeTab = null;
      }
    }
    this.render();
  }

  /** Set the active tab without opening a new one */
  setActiveTab(filename: string): void {
    this.activeTab = filename;
    this.render();
  }

  /** Rename a tab (when file is renamed) */
  renameTab(oldName: string, newName: string): void {
    const tab = this.tabs.find((t) => t.filename === oldName);
    if (tab) {
      tab.filename = newName;
    }
    if (this.activeTab === oldName) {
      this.activeTab = newName;
    }
    this.render();
  }

  /** Store the current EditorState for a tab */
  saveEditorState(filename: string, state: EditorState): void {
    const tab = this.tabs.find((t) => t.filename === filename);
    if (tab) {
      tab.editorState = state;
    }
  }

  /** Get the stored EditorState for a tab */
  getEditorState(filename: string): EditorState | null {
    const tab = this.tabs.find((t) => t.filename === filename);
    return tab?.editorState ?? null;
  }

  /** Get list of open tab filenames */
  getOpenTabs(): string[] {
    return this.tabs.map((t) => t.filename);
  }

  /** Get the active tab filename */
  getActiveTab(): string | null {
    return this.activeTab;
  }

  /** Re-render the tab bar */
  render(): void {
    this.container.innerHTML = '';
    for (const tab of this.tabs) {
      const el = document.createElement('div');
      el.className = 'tab' + (tab.filename === this.activeTab ? ' tab-active' : '');

      const nameSpan = document.createElement('span');
      nameSpan.className = 'tab-name';
      nameSpan.textContent = tab.filename;
      nameSpan.addEventListener('click', () => {
        if (tab.filename !== this.activeTab) {
          this.onTabSelect(tab.filename);
        }
      });

      const closeBtn = document.createElement('span');
      closeBtn.className = 'tab-close';
      closeBtn.textContent = '\u00D7'; // ×
      closeBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        this.onTabClose(tab.filename);
      });

      el.appendChild(nameSpan);
      el.appendChild(closeBtn);
      this.container.appendChild(el);
    }
  }
}
