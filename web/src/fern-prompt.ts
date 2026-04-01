/**
 * System prompt and message assembly for the ferncad LLM assistant.
 *
 * Manages token budget (targeting ~4096 total), constructs the system
 * prompt with language reference and few-shot examples, and truncates
 * editor context / chat history to fit.
 */

import type { ChatCompletionMessageParam } from '@mlc-ai/web-llm';

// ── System prompt ───────────────────────────────────────────────────

const SYSTEM_PROMPT = `You are a CAD code assistant for ferncad, a Lisp-based 3D modeling language.
Generate ONLY valid .fern code. Do not include explanations unless the user asks for them.
Output raw code without markdown fencing.

## Language Reference

Syntax: S-expressions (function :keyword value ...)
Comments: ; line comment
Booleans: t (true), nil (false)

### Literals
- Numbers: 42, 3.14
- Units: #u(10 :mm), #u(1 :inch), #u(5 :cm)
- Angles: #a(90 :deg), #a(3.14 :rad)
- Vectors: #v(1 0 0)
- Points: #p(0 0 5)

### Primitives (all return Shape)
(box :width W :depth D :height H)
(cube :size S)
(sphere :radius R :segments 32)
(cylinder :radius R :height H :segments 32)
(cone :radius-bottom R1 :radius-top R2 :height H)
(torus :radius-major R :radius-minor r)
(prism :sides N :radius R :height H)

### CSG Operations
(union shape1 shape2 ...)
(difference base cutter1 ...)
(intersection shape1 shape2 ...)

### Transforms
(translate :shape S :by (list X Y Z))
(rotate :shape S :axis :x :angle #a(45 :deg))    ; :axis is :x, :y, or :z
(scale :shape S :factor F)                         ; uniform
(scale :shape S :factor (list Fx Fy Fz))           ; per-axis

### 2D Profiles & Extrusion
(polygon (list x1 y1) (list x2 y2) ...)
(circle :radius R :segments 32)
(extrude :profile P :height H)
(revolve :profile P :angle #a(360 :deg) :segments 32)

### Sweep & Loft
(helix :radius R :pitch P :turns N)
(arc :radius R :angle #a(180 :deg))
(bezier :points (list #p(0 0 0) #p(5 5 0) #p(10 0 0)))
(sweep :profile P :path PATH :segments 64)
(loft :profiles (list P1 P2) :at (list 0.0 1.0) :segments 32)

### Definitions
(defvar name value)
(defun name (args) body)
(defpart name "description"
  :params ((param :: type :default val :doc "..."))
  :body expression)

### Control Flow
(let* ((v1 expr1) (v2 expr2)) body)
(if condition then-expr else-expr)
(cond (test1 result1) (test2 result2) ...)
(dotimes (i n) body)
(lambda (args) body)
(progn expr1 expr2 ...)

### Math
+, -, *, /, sin, cos, tan, sqrt, abs, mod, expt, floor, ceil, min, max, pi, atan2

### Lists
(list a b c), (cons h t), (car l), (cdr l), (append l1 l2), (nth n l), (length l), (reverse l)

### Standard Library
(require "fasteners/m3-bolt")   ; (m3-bolt :length L)
(require "fasteners/flat-washer")
(require "gears/spur-gear")     ; (spur-gear :module M :teeth N :thickness T)
(require "gears/bevel-gear")

## Examples

User: a box
Code:
(box :width 20 :depth 20 :height 10)

User: a cylinder with a hole through it
Code:
(difference
  (cylinder :radius 10 :height 20)
  (cylinder :radius 5 :height 22))

User: an L-shaped bracket with two mounting holes
Code:
(let* ((base (box :width 40 :depth 20 :height 5))
       (wall (translate :shape (box :width 5 :depth 20 :height 30)
                        :by (list 17.5 0 15)))
       (body (union base wall))
       (hole1 (translate :shape (cylinder :radius 3 :height 7)
                         :by (list -10 0 0)))
       (hole2 (translate :shape (cylinder :radius 3 :height 7)
                         :by (list 10 0 0))))
  (difference body hole1 hole2))

User: a spring
Code:
(sweep :profile (circle :radius 0.5 :segments 12)
       :path (helix :radius 5 :pitch 3 :turns 6)
       :segments 128)

User: a simple gear with 20 teeth
Code:
(require "gears/spur-gear")
(spur-gear :module 2.0 :teeth 20 :thickness 8)`;

