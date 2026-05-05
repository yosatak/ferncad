// Renders the "Why ferncad" feature card grid into the empty container in
// index.html. i18n keys are set up here so PR-D can swap labels without
// touching this code.

interface Feature {
  titleKey: string;
  descKey: string;
  titleEn: string;
  descEn: string;
}

const FEATURES: Feature[] = [
  {
    titleKey: 'features.lisp.title',
    titleEn: 'Lisp syntax',
    descKey: 'features.lisp.desc',
    descEn: 'Familiar S-expressions with macros, modules, and parametric variables.',
  },
  {
    titleKey: 'features.brep.title',
    titleEn: 'Exact BREP',
    descKey: 'features.brep.desc',
    descEn: 'Backed by truck for analytical curves and surfaces, with BSP fallback for stubborn CSG.',
  },
  {
    titleKey: 'features.export.title',
    titleEn: 'STL and STEP export',
    descKey: 'features.export.desc',
    descEn: 'STL for slicers, STEP for CAM and downstream CAD. Both one click.',
  },
  {
    titleKey: 'features.stdlib.title',
    titleEn: 'Standard library',
    descKey: 'features.stdlib.desc',
    descEn: 'JIS-spec spur and bevel gears, fasteners, and washers ready to drop in.',
  },
  {
    titleKey: 'features.assembly.title',
    titleEn: 'Assemblies',
    descKey: 'features.assembly.desc',
    descEn: 'Compose parts with named transforms; the viewer color-codes each piece.',
  },
  {
    titleKey: 'features.browser.title',
    titleEn: 'Runs in your browser',
    descKey: 'features.browser.desc',
    descEn: 'The Rust kernel ships as WebAssembly. No installation, your files stay local.',
  },
];

export function setupFeatures(root: HTMLElement): void {
  for (const f of FEATURES) {
    const card = document.createElement('div');
    card.className = 'lp-feature-card';

    const h3 = document.createElement('h3');
    h3.dataset.i18n = f.titleKey;
    h3.textContent = f.titleEn;

    const p = document.createElement('p');
    p.dataset.i18n = f.descKey;
    p.textContent = f.descEn;

    card.appendChild(h3);
    card.appendChild(p);
    root.appendChild(card);
  }
}
