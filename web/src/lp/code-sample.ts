// Code-and-3D tab section. Reuses the editor's syntax highlighting from
// web/src/editor.ts (StreamLanguage + HighlightStyle + dark theme), but
// loads CodeMirror in read-only mode and skips the linter / completion /
// WASM bridge so the LP doesn't pull in the editor runtime.

import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { syntaxHighlighting } from '@codemirror/language';

import { fernLanguage, fernHighlightStyle, darkTheme } from '../editor';
import { HeroScene, type MeshDoc } from './hero-scene';

interface Sample {
  id: string;
  label: string;
  code: string;
  meshUrl: string;
}

const SAMPLES: Sample[] = [
  {
    id: 'box',
    label: 'box',
    code: '(box :width 30 :depth 30 :height 30)',
    meshUrl: '/lp-mesh/samples/box.json',
  },
  {
    id: 'sphere',
    label: 'sphere',
    code: '(sphere :radius 18)',
    meshUrl: '/lp-mesh/samples/sphere.json',
  },
  {
    id: 'spur-gear',
    label: 'spur-gear',
    code: '(require :ferncad-std/spur-gear)\n\n(spur-gear :module 2.0\n           :teeth 18\n           :face-width 6.0\n           :bore 5.0)',
    meshUrl: '/lp-mesh/samples/spur-gear.json',
  },
  {
    id: 'boolean',
    label: 'CSG',
    code: '(difference\n  (box :width 24 :depth 24 :height 24)\n  (sphere :radius 14))',
    meshUrl: '/lp-mesh/samples/boolean.json',
  },
  {
    id: 'fern',
    label: 'fern',
    code: [
      ';; namesake of ferncad — every (pinna 1.0) returns the same Arc',
      '(defvar pinna',
      '  (memoize (lambda (len) ...)))',
      '',
      '(defun pinna-placements (n len)',
      '  (mapcar (lambda (i)',
      '            (let* ((tt (/ (+ i 1.0) (+ n 1.0))))',
      '              (list :z     (* tt len 0.92)',
      '                    :angle (deg ...)',
      '                    :scale ...)))',
      '          (iota n)))',
      '',
      '(union (rachis 12.0 0.14)',
      '       (apply union',
      '         (mapcar (lambda (p) (place-one (pinna 1.0) p))',
      '                 (pinna-placements 13 12.0))))',
    ].join('\n'),
    meshUrl: '/lp-mesh/samples/fern.json',
  },
];

const meshCache = new Map<string, Promise<MeshDoc | null>>();

function loadMesh(url: string): Promise<MeshDoc | null> {
  let cached = meshCache.get(url);
  if (!cached) {
    cached = fetch(url, { cache: 'force-cache' })
      .then((r) => (r.ok ? (r.json() as Promise<MeshDoc>) : null))
      .catch(() => null);
    meshCache.set(url, cached);
  }
  return cached;
}

export async function setupCodeSamples(root: HTMLElement): Promise<void> {
  root.innerHTML = '';

  const tabsEl = document.createElement('div');
  tabsEl.className = 'lp-samples-tabs';
  tabsEl.setAttribute('role', 'tablist');

  const bodyEl = document.createElement('div');
  bodyEl.className = 'lp-samples-body';

  const codeContainer = document.createElement('div');
  codeContainer.className = 'lp-samples-code';

  const viewerWrap = document.createElement('div');
  viewerWrap.className = 'lp-samples-viewer-wrap';
  const viewerCanvas = document.createElement('canvas');
  viewerWrap.appendChild(viewerCanvas);

  bodyEl.appendChild(codeContainer);
  bodyEl.appendChild(viewerWrap);

  root.appendChild(tabsEl);
  root.appendChild(bodyEl);

  let editorView: EditorView | null = null;
  function setCode(code: string): void {
    editorView?.destroy();
    editorView = new EditorView({
      state: EditorState.create({
        doc: code,
        extensions: [
          fernLanguage,
          syntaxHighlighting(fernHighlightStyle),
          darkTheme,
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
        ],
      }),
      parent: codeContainer,
    });
  }

  const scene = new HeroScene(viewerCanvas);
  scene.setRotating(true);

  // Pause when the section scrolls off-screen.
  const sectionEl = root.closest('.lp-samples');
  if (sectionEl) {
    new IntersectionObserver((entries) => {
      for (const entry of entries) {
        scene.setRotating(entry.isIntersecting);
      }
    }).observe(sectionEl);
  }

  let activeId = SAMPLES[0].id;

  for (const sample of SAMPLES) {
    const btn = document.createElement('button');
    btn.className = 'lp-samples-tab';
    btn.textContent = sample.label;
    btn.dataset.id = sample.id;
    btn.setAttribute('role', 'tab');
    btn.setAttribute('aria-selected', String(sample.id === activeId));
    btn.addEventListener('click', () => {
      void activate(sample.id);
    });
    tabsEl.appendChild(btn);
  }

  async function activate(id: string): Promise<void> {
    activeId = id;
    for (const btn of tabsEl.querySelectorAll<HTMLButtonElement>('.lp-samples-tab')) {
      btn.setAttribute('aria-selected', String(btn.dataset.id === id));
    }
    const sample = SAMPLES.find((s) => s.id === id);
    if (!sample) return;
    setCode(sample.code);
    const doc = await loadMesh(sample.meshUrl);
    if (doc && activeId === id) {
      scene.setMesh(doc, 1);
    }
  }

  await activate(activeId);
}
