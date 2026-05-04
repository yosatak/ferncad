// Landing page entry point.
//
// Wires up the sections defined in index.html. i18n and shader-bg land in
// the next commits.

import { setupHero } from './hero';
import { setupFeatures } from './sections/features';

function init(): void {
  void setupHero();

  const featuresGrid = document.getElementById('features-grid');
  if (featuresGrid) setupFeatures(featuresGrid);

  // Code samples pull in CodeMirror; defer until the section scrolls in so
  // the LP first paint stays light.
  const samplesRoot = document.getElementById('samples-root');
  if (samplesRoot) {
    const observer = new IntersectionObserver((entries, obs) => {
      if (entries.some((e) => e.isIntersecting)) {
        obs.disconnect();
        void import('./code-sample').then((m) => m.setupCodeSamples(samplesRoot));
      }
    }, { rootMargin: '200px' });
    observer.observe(samplesRoot);
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
