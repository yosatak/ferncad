/**
 * localStorage persistence for ferncad models
 */

const CURRENT_KEY = 'ferncad-current';
const MODELS_KEY = 'ferncad-models';

/** Auto-save current editor content */
export function saveCurrentCode(code: string): void {
  try {
    localStorage.setItem(CURRENT_KEY, code);
  } catch {
    // localStorage may be unavailable (private browsing, quota exceeded)
  }
}

/** Load last auto-saved editor content */
export function loadCurrentCode(): string | null {
  try {
    return localStorage.getItem(CURRENT_KEY);
  } catch {
    return null;
  }
}

/** Saved model entry */
export interface SavedModel {
  name: string;
  code: string;
  timestamp: number;
}

/** Save a named model */
export function saveModel(name: string, code: string): void {
  const models = listModels();
  const existing = models.findIndex((m) => m.name === name);
  const entry: SavedModel = { name, code, timestamp: Date.now() };
  if (existing >= 0) {
    models[existing] = entry;
  } else {
    models.push(entry);
  }
  try {
    localStorage.setItem(MODELS_KEY, JSON.stringify(models));
  } catch {
    // ignore
  }
}

/** Load a named model */
export function loadModel(name: string): string | null {
  const models = listModels();
  return models.find((m) => m.name === name)?.code ?? null;
}

/** List all saved models */
export function listModels(): SavedModel[] {
  try {
    const raw = localStorage.getItem(MODELS_KEY);
    if (!raw) return [];
    return JSON.parse(raw) as SavedModel[];
  } catch {
    return [];
  }
}

/** Delete a named model */
export function deleteModel(name: string): void {
  const models = listModels().filter((m) => m.name !== name);
  try {
    localStorage.setItem(MODELS_KEY, JSON.stringify(models));
  } catch {
    // ignore
  }
}
