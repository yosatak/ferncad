/**
 * Shared type definitions for the LLM worker protocol.
 *
 * Kept in a separate file so the main-thread bridge can import types
 * without pulling in the worker module (and its heavy @mlc-ai/web-llm dependency).
 */

/** Chat message format (mirrors @mlc-ai/web-llm ChatCompletionMessageParam). */
export interface ChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

export type WorkerRequest =
  | { type: 'init'; modelId: string }
  | { type: 'generate'; messages: ChatMessage[]; requestId: string }
  | { type: 'abort'; requestId: string };

export type WorkerResponse =
  | { type: 'init-progress'; progress: number; text: string }
  | { type: 'init-done' }
  | { type: 'init-error'; error: string }
  | { type: 'token'; requestId: string; token: string }
  | { type: 'done'; requestId: string; fullText: string }
  | { type: 'error'; requestId: string; error: string };
