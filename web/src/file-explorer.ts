/**
 * File Explorer
 *
 * Left sidebar component showing the project's file list with
 * create, rename, and delete functionality.
 */

import type { VirtualFileSystem } from './vfs';

export interface FileExplorerOptions {
  container: HTMLElement;
  vfs: VirtualFileSystem;
  onFileSelect: (filename: string) => void;
  onFileCreate: (filename: string) => void;
  onFileDelete: (filename: string) => void;
  onFileRename: (oldName: string, newName: string) => void;
}

export class FileExplorer {
  private container: HTMLElement;
  private vfs: VirtualFileSystem;
  private onFileSelect: (filename: string) => void;
  private onFileCreate: (filename: string) => void;
  private onFileDelete: (filename: string) => void;
  private onFileRename: (oldName: string, newName: string) => void;
  private activeFile: string = '';
  private listEl: HTMLElement;

  constructor(options: FileExplorerOptions) {
    this.container = options.container;
    this.vfs = options.vfs;
    this.onFileSelect = options.onFileSelect;
    this.onFileCreate = options.onFileCreate;
    this.onFileDelete = options.onFileDelete;
    this.onFileRename = options.onFileRename;

    // Build DOM
    const header = document.createElement('div');
    header.className = 'fe-header';

    const title = document.createElement('span');
    title.className = 'fe-title';
    title.textContent = 'Files';
    header.appendChild(title);

    const addBtn = document.createElement('button');
    addBtn.className = 'fe-add';
    addBtn.title = 'New file';
    addBtn.textContent = '+';
    addBtn.addEventListener('click', () => this.promptNewFile());
    header.appendChild(addBtn);

    this.listEl = document.createElement('div');
    this.listEl.className = 'fe-list';

    this.container.appendChild(header);
    this.container.appendChild(this.listEl);

    // Re-render on VFS changes
    this.vfs.on(() => this.render());
  }

  /** Set which file is visually active */
  setActiveFile(filename: string): void {
    this.activeFile = filename;
    this.render();
  }

  /** Re-render the file list from VFS */
  render(): void {
    this.listEl.innerHTML = '';
    const files = this.vfs.listFiles();

    for (const filename of files) {
      const item = document.createElement('div');
      item.className = 'fe-item' + (filename === this.activeFile ? ' fe-active' : '');

      const nameSpan = document.createElement('span');
      nameSpan.className = 'fe-name';
      if (filename === 'main.fern') {
        nameSpan.classList.add('fe-main');
      }
      nameSpan.textContent = filename;
      nameSpan.addEventListener('click', () => this.onFileSelect(filename));

      const actions = document.createElement('span');
      actions.className = 'fe-actions';

      // Rename button
      const renameBtn = document.createElement('button');
      renameBtn.className = 'fe-rename';
      renameBtn.title = 'Rename';
      renameBtn.textContent = '\u270E'; // ✎
      renameBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        this.startInlineRename(item, nameSpan, filename);
      });
      actions.appendChild(renameBtn);

      // Delete button (disabled for main.fern and when only 1 file)
      if (filename !== 'main.fern' && this.vfs.fileCount > 1) {
        const deleteBtn = document.createElement('button');
        deleteBtn.className = 'fe-delete';
        deleteBtn.title = 'Delete';
        deleteBtn.textContent = '\u00D7'; // ×
        deleteBtn.addEventListener('click', (e) => {
          e.stopPropagation();
          if (confirm(`Delete "${filename}"?`)) {
            this.onFileDelete(filename);
          }
        });
        actions.appendChild(deleteBtn);
      }

      item.appendChild(nameSpan);
      item.appendChild(actions);
      this.listEl.appendChild(item);
    }
  }

  private promptNewFile(): void {
    let name = prompt('New file name:', 'untitled.fern');
    if (!name) return;
    if (!name.endsWith('.fern')) {
      name += '.fern';
    }
    if (this.vfs.hasFile(name)) {
      alert(`File "${name}" already exists.`);
      return;
    }
    this.onFileCreate(name);
  }

  private startInlineRename(
    item: HTMLElement,
    nameSpan: HTMLElement,
    oldName: string,
  ): void {
    const input = document.createElement('input');
    input.className = 'fe-rename-input';
    input.value = oldName;
    input.select();

    const commit = () => {
      let newName = input.value.trim();
      if (!newName) {
        this.render();
        return;
      }
      if (!newName.endsWith('.fern')) {
        newName += '.fern';
      }
      if (newName !== oldName && this.vfs.hasFile(newName)) {
        alert(`File "${newName}" already exists.`);
        this.render();
        return;
      }
      if (newName !== oldName) {
        this.onFileRename(oldName, newName);
      } else {
        this.render();
      }
    };

    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        commit();
      } else if (e.key === 'Escape') {
        this.render();
      }
    });
    input.addEventListener('blur', commit);

    nameSpan.replaceWith(input);
    input.focus();
  }
}
