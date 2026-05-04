// Subtle shader background behind the hero. A fullscreen quad with a fragment
// shader that draws a soft dot-grid + slow moving contour wash, parallaxed
// against scroll. Mobile / reduced-motion / low-CPU clients fall back to the
// plain page background (no canvas inserted).

import * as THREE from 'three';

const VERTEX = /* glsl */ `
  varying vec2 vUv;
  void main() {
    vUv = uv;
    gl_Position = vec4(position, 1.0);
  }
`;

const FRAGMENT = /* glsl */ `
  varying vec2 vUv;
  uniform float uTime;
  uniform float uScroll;
  uniform vec2  uResolution;

  void main() {
    vec2 uv = vUv;
    vec2 a = vec2(uv.x * uResolution.x / uResolution.y, uv.y + uScroll * 0.0005);

    // Dot grid (CAD blueprint feel)
    vec2 grid = fract(a * 22.0) - 0.5;
    float dot = smoothstep(0.06, 0.0, length(grid));

    // Slow contour wash
    float contour = 0.5 + 0.5 * sin(a.x * 6.0 + uTime * 0.18) * sin(a.y * 6.0 - uTime * 0.12);

    // Edge vignette so the canvas blends into the page bg
    float vig = smoothstep(0.0, 0.35, min(uv.x, 1.0 - uv.x))
              * smoothstep(0.0, 0.35, min(uv.y, 1.0 - uv.y));

    vec3 brand = vec3(0.545, 0.764, 0.290);
    vec3 blue  = vec3(0.337, 0.612, 0.839);
    vec3 col   = brand * dot * 0.22 * vig + blue * contour * 0.05 * vig;

    gl_FragColor = vec4(col, 1.0);
  }
`;

export function setupShaderBg(host: HTMLElement): boolean {
  const isMobile = matchMedia('(max-width: 880px)').matches;
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const lowSpec = (navigator.hardwareConcurrency ?? 4) < 4;
  if (isMobile || reducedMotion || lowSpec) return false;

  const canvas = document.createElement('canvas');
  canvas.className = 'lp-shader-bg';
  canvas.setAttribute('aria-hidden', 'true');
  host.insertBefore(canvas, host.firstChild);

  let renderer: THREE.WebGLRenderer;
  try {
    renderer = new THREE.WebGLRenderer({ canvas, alpha: false, antialias: false });
  } catch {
    canvas.remove();
    return false;
  }
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));

  const scene = new THREE.Scene();
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);

  const uniforms = {
    uTime: { value: 0 },
    uScroll: { value: 0 },
    uResolution: { value: new THREE.Vector2(1, 1) },
  };

  const material = new THREE.ShaderMaterial({
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
    uniforms,
    depthTest: false,
    depthWrite: false,
  });
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), material);
  scene.add(mesh);

  function resize(): void {
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (w <= 0 || h <= 0) return;
    renderer.setSize(w, h, false);
    uniforms.uResolution.value.set(w, h);
  }
  new ResizeObserver(resize).observe(canvas);
  resize();

  const onScroll = (): void => {
    uniforms.uScroll.value = window.scrollY;
  };
  window.addEventListener('scroll', onScroll, { passive: true });

  let running = true;
  let rafId = 0;
  const tick = (): void => {
    if (!running) return;
    rafId = requestAnimationFrame(tick);
    uniforms.uTime.value = performance.now() / 1000;
    renderer.render(scene, camera);
  };
  tick();

  new IntersectionObserver((entries) => {
    for (const entry of entries) {
      const next = entry.isIntersecting;
      if (next === running) continue;
      running = next;
      if (running) {
        tick();
      } else {
        cancelAnimationFrame(rafId);
      }
    }
  }).observe(host);

  return true;
}
