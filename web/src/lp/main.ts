// Landing page entry point.

import { setupHero } from './hero';
import { setupFeatures } from './sections/features';
import { applyI18n, detectLang, setupLangToggle } from './i18n';

function init(): void {
  const lang = detectLang();
  applyI18n(lang);

  void setupHero();

  const heroEl = document.getElementById('hero');
  if (heroEl) {
    void import('./shader-bg').then((m) => m.setupShaderBg(heroEl));
  }

  const featuresGrid = document.getElementById('features-grid');
  if (featuresGrid) {
    setupFeatures(featuresGrid);
    applyI18n(lang, featuresGrid);
  }

  // Code samples pull in CodeMirror; defer until the section scrolls in so
  // the LP first paint stays light.
  const samplesRoot = document.getElementById('samples-root');
  if (samplesRoot) {
    const observer = new IntersectionObserver(
      (entries, obs) => {
        if (entries.some((e) => e.isIntersecting)) {
          obs.disconnect();
          void import('./code-sample').then((m) => m.setupCodeSamples(samplesRoot));
        }
      },
      { rootMargin: '200px' },
    );
    observer.observe(samplesRoot);
  }

  const langBtn = document.getElementById('lang-toggle');
  if (langBtn) setupLangToggle(langBtn, lang);
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