// ── Token estimation ────────────────────────────────────────────────

const CHARS_PER_TOKEN = 4;
const MAX_CONTEXT_TOKENS = 4096;
const SYSTEM_TOKENS = Math.ceil(SYSTEM_PROMPT.length / CHARS_PER_TOKEN);
const GENERATION_RESERVE = 1500;

function estimateTokens(text: string): number {
  return Math.ceil(text.length / CHARS_PER_TOKEN);
}

// ── Message builder ─────────────────────────────────────────────────

export interface ChatEntry {
  role: 'user' | 'assistant';
  content: string;
}

/**
 * Build a ChatCompletionMessageParam array that fits within ~4096 tokens.
 *
 * Priority order for space allocation:
 *   1. System prompt (fixed)
 *   2. Current user message
 *   3. Editor context (truncated if needed)
 *   4. Chat history (oldest dropped first)
 */
export function buildMessages(
  userMessage: string,
  editorContent: string,
  chatHistory: ChatEntry[],
): ChatCompletionMessageParam[] {
  const messages: ChatCompletionMessageParam[] = [{ role: 'system', content: SYSTEM_PROMPT }];

  let usedTokens = SYSTEM_TOKENS + GENERATION_RESERVE;

  // Current user message with optional editor context
  let userContent = '';

  // Trim editor content to fit
  if (editorContent.trim()) {
    const editorTokens = estimateTokens(editorContent);
    const availableForEditor = MAX_CONTEXT_TOKENS - usedTokens - estimateTokens(userMessage) - 50;

    if (availableForEditor > 100 && editorTokens > 0) {
      let trimmedEditor = editorContent;
      if (editorTokens > availableForEditor) {
        // Keep first and last portions
        const charBudget = availableForEditor * CHARS_PER_TOKEN;
        const half = Math.floor(charBudget / 2);
        trimmedEditor =
          editorContent.slice(0, half) + '\n;; ... (truncated) ...\n' + editorContent.slice(-half);
      }
      userContent += `Current code:\n${trimmedEditor}\n\n`;
    }
  }

  userContent += userMessage;
  usedTokens += estimateTokens(userContent);

  // Add chat history (newest first, then reverse)
  const historyMessages: ChatCompletionMessageParam[] = [];
  for (let i = chatHistory.length - 1; i >= 0; i--) {
    const entry = chatHistory[i];
    const tokens = estimateTokens(entry.content);
    if (usedTokens + tokens > MAX_CONTEXT_TOKENS) break;
    usedTokens += tokens;
    historyMessages.unshift({ role: entry.role, content: entry.content });
  }

  messages.push(...historyMessages);
  messages.push({ role: 'user', content: userContent });

  return messages;
}

/**
 * Extract .fern code from LLM output.
 *
 * Strips markdown fencing if present, otherwise returns content
 * that looks like S-expressions.
 */
export function extractCode(response: string): string {
  // Strip markdown code fences
  const fenced = response.match(/```(?:lisp|fern|scheme)?\s*\n?([\s\S]*?)```/);
  if (fenced) return fenced[1].trim();

  // Look for content starting with ( or ;
  const lines = response.split('\n');
  const codeLines: string[] = [];
  let inCode = false;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!inCode && (trimmed.startsWith('(') || trimmed.startsWith(';'))) {
      inCode = true;
    }
    if (inCode) {
      codeLines.push(line);
    }
  }

  if (codeLines.length > 0) return codeLines.join('\n').trim();

  // Fallback: return as-is
  return response.trim();
}
