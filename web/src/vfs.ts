/**
 * Virtual File System
 *
 * In-memory file store for managing multiple .fern files in a project.
 * Emits events on changes so that UI components can react.
 */

export type VfsEventType = 'create' | 'delete' | 'rename' | 'change' | 'load';

export interface VfsEvent {
  type: VfsEventType;
  filename: string;
  oldFilename?: string;
}

export type VfsListener = (event: VfsEvent) => void;

export class VirtualFileSystem {
  private files: Map<string, string> = new Map();
  private listeners: VfsListener[] = [];

  /** Load all files at once (used when opening a project). Emits 'load' for each file. */
  loadFiles(files: Array<{ name: string; content: string }>): void {
    this.files.clear();
    for (const f of files) {
      this.files.set(f.name, f.content);
    }
    for (const f of files) {
      this.emit({ type: 'load', filename: f.name });
    }
  }

  /** Get file content; returns undefined if file does not exist */
  getFile(name: string): string | undefined {
    return this.files.get(name);
  }

  /** Set file content. Creates if new (emits 'create'), updates if existing (emits 'change'). */
  setFile(name: string, content: string): void {
    const isNew = !this.files.has(name);
    this.files.set(name, content);
    this.emit({ type: isNew ? 'create' : 'change', filename: name });
  }

  /** Delete a file */
  deleteFile(name: string): void {
    if (this.files.delete(name)) {
      this.emit({ type: 'delete', filename: name });
    }
  }

  /** Rename a file */
  renameFile(oldName: string, newName: string): void {
    const content = this.files.get(oldName);
    if (content === undefined) return;
    this.files.delete(oldName);
    this.files.set(newName, content);
    this.emit({ type: 'rename', filename: newName, oldFilename: oldName });
  }

  /** List all filenames sorted alphabetically, with main.fern always first */
  listFiles(): string[] {
    const names = Array.from(this.files.keys()).sort();
    const mainIdx = names.indexOf('main.fern');
    if (mainIdx > 0) {
      names.splice(mainIdx, 1);
      names.unshift('main.fern');
    }
    return names;
  }

  /** Get all files as a plain object for passing to WASM */
  toFileMap(): Record<string, string> {
    const map: Record<string, string> = {};
    for (const [name, content] of this.files) {
      map[name] = content;
    }
    return map;
  }

  /** Check if a file exists */
  hasFile(name: string): boolean {
    return this.files.has(name);
  }

  /** Get the number of files */
  get fileCount(): number {
    return this.files.size;
  }

  /** Subscribe to VFS events. Returns an unsubscribe function. */
  on(listener: VfsListener): () => void {
    this.listeners.push(listener);
    return () => {
      const idx = this.listeners.indexOf(listener);
      if (idx >= 0) this.listeners.splice(idx, 1);
    };
  }

  private emit(event: VfsEvent): void {
    for (const listener of this.listeners) {
      listener(event);
    }
  }
}
