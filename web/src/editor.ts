/**
 * CodeMirror 6 editor setup
 *
 * Provides Lisp-style syntax highlighting and bracket matching.
 */

import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { StreamLanguage } from '@codemirror/language';
import { tags } from '@lezer/highlight';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';

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
});

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
 * @returns EditorView instance
 */
export function createEditor(
  container: HTMLElement,
  onChange: (code: string) => void,
  initialCode?: string,
): EditorView {
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const state = EditorState.create({
    doc: initialCode ?? DEFAULT_CODE,
    extensions: [
      basicSetup,
      fernLanguage,
      syntaxHighlighting(fernHighlightStyle),
      darkTheme,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          if (debounceTimer) clearTimeout(debounceTimer);
          debounceTimer = setTimeout(() => {
            onChange(update.state.doc.toString());
          }, 300);
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
