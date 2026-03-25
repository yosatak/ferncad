/**
 * Parameter slider panel
 *
 * Dynamically generates sliders from defpart parameter specs.
 */

import type { ParamSpec } from './wasm-bridge';

/** Current slider overrides */
const overrides: Map<string, number> = new Map();

/** Create or update the slider panel from parameter specs */
export function updateSliders(
  container: HTMLElement,
  params: ParamSpec[],
  onChange: (overrides: Map<string, number>) => void,
): void {
  container.innerHTML = '';

  if (params.length === 0) {
    container.classList.add('hidden');
    return;
  }
  container.classList.remove('hidden');

  for (const param of params) {
    const current = overrides.get(param.name) ?? param.default;
    const min = 0;
    const max = Math.max(param.default * 3, 1);
    const step = param.type === 'angle' ? 1 : 0.1;

    const row = document.createElement('div');
    row.className = 'slider-row';

    const label = document.createElement('label');
    label.className = 'slider-label';
    label.textContent = param.name;
    if (param.doc) label.title = param.doc;

    const input = document.createElement('input');
    input.type = 'range';
    input.className = 'slider-input';
    input.min = String(min);
    input.max = String(max);
    input.step = String(step);
    input.value = String(current);

    const display = document.createElement('span');
    display.className = 'slider-value';
    display.textContent = current.toFixed(1);

    input.addEventListener('input', () => {
      const val = parseFloat(input.value);
      display.textContent = val.toFixed(1);
      overrides.set(param.name, val);
      onChange(overrides);
    });

    row.appendChild(label);
    row.appendChild(input);
    row.appendChild(display);
    container.appendChild(row);
  }
}

/** Generate code prefix with defvar overrides */
export function generateOverrides(overrides: Map<string, number>): string {
  if (overrides.size === 0) return '';
  const lines: string[] = [];
  for (const [name, value] of overrides) {
    lines.push(`(defvar ${name} ${value})`);
  }
  return lines.join('\n') + '\n';
}

/** Clear all overrides */
export function clearOverrides(): void {
  overrides.clear();
}
