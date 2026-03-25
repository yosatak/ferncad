/**
 * CodeMirror 6 エディタ設定
 *
 * Lisp 風シンタックスハイライトと括弧マッチングを提供する。
 */

import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { StreamLanguage } from '@codemirror/language';
import { tags } from '@lezer/highlight';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';

/** ferncad 言語の StreamLanguage 定義 */
const fernLanguage = StreamLanguage.define({
  startState() {
    return { depth: 0 };
  },
  token(stream, _state) {
    // コメント
    if (stream.match(/;.*/)) {
      return 'lineComment';
    }
    // 空白スキップ
    if (stream.eatSpace()) {
      return null;
    }
    // 文字列
    if (stream.match(/"([^"\\]|\\.)*"/)) {
      return 'string';
    }
    // 特殊リテラルプレフィックス
    if (stream.match(/#[uavp]/)) {
      return 'meta';
    }
    // 型アノテーション
    if (stream.match(/::/)) {
      return 'operator';
    }
    // キーワード
    if (stream.match(/:[a-zA-Z][a-zA-Z0-9\-_/]*/)) {
      return 'atom';
    }
    // 数値
    if (stream.match(/-?[0-9]+(\.[0-9]*)?([eE][+-]?[0-9]+)?/)) {
      return 'number';
    }
    // 括弧
    if (stream.eat('(')) {
      return 'paren';
    }
    if (stream.eat(')')) {
      return 'paren';
    }
    // シンボル（定義キーワード）
    if (stream.match(/defpart|defun|defmacro|defvar|defmeta/)) {
      return 'definitionKeyword';
    }
    // シンボル（制御フロー）
    if (stream.match(/let\*|if|cond|quote|lambda|progn/)) {
      return 'keyword';
    }
    // シンボル（CSG）
    if (stream.match(/union|difference|intersection/)) {
      return 'operatorKeyword';
    }
    // シンボル（プリミティブ）
    if (stream.match(/box|sphere|cylinder|cone|torus|prism|cube/)) {
      return 'typeName';
    }
    // シンボル（変換）
    if (stream.match(/translate|rotate|scale|mirror/)) {
      return 'function';
    }
    // 一般シンボル
    if (stream.match(/[a-zA-Z_+*!?<>=][a-zA-Z0-9_+*\-/!?.<>=]*/)) {
      return 'variableName';
    }
    // その他の文字をスキップ
    stream.next();
    return null;
  },
});

/** ダークテーマのシンタックスハイライト */
const fernHighlightStyle = HighlightStyle.define([
  { tag: tags.lineComment, color: '#6a9955' },
  { tag: tags.string, color: '#ce9178' },
  { tag: tags.number, color: '#b5cea8' },
  { tag: tags.atom, color: '#9cdcfe' },        // :keywords
  { tag: tags.meta, color: '#c586c0' },         // #u, #a, etc
  { tag: tags.operator, color: '#d4d4d4' },
  { tag: tags.definitionKeyword, color: '#569cd6', fontWeight: 'bold' },
  { tag: tags.keyword, color: '#c586c0' },
  { tag: tags.operatorKeyword, color: '#dcdcaa', fontWeight: 'bold' },
  { tag: tags.typeName, color: '#4ec9b0' },
  { tag: tags.function(tags.name), color: '#dcdcaa' },
  { tag: tags.variableName, color: '#d4d4d4' },
  { tag: tags.paren, color: '#808080' },
]);

/** ダークテーマ */
const darkTheme = EditorView.theme({
  '&': {
    backgroundColor: '#1e1e1e',
    color: '#d4d4d4',
  },
  '.cm-content': {
    fontFamily: "'JetBrains Mono', 'Fira Code', 'Consolas', monospace",
    fontSize: '14px',
    caretColor: '#d4d4d4',
  },
  '.cm-gutters': {
    backgroundColor: '#1e1e1e',
    color: '#858585',
    border: 'none',
  },
  '.cm-activeLine': {
    backgroundColor: '#2a2d2e',
  },
  '.cm-activeLineGutter': {
    backgroundColor: '#2a2d2e',
  },
  '&.cm-focused .cm-cursor': {
    borderLeftColor: '#d4d4d4',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: '#264f78',
  },
  '.cm-matchingBracket': {
    backgroundColor: '#3e3e3e',
    outline: '1px solid #888',
  },
});

/** デフォルトのサンプルコード */
const DEFAULT_CODE = `;; ferncad — Lisp CAD Modeler
;; 評価ボタンまたは Ctrl+Enter で実行

;; 基本プリミティブ
; (box :width 20 :depth 20 :height 20)
; (sphere :radius 10)

;; CSG演算の例
(difference
  (box :width 20 :depth 20 :height 20)
  (sphere :radius 12))
`;

/**
 * CodeMirror エディタを作成する
 *
 * @param container - エディタを配置する DOM 要素
 * @param onChange - コード変更時のコールバック
 * @returns EditorView インスタンス
 */
export function createEditor(
  container: HTMLElement,
  onChange: (code: string) => void,
): EditorView {
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const state = EditorState.create({
    doc: DEFAULT_CODE,
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

  return new EditorView({
    state,
    parent: container,
  });
}

/** エディタの現在のコードを取得する */
export function getCode(editor: EditorView): string {
  return editor.state.doc.toString();
}
