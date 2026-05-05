// Slim Three.js scene used by the LP hero. Reuses the editor viewer's lighting
// and grid look (web/src/viewer.ts), but drops OrbitControls input handling,
// raycaster, and span tracking — the LP scene is presentation-only.

import * as THREE from 'three';

export interface MeshPart {
  name: string;
  color: [number, number, number];
  positions: number[];
  normals: number[];
}

export interface MeshDoc {
  parts: MeshPart[];
}

export class HeroScene {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private partsGroup: THREE.Group;
  private rafId = 0;
  private rotating = true;
  private rotationY = 0;
  private rotationSpeed = 0.005;

  constructor(canvas: HTMLCanvasElement) {
    this.scene = new THREE.Scene();

    this.camera = new THREE.PerspectiveCamera(40, 1, 0.1, 1000);
    this.camera.position.set(60, 40, 60);
    this.camera.lookAt(0, 0, 0);

    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5));
    this.renderer.setSize(canvas.clientWidth, canvas.clientHeight, false);

    this.scene.add(new THREE.AmbientLight(0x404040, 2));
    const dirLight1 = new THREE.DirectionalLight(0xffffff, 1.5);
    dirLight1.position.set(50, 80, 50);
    this.scene.add(dirLight1);
    const dirLight2 = new THREE.DirectionalLight(0x4488ff, 0.5);
    dirLight2.position.set(-30, -20, -50);
    this.scene.add(dirLight2);

    const grid = new THREE.GridHelper(120, 24, 0x444466, 0x333355);
    grid.rotation.x = Math.PI / 2;
    grid.position.z = -25;
    const gridMat = grid.material as THREE.Material | THREE.Material[];
    if (Array.isArray(gridMat)) {
      gridMat.forEach((m) => {
        m.transparent = true;
        m.opacity = 0.45;
      });
    } else {
      gridMat.transparent = true;
      gridMat.opacity = 0.45;
    }
    this.scene.add(grid);

    this.partsGroup = new THREE.Group();
    this.scene.add(this.partsGroup);

    new ResizeObserver(() => {
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (w <= 0 || h <= 0) return;
      this.camera.aspect = w / h;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(w, h, false);
    }).observe(canvas);

    this.animate();
  }

  setMesh(doc: MeshDoc, opacity = 1): void {
    while (this.partsGroup.children.length > 0) {
      const child = this.partsGroup.children[0] as THREE.Mesh;
      this.partsGroup.remove(child);
      child.geometry.dispose();
      const mat = child.material as THREE.Material;
      mat.dispose();
    }

    const bbox = new THREE.Box3();
    for (const part of doc.parts) {
      if (part.positions.length === 0) continue;
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(part.positions), 3));
      geometry.setAttribute('normal', new THREE.BufferAttribute(new Float32Array(part.normals), 3));
      const material = new THREE.MeshStandardMaterial({
        color: new THREE.Color(part.color[0], part.color[1], part.color[2]),
        metalness: 0.3,
        roughness: 0.5,
        side: THREE.DoubleSide,
        flatShading: true,
        transparent: true,
        opacity,
      });
      const mesh = new THREE.Mesh(geometry, material);
      this.partsGroup.add(mesh);
      bbox.expandByObject(mesh);
    }

    if (!bbox.isEmpty()) {
      const center = bbox.getCenter(new THREE.Vector3());
      const size = bbox.getSize(new THREE.Vector3());
      const maxDim = Math.max(size.x, size.y, size.z);
      const dist = Math.max(maxDim * 1.6, 20);
      this.camera.position.set(dist, dist * 0.7, dist);
      this.camera.lookAt(0, 0, 0);
      this.partsGroup.position.copy(center.negate());
    }
  }

  fadeIn(durationMs = 600): Promise<void> {
    return new Promise((resolve) => {
      const startMs = performance.now();
      const tick = (): void => {
        const t = Math.min(1, (performance.now() - startMs) / durationMs);
        for (const child of this.partsGroup.children) {
          const mat = (child as THREE.Mesh).material as THREE.MeshStandardMaterial;
          mat.opacity = t;
        }
        if (t < 1) requestAnimationFrame(tick);
        else resolve();
      };
      requestAnimationFrame(tick);
    });
  }

  setRotating(rotating: boolean): void {
    this.rotating = rotating;
  }

  dispose(): void {
    cancelAnimationFrame(this.rafId);
    this.renderer.dispose();
  }

  private animate = (): void => {
    this.rafId = requestAnimationFrame(this.animate);
    if (this.rotating) {
      this.rotationY += this.rotationSpeed;
      this.partsGroup.rotation.y = this.rotationY;
    }
    this.renderer.render(this.scene, this.camera);
  };
}
