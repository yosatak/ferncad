/**
 * IndexedDB-based project storage
 *
 * Stores multi-file projects with metadata. Replaces the previous
 * localStorage-based single-file persistence.
 */

export interface ProjectFile {
  name: string;
  content: string;
}

export interface Project {
  id: string;
  name: string;
  files: ProjectFile[];
  activeFile: string;
  timestamp: number;
}

export interface ProjectSummary {
  id: string;
  name: string;
  timestamp: number;
}

interface SessionState {
  key: string;
  currentProjectId: string | null;
}

const DB_NAME = 'ferncad-projects';
const DB_VERSION = 1;
const STORE_PROJECTS = 'projects';
const STORE_SESSION = 'session';

let db: IDBDatabase | null = null;

/** Open (or create) the IndexedDB database */
export async function initProjectDB(): Promise<void> {
  if (db) return;
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(STORE_PROJECTS)) {
        database.createObjectStore(STORE_PROJECTS, { keyPath: 'id' });
      }
      if (!database.objectStoreNames.contains(STORE_SESSION)) {
        database.createObjectStore(STORE_SESSION, { keyPath: 'key' });
      }
    };
    request.onsuccess = () => {
      db = request.result;
      resolve();
    };
    request.onerror = () => reject(request.error);
  });
}

function getDB(): IDBDatabase {
  if (!db) throw new Error('Project database not initialized. Call initProjectDB() first.');
  return db;
}

/** Generate a simple unique ID */
function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
}

/** Save a project (insert or update) */
export async function saveProject(project: Project): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = getDB().transaction(STORE_PROJECTS, 'readwrite');
    const store = tx.objectStore(STORE_PROJECTS);
    project.timestamp = Date.now();
    store.put(project);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** Load a project by ID */
export async function loadProject(id: string): Promise<Project | null> {
  return new Promise((resolve, reject) => {
    const tx = getDB().transaction(STORE_PROJECTS, 'readonly');
    const store = tx.objectStore(STORE_PROJECTS);
    const request = store.get(id);
    request.onsuccess = () => resolve(request.result ?? null);
    request.onerror = () => reject(request.error);
  });
}

/** List all projects (summary only) */
export async function listProjects(): Promise<ProjectSummary[]> {
  return new Promise((resolve, reject) => {
    const tx = getDB().transaction(STORE_PROJECTS, 'readonly');
    const store = tx.objectStore(STORE_PROJECTS);
    const request = store.getAll();
    request.onsuccess = () => {
      const projects: Project[] = request.result;
      const summaries = projects
        .map((p) => ({ id: p.id, name: p.name, timestamp: p.timestamp }))
        .sort((a, b) => b.timestamp - a.timestamp);
      resolve(summaries);
    };
    request.onerror = () => reject(request.error);
  });
}

/** Delete a project by ID */
export async function deleteProject(id: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = getDB().transaction(STORE_PROJECTS, 'readwrite');
    const store = tx.objectStore(STORE_PROJECTS);
    store.delete(id);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** Save session state */
export async function saveSession(currentProjectId: string | null): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx = getDB().transaction(STORE_SESSION, 'readwrite');
    const store = tx.objectStore(STORE_SESSION);
    store.put({ key: 'current', currentProjectId } as SessionState);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** Load session state */
export async function loadSession(): Promise<string | null> {
  return new Promise((resolve, reject) => {
    const tx = getDB().transaction(STORE_SESSION, 'readonly');
    const store = tx.objectStore(STORE_SESSION);
    const request = store.get('current');
    request.onsuccess = () => {
      const state = request.result as SessionState | undefined;
      resolve(state?.currentProjectId ?? null);
    };
    request.onerror = () => reject(request.error);
  });
}

/** Create a new empty project and return it */
export function createNewProject(name: string): Project {
  return {
    id: generateId(),
    name,
    files: [{ name: 'main.fern', content: defaultMainContent() }],
    activeFile: 'main.fern',
    timestamp: Date.now(),
  };
}

/** Migrate data from localStorage to IndexedDB. Returns the migrated project ID or null. */
export async function migrateFromLocalStorage(): Promise<string | null> {
  try {
    if (localStorage.getItem('ferncad-migrated') === 'true') {
      return null;
    }

    const currentCode = localStorage.getItem('ferncad-current');
    const modelsJson = localStorage.getItem('ferncad-models');

    let migratedProjectId: string | null = null;

    // Migrate current code as the active project
    if (currentCode) {
      const project = createNewProject('Untitled');
      project.files = [{ name: 'main.fern', content: currentCode }];
      await saveProject(project);
      await saveSession(project.id);
      migratedProjectId = project.id;
    }

    // Migrate saved models as separate projects
    if (modelsJson) {
      try {
        const models = JSON.parse(modelsJson) as Array<{
          name: string;
          code: string;
          timestamp: number;
        }>;
        for (const model of models) {
          const project: Project = {
            id: generateId(),
            name: model.name,
            files: [{ name: 'main.fern', content: model.code }],
            activeFile: 'main.fern',
            timestamp: model.timestamp,
          };
          await saveProject(project);
        }
      } catch {
        // Ignore malformed models JSON
      }
    }

    localStorage.setItem('ferncad-migrated', 'true');
    return migratedProjectId;
  } catch {
    // localStorage or IndexedDB errors in private browsing, etc.
    return null;
  }
}

function defaultMainContent(): string {
  return `;; ferncad — Lisp CAD Modeler
;; Press Run or Ctrl+Enter to evaluate

;; Basic primitives
; (box :width 20 :depth 20 :height 20)
; (sphere :radius 10)

;; CSG example
(difference
  (box :width 20 :depth 20 :height 20)
  (sphere :radius 12))
`;
}
