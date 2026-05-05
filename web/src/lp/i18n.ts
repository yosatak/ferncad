// Tiny self-rolled i18n: walks `[data-i18n]` nodes and swaps text content.
// Attribute translations use `data-i18n-attr="<attr-name>"` alongside
// `data-i18n="<key>"`.

import { en } from './locales/en';
import { ja } from './locales/ja';

export type Lang = 'ja' | 'en';

const dict: Record<Lang, Record<string, string>> = { en, ja };
const STORAGE_KEY = 'ferncad-lang';

export function detectLang(): Lang {
  const fromQuery = new URLSearchParams(window.location.search).get('lang');
  if (fromQuery === 'ja' || fromQuery === 'en') return fromQuery;
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === 'ja' || stored === 'en') return stored;
  return navigator.language.toLowerCase().startsWith('ja') ? 'ja' : 'en';
}

export function applyI18n(lang: Lang, root: ParentNode = document): void {
  document.documentElement.lang = lang;
  const table = dict[lang];

  for (const el of root.querySelectorAll<HTMLElement>('[data-i18n]')) {
    const key = el.dataset.i18n;
    if (!key) continue;
    const val = table[key];
    if (val === undefined) continue;

    const attr = el.dataset.i18nAttr;
    if (attr) {
      el.setAttribute(attr, val);
    } else {
      el.textContent = val;
    }
  }

  if (table['meta.title']) {
    document.title = table['meta.title'];
  }
}

export function setLang(lang: Lang, persist = true): void {
  if (persist) {
    window.localStorage.setItem(STORAGE_KEY, lang);
    const url = new URL(window.location.href);
    url.searchParams.set('lang', lang);
    window.history.replaceState(null, '', url);
  }
  applyI18n(lang);
}

export function setupLangToggle(button: HTMLElement, initial: Lang): void {
  let current = initial;
  const update = (lang: Lang): void => {
    current = lang;
    button.textContent = lang === 'ja' ? 'EN' : 'JA';
  };
  update(initial);
  button.addEventListener('click', () => {
    const next: Lang = current === 'ja' ? 'en' : 'ja';
    setLang(next);
    update(next);
  });
}
