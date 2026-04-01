/**
 * Web Worker for browser-local LLM inference via WebLLM.
 *
 * Runs in a dedicated worker thread to avoid blocking the UI.
 * Communicates with the main thread via a typed postMessage protocol.
 */

import { CreateMLCEngine, type MLCEngine, type ChatCompletionMessageParam } from '@mlc-ai/web-llm';

// ── Message protocol ────────────────────────────────────────────────

export type WorkerRequest =
  | { type: 'init'; modelId: string }
  | { type: 'generate'; messages: ChatCompletionMessageParam[]; requestId: string }
  | { type: 'abort'; requestId: string };

export type WorkerResponse =
  | { type: 'init-progress'; progress: number; text: string }
  | { type: 'init-done' }
  | { type: 'init-error'; error: string }
  | { type: 'token'; requestId: string; token: string }
  | { type: 'done'; requestId: string; fullText: string }
  | { type: 'error'; requestId: string; error: string };

// ── Worker state ────────────────────────────────────────────────────

let engine: MLCEngine | null = null;

function post(msg: WorkerResponse): void {
  self.postMessage(msg);
}

// ── Handlers ────────────────────────────────────────────────────────

async function handleInit(modelId: string): Promise<void> {
  try {
    engine = await CreateMLCEngine(modelId, {
      initProgressCallback: (report) => {
        post({ type: 'init-progress', progress: report.progress, text: report.text });
      },
    });
    post({ type: 'init-done' });
  } catch (e) {
    post({ type: 'init-error', error: String(e) });
  }
}

async function handleGenerate(
  messages: ChatCompletionMessageParam[],
  requestId: string,
): Promise<void> {
  if (!engine) {
    post({ type: 'error', requestId, error: 'Engine not initialized' });
    return;
  }

  try {
    const stream = await engine.chat.completions.create({
      messages,
      stream: true,
      max_tokens: 2048,
      temperature: 0.3,
    });

    let fullText = '';
    for await (const chunk of stream) {
      const token = chunk.choices[0]?.delta?.content ?? '';
      if (token) {
        fullText += token;
        post({ type: 'token', requestId, token });
      }
    }
    post({ type: 'done', requestId, fullText });
  } catch (e) {
    post({ type: 'error', requestId, error: String(e) });
  }
}

function handleAbort(): void {
  if (engine) {
    engine.interruptGenerate();
  }
}

// ── Message listener ────────────────────────────────────────────────

self.onmessage = (e: MessageEvent<WorkerRequest>) => {
  const msg = e.data;
  switch (msg.type) {
    case 'init':
      handleInit(msg.modelId);
      break;
    case 'generate':
      handleGenerate(msg.messages, msg.requestId);
      break;
    case 'abort':
      handleAbort();
      break;
  }
};
