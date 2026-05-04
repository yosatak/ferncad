// Landing page entry point.
//
// Wires up the sections defined in index.html. Hero ships in this commit;
// features, samples, i18n and shader-bg arrive in subsequent commits.

import { setupHero } from './hero';

function init(): void {
  void setupHero();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
