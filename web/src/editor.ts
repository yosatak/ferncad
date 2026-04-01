/**
 * CodeMirror 6 editor setup
 *
 * Provides Lisp-style syntax highlighting and bracket matching.
 */

import { EditorView, basicSetup } from 'codemirror';
import { EditorState, StateField, StateEffect } from '@codemirror/state';
import { Decoration, type DecorationSet } from '@codemirror/view';
import { StreamLanguage } from '@codemirror/language';
import { tags } from '@lezer/highlight';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { linter, type Diagnostic } from '@codemirror/lint';
import { autocompletion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete';
import { checkSyntax, getCompletions } from './wasm-bridge';

/** Effect to set/clear the highlighted span */
const setHighlightEffect = StateEffect.define<{ from: number; to: number } | null>();

/** Decoration mark for shape highlighting */
const highlightMark = Decoration.mark({ class: 'cm-shape-highlight' });

/** StateField managing the highlight decoration set */
const highlightField = StateField.define<DecorationSet>({
  create() {
    return Decoration.none;
  },
  update(decorations, tr) {
    for (const effect of tr.effects) {
      if (effect.is(setHighlightEffect)) {
        if (effect.value) {
          return Decoration.set([
            highlightMark.range(effect.value.from, effect.value.to),
          ]);
        }
        return Decoration.none;
      }
    }
    // Remap decorations on doc changes
    if (tr.docChanged) return Decoration.none;
    return decorations;
  },
  provide: (f) => EditorView.decorations.from(f),
});

/** ferncad language StreamLanguage definition */
const fernLanguage = StreamLanguage.define({
  startState() {
    return { depth: 0 };
  },
  token(stream, _state) {
    // Comments
    if (stream.match(/;.*/)) return 'lineComment';
    // Whitespace
    if (stream.eatSpace()) return null;
    // Strings
    if (stream.match(/"([^"\\]|\\.)*"/)) return 'string';
    // Special literal prefixes
    if (stream.match(/#[uavp]/)) return 'meta';
    // Type annotation
    if (stream.match(/::/)) return 'operator';
    // Keywords
    if (stream.match(/:[a-zA-Z][a-zA-Z0-9\-_/]*/)) return 'atom';
    // Numbers
    if (stream.match(/-?[0-9]+(\.[0-9]*)?([eE][+-]?[0-9]+)?/)) return 'number';
    // Parens
    if (stream.eat('(')) return 'paren';
    if (stream.eat(')')) return 'paren';
    // Definition keywords
    if (stream.match(/defpart|defun|defmacro|defvar|defmeta/)) return 'definitionKeyword';
    // Control flow
    if (stream.match(/let\*|if|cond|quote|quasiquote|lambda|progn/)) return 'keyword';
    // CSG operators
    if (stream.match(/union|difference|intersection/)) return 'operatorKeyword';
    // Primitives
    if (stream.match(/box|sphere|cylinder|cone|torus|prism|cube/)) return 'typeName';
    // Transforms
    if (stream.match(/translate|rotate|scale|mirror|extrude|revolve|chamfer/)) return 'function';
    // General symbols
    if (stream.match(/[a-zA-Z_+*!?<>=][a-zA-Z0-9_+*\-/!?.<>=]*/)) return 'variableName';
    // Skip unknown characters
    stream.next();
    return null;
  },
});

/** Dark theme syntax highlighting */
const fernHighlightStyle = HighlightStyle.define([
  { tag: tags.lineComment, color: '#6a9955' },
  { tag: tags.string, color: '#ce9178' },
  { tag: tags.number, color: '#b5cea8' },
  { tag: tags.atom, color: '#9cdcfe' },
  { tag: tags.meta, color: '#c586c0' },
  { tag: tags.operator, color: '#d4d4d4' },
  { tag: tags.definitionKeyword, color: '#569cd6', fontWeight: 'bold' },
  { tag: tags.keyword, color: '#c586c0' },
  { tag: tags.operatorKeyword, color: '#dcdcaa', fontWeight: 'bold' },
  { tag: tags.typeName, color: '#4ec9b0' },
  { tag: tags.function(tags.name), color: '#dcdcaa' },
  { tag: tags.variableName, color: '#d4d4d4' },
  { tag: tags.paren, color: '#808080' },
]);

/** Dark theme */
const darkTheme = EditorView.theme({
  '&': { backgroundColor: '#1e1e1e', color: '#d4d4d4' },
  '.cm-content': {
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
    fontSize: '14px',
    caretColor: '#d4d4d4',
  },
  '.cm-gutters': { backgroundColor: '#1e1e1e', color: '#858585', border: 'none' },
  '.cm-activeLine': { backgroundColor: '#2a2d2e' },
  '.cm-activeLineGutter': { backgroundColor: '#2a2d2e' },
  '&.cm-focused .cm-cursor': { borderLeftColor: '#d4d4d4' },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: '#264f78',
  },
  '.cm-matchingBracket': { backgroundColor: '#3e3e3e', outline: '1px solid #888' },
  '.cm-shape-highlight': {
    backgroundColor: 'rgba(51, 85, 153, 0.35)',
    borderRadius: '2px',
  },
  '.cm-tooltip.cm-tooltip-autocomplete': {
    backgroundColor: '#252526',
    border: '1px solid #454545',
  },
  '.cm-tooltip-autocomplete .cm-completionInfo': {
    backgroundColor: '#2d2d2d',
    border: '1px solid #454545',
    padding: '6px 8px',
    maxWidth: '400px',
    fontSize: '13px',
    lineHeight: '1.4',
  },
  '.cm-completion-info-detail': {
    color: '#ccc',
  },
  '.cm-completion-example': {
    marginTop: '6px',
    padding: '6px 8px',
    backgroundColor: '#1e1e1e',
    borderRadius: '3px',
    fontSize: '12px',
    lineHeight: '1.5',
    overflowX: 'auto',
    whiteSpace: 'pre',
  },
  '.cm-completion-example code': {
    color: '#d4d4d4',
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
  },
});

/** Completion source powered by the Rust completion engine via WASM */
function fernCompletionSource(context: CompletionContext): CompletionResult | null {
  // Match symbol-like identifiers (including Lisp operators like +, -, *, etc.)
  const word = context.matchBefore(/[a-zA-Z_+*!?<>=][a-zA-Z0-9_+*\-/!?.<>=]*/);
  // Match keyword arguments like :radius, :height
  const kwTrigger = context.matchBefore(/:[a-zA-Z][a-zA-Z0-9\-_]*/);

  if (!word && !kwTrigger && !context.explicit) return null;

  const from = word?.from ?? kwTrigger?.from ?? context.pos;
  const source = context.state.doc.toString();
  const items = getCompletions(source, context.pos);

  return {
    from,
    options: items.map(item => ({
      label: item.label,
      type: item.kind,
      detail: item.detail,
      info: () => {
        const dom = document.createElement('div');
        dom.className = 'cm-completion-info-detail';
        const desc = document.createElement('div');
        desc.textContent = item.doc;
        dom.appendChild(desc);
        if (item.example) {
          const pre = document.createElement('pre');
          pre.className = 'cm-completion-example';
          const code = document.createElement('code');
          code.textContent = item.example;
          pre.appendChild(code);
          dom.appendChild(pre);
        }
        return dom;
      },
      boost: item.boost,
    })),
  };
}

/** Default sample code */
const DEFAULT_CODE = `;; ferncad — Lisp CAD Modeler
;; Press Run or Ctrl+Enter to evaluate

;; Basic primitives
; (box :width 20 :depth 20 :height 20)
; (sphere :radius 10)

;; CSG example
(difference
  (box :width 20 :depth 20 :height 20)
  (sphere :radius 12))
`;

/**
 * Create a CodeMirror editor instance.
 *
 * @param container - DOM element to mount the editor in
 * @param onChange - Callback fired on code changes (debounced 300ms)
 * @param onCursorChange - Callback fired on cursor position changes
 * @returns EditorView instance
 */
export function createEditor(
  container: HTMLElement,
  onChange: (code: string) => void,
  initialCode?: string,
  onCursorChange?: () => void,
): EditorView {
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  /** Linter that calls WASM check_syntax and maps diagnostics to editor positions */
  const fernLinter = linter((view) => {
    const doc = view.state.doc;
    const source = doc.toString();
    const entries = checkSyntax(source);
    const diagnostics: Diagnostic[] = [];
    for (const entry of entries) {
      if (entry.line < 1) continue;
      const lineCount = doc.lines;
      const lineNum = Math.min(entry.line, lineCount);
      const line = doc.line(lineNum);
      const col = Math.min(entry.col, line.length + 1);
      const from = line.from + col - 1;
      const to = Math.min(from + 1, line.to);
      diagnostics.push({
        from,
        to,
        severity: entry.severity === 'warning' ? 'warning' : 'error',
        message: entry.message,
      });
    }
    return diagnostics;
  }, { delay: 500 });

  const state = EditorState.create({
    doc: initialCode ?? DEFAULT_CODE,
    extensions: [
      basicSetup,
      fernLanguage,
      syntaxHighlighting(fernHighlightStyle),
      darkTheme,
      highlightField,
      autocompletion({ override: [fernCompletionSource] }),
      fernLinter,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          if (debounceTimer) clearTimeout(debounceTimer);
          debounceTimer = setTimeout(() => {
            onChange(update.state.doc.toString());
          }, 300);
        }
        if (update.selectionSet && onCursorChange) {
          onCursorChange();
        }
      }),
    ],
  });

  return new EditorView({ state, parent: container });
}

/** Get the current code from the editor */
export function getCode(editor: EditorView): string {
  return editor.state.doc.toString();
}

/** Highlight a source span in the editor using a decoration mark */
export function highlightSpan(editor: EditorView, start: number, end: number): void {
  const docLen = editor.state.doc.length;
  const from = Math.min(start, docLen);
  const to = Math.min(end, docLen);
  editor.dispatch({
    effects: setHighlightEffect.of(from < to ? { from, to } : null),
    scrollIntoView: true,
  });
}

/** Clear the shape highlight decoration */
export function clearHighlightSpan(editor: EditorView): void {
  editor.dispatch({
    effects: setHighlightEffect.of(null),
  });
}

/** Get the current cursor offset in the editor */
export function getCursorOffset(editor: EditorView): number {
  return editor.state.selection.main.head;
}

/** Replace the entire document content */
export function setEditorContent(editor: EditorView, content: string): void {
  editor.dispatch({
    changes: { from: 0, to: editor.state.doc.length, insert: content },
  });
}

/** Get the current EditorState for later restoration */
export function getEditorState(editor: EditorView): EditorState {
  return editor.state;
}

/** Restore a previously saved EditorState */
export function setEditorState(editor: EditorView, state: EditorState): void {
  editor.setState(state);
}
