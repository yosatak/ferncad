// Landing page entry point.
//
// This file wires up the sections defined in index.html. The hero, features,
// samples, i18n and shader-bg modules will land in subsequent commits — this
// initial commit ships the static shell so the rest can plug in incrementally.

function init(): void {
  // Skip button is only meaningful once hero animation arrives; hide it for
  // now so the shell looks intentional.
  const skip = document.getElementById('hero-skip');
  if (skip) skip.style.display = 'none';
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
