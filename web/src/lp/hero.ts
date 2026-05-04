// Hero coordinator: types out a Lisp snippet and then fades in the prebuilt
// 3D mesh. The mesh JSON lives at /lp-mesh/hero.json (emitted by
// scripts/prebuild-meshes.mjs into web/public/lp-mesh/).

import { HeroScene, type MeshDoc } from './hero-scene';
import { typewrite } from './typewriter';

const HERO_SNIPPET =
  '(spur-gear\n  :teeth 22\n  :module 2.0\n  :face-width 8.0\n  :bore 6.0)';

const HERO_MESH_URL = '/lp-mesh/hero.json';

async function fetchMesh(url: string): Promise<MeshDoc | null> {
  try {
    const r = await fetch(url, { cache: 'force-cache' });
    if (!r.ok) return null;
    return (await r.json()) as MeshDoc;
  } catch {
    return null;
  }
}

export async function setupHero(): Promise<void> {
  const canvas = document.getElementById('hero-canvas') as HTMLCanvasElement | null;
  const codeEl = document.getElementById('hero-typewriter') as HTMLPreElement | null;
  const skipBtn = document.getElementById('hero-skip') as HTMLButtonElement | null;
  if (!canvas || !codeEl) return;

  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const isMobile = matchMedia('(max-width: 720px)').matches;

  const meshPromise = fetchMesh(HERO_MESH_URL);

  // Build the scene immediately so the canvas isn't blank during fetch.
  const scene = new HeroScene(canvas);

  if (isMobile) scene.setRotating(false);

  // Pause rotation when hero scrolls off-screen to save battery.
  const heroEl = canvas.closest('.lp-hero');
  if (heroEl) {
    new IntersectionObserver((entries) => {
      for (const entry of entries) {
        scene.setRotating(entry.isIntersecting && !isMobile);
      }
    }).observe(heroEl);
  }

  const mesh = await meshPromise;
  if (!mesh) {
    if (skipBtn) skipBtn.style.display = 'none';
    return;
  }

  if (reducedMotion) {
    codeEl.textContent = HERO_SNIPPET;
    scene.setMesh(mesh, 1);
    if (skipBtn) skipBtn.style.display = 'none';
    return;
  }

  scene.setMesh(mesh, 0);

  const ac = new AbortController();
  if (skipBtn) {
    skipBtn.style.display = '';
    skipBtn.addEventListener('click', () => ac.abort(), { once: true });
  }

  await typewrite({ text: HERO_SNIPPET, el: codeEl, cps: 28, signal: ac.signal });
  await scene.fadeIn(700);
  if (skipBtn) skipBtn.style.display = 'none';
}
