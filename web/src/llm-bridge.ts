/**
 * Main-thread bridge for the LLM Web Worker.
 *
 * Provides an async API for model initialization, streaming generation,
 * and abort. The worker is lazily created on first init() call.
 */

import type { ChatMessage, WorkerRequest, WorkerResponse } from './llm-types';

export interface StreamCallbacks {
  onToken: (token: string) => void;
  onDone: (fullText: string) => void;
  onError: (error: string) => void;
}

export interface ProgressCallback {
  (progress: number, text: string): void;
}

/** Available model presets */
export const MODEL_OPTIONS = [
  {
    id: 'Phi-3.5-mini-instruct-q4f16_1-MLC',
    label: 'Phi-3.5 mini (2 GB)',
    size: '~2 GB',
  },
  {
    id: 'SmolLM2-1.7B-Instruct-q4f16_1-MLC',
    label: 'SmolLM2 1.7B (1 GB)',
    size: '~1 GB',
  },
] as const;

export const DEFAULT_MODEL: string = MODEL_OPTIONS[0].id;

export class LLMBridge {
  private worker: Worker | null = null;
  private ready = false;
  private pendingRequests = new Map<string, StreamCallbacks>();
  private requestCounter = 0;

  /** Lazily create the worker and load the model. */
  async init(modelId: string, onProgress: ProgressCallback): Promise<void> {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
      this.ready = false;
    }

    return new Promise<void>((resolve, reject) => {
      this.worker = new Worker(new URL('./llm-worker.ts', import.meta.url), { type: 'module' });

      this.worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
        const msg = e.data;
        switch (msg.type) {
          case 'init-progress':
            onProgress(msg.progress, msg.text);
            break;
          case 'init-done':
            this.ready = true;
            // Switch to streaming handler
            this.worker!.onmessage = this.handleStreamMessage.bind(this);
            resolve();
            break;
          case 'init-error':
            reject(new Error(msg.error));
            break;
        }
      };

      const req: WorkerRequest = { type: 'init', modelId };
      this.worker.postMessage(req);
    });
  }

  /** Start a streaming generation. Returns a requestId for abort. */
  generate(messages: ChatMessage[], callbacks: StreamCallbacks): string {
    if (!this.worker || !this.ready) {
      callbacks.onError('LLM not initialized');
      return '';
    }

    const requestId = `req-${++this.requestCounter}`;
    this.pendingRequests.set(requestId, callbacks);

    const req: WorkerRequest = { type: 'generate', messages, requestId };
    this.worker.postMessage(req);
    return requestId;
  }

  /** Abort an in-progress generation. */
  abort(requestId: string): void {
    if (!this.worker) return;
    this.pendingRequests.delete(requestId);
    const req: WorkerRequest = { type: 'abort', requestId };
    this.worker.postMessage(req);
  }

  isReady(): boolean {
    return this.ready;
  }

  dispose(): void {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
      this.ready = false;
    }
    this.pendingRequests.clear();
  }

  private handleStreamMessage(e: MessageEvent<WorkerResponse>): void {
    const msg = e.data;
    if (!('requestId' in msg)) return;

    const cb = this.pendingRequests.get(msg.requestId);
    if (!cb) return;

    switch (msg.type) {
      case 'token':
        cb.onToken(msg.token);
        break;
      case 'done':
        this.pendingRequests.delete(msg.requestId);
        cb.onDone(msg.fullText);
        break;
      case 'error':
        this.pendingRequests.delete(msg.requestId);
        cb.onError(msg.error);
        break;
    }
  }
}
