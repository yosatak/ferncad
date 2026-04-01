/**
 * Three.js 3D viewer
 *
 * Manages mesh display, OrbitControls, and lighting.
 * Supports per-part colored meshes for assemblies.
 */

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

/** Part mesh data */
export interface PartMeshData {
  name: string;
  positions: Float32Array;
  normals: Float32Array;
  color: [number, number, number];
  /** Source span start (byte offset) for editor↔viewer highlighting */
  spanStart: number;
  /** Source span end (byte offset) for editor↔viewer highlighting */
  spanEnd: number;
}

/** Span info for a part */
interface PartSpan {
  start: number;
  end: number;
}

/** 3D viewer class */
export class Viewer {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private partsGroup: THREE.Group;
  private raycaster: THREE.Raycaster;
  private highlightedMesh: THREE.Mesh | null = null;
  private partSpans: Map<string, PartSpan> = new Map();
  private mouseDownPos: { x: number; y: number } | null = null;

  /** Callback fired when a part is clicked in the viewer */
  onPartClick: ((span: PartSpan) => void) | null = null;

  constructor(canvas: HTMLCanvasElement) {
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x1a1a2e);

    this.camera = new THREE.PerspectiveCamera(
      45,
      canvas.clientWidth / canvas.clientHeight,
      0.1,
      10000,
    );
    this.camera.position.set(40, 30, 40);
    this.camera.lookAt(0, 0, 0);

    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    // Pass false to avoid setting CSS styles, which would override flex layout
    this.renderer.setSize(canvas.clientWidth, canvas.clientHeight, false);

    // Lighting
    this.scene.add(new THREE.AmbientLight(0x404040, 2));
    const dirLight1 = new THREE.DirectionalLight(0xffffff, 1.5);
    dirLight1.position.set(50, 80, 50);
    this.scene.add(dirLight1);
    const dirLight2 = new THREE.DirectionalLight(0x4488ff, 0.5);
    dirLight2.position.set(-30, -20, -50);
    this.scene.add(dirLight2);

    // Grid + axes
    const grid = new THREE.GridHelper(100, 20, 0x444466, 0x333355);
    grid.rotation.x = Math.PI / 2;
    this.scene.add(grid);
    this.scene.add(new THREE.AxesHelper(15));

    // OrbitControls
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.1;

    // Parts group
    this.partsGroup = new THREE.Group();
    this.scene.add(this.partsGroup);

    // Raycaster for part selection
    this.raycaster = new THREE.Raycaster();

    // Click detection (distinguish from orbit drag)
    canvas.addEventListener('mousedown', (e) => {
      this.mouseDownPos = { x: e.clientX, y: e.clientY };
    });
    canvas.addEventListener('mouseup', (e) => {
      if (!this.mouseDownPos) return;
      const dx = e.clientX - this.mouseDownPos.x;
      const dy = e.clientY - this.mouseDownPos.y;
      // Only treat as click if mouse didn't move much (not a drag)
      if (dx * dx + dy * dy < 9) {
        this.handleClick(e);
      }
      this.mouseDownPos = null;
    });

    // Resize — observe the canvas element. We use setSize with updateStyle=false
    // so the CSS flex layout controls the canvas dimensions, and we only update
    // the WebGL drawing buffer to match.
    const resizeObserver = new ResizeObserver(() => {
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      if (w <= 0 || h <= 0) return;
      this.camera.aspect = w / h;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(w, h, false);
    });
    resizeObserver.observe(canvas);

    this.animate();
  }

  /** Update with a single mesh (backward compatible) */
  updateMesh(positions: Float32Array, normals: Float32Array): void {
    this.updateParts([{
      name: 'shape',
      positions,
      normals,
      color: [0.53, 0.53, 0.80],
      spanStart: 0,
      spanEnd: 0,
    }]);
  }

  /** Update with per-part meshes */
  updateParts(parts: PartMeshData[]): void {
    this.clearMesh();
    this.clearHighlight();
    this.partSpans.clear();

    // Store span info for each part
    for (const part of parts) {
      this.partSpans.set(part.name, {
        start: part.spanStart,
        end: part.spanEnd,
      });
    }

    for (const part of parts) {
      if (part.positions.length === 0) continue;

      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.BufferAttribute(part.positions, 3));
      geometry.setAttribute('normal', new THREE.BufferAttribute(part.normals, 3));

      const color = new THREE.Color(part.color[0], part.color[1], part.color[2]);
      const material = new THREE.MeshStandardMaterial({
        color,
        metalness: 0.2,
        roughness: 0.6,
        side: THREE.DoubleSide,
        flatShading: true,
      });

      const mesh = new THREE.Mesh(geometry, material);
      mesh.name = part.name;
      this.partsGroup.add(mesh);

      // Wireframe
      const edges = new THREE.EdgesGeometry(geometry, 15);
      const edgeMat = new THREE.LineBasicMaterial({
        color: 0x444466,
        transparent: true,
        opacity: 0.3,
      });
      const wireframe = new THREE.LineSegments(edges, edgeMat);
      wireframe.name = `${part.name}-wireframe`;
      this.partsGroup.add(wireframe);
    }
  }

  /** Clear all meshes */
  clearMesh(): void {
    while (this.partsGroup.children.length > 0) {
      const child = this.partsGroup.children[0];
      this.partsGroup.remove(child);
      if (child instanceof THREE.Mesh) {
        child.geometry.dispose();
        (child.material as THREE.Material).dispose();
      }
      if (child instanceof THREE.LineSegments) {
        child.geometry.dispose();
        (child.material as THREE.Material).dispose();
      }
    }
  }

  /** Highlight a part by name */
  setHighlight(partName: string): void {
    this.clearHighlight();
    for (const child of this.partsGroup.children) {
      if (child instanceof THREE.Mesh && child.name === partName) {
        const mat = child.material as THREE.MeshStandardMaterial;
        mat.emissive.set(0x335599);
        mat.emissiveIntensity = 0.4;
        this.highlightedMesh = child;
        break;
      }
    }
  }

  /** Clear any part highlight */
  clearHighlight(): void {
    if (this.highlightedMesh) {
      const mat = this.highlightedMesh.material as THREE.MeshStandardMaterial;
      mat.emissive.set(0x000000);
      mat.emissiveIntensity = 0;
      this.highlightedMesh = null;
    }
  }

  /** Get span for a part name */
  getPartSpan(partName: string): PartSpan | undefined {
    return this.partSpans.get(partName);
  }

  /** Find which part contains a given source offset */
  findPartByOffset(offset: number): string | null {
    for (const [name, span] of this.partSpans) {
      if (offset >= span.start && offset < span.end) {
        return name;
      }
    }
    return null;
  }

  private handleClick(event: MouseEvent): void {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const mouse = new THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(mouse, this.camera);

    const meshes = this.partsGroup.children.filter(
      (c): c is THREE.Mesh => c instanceof THREE.Mesh,
    );
    const intersects = this.raycaster.intersectObjects(meshes);

    if (intersects.length > 0) {
      const mesh = intersects[0].object as THREE.Mesh;
      this.setHighlight(mesh.name);
      const span = this.partSpans.get(mesh.name);
      if (span && this.onPartClick) {
        this.onPartClick(span);
      }
    } else {
      this.clearHighlight();
    }
  }

  private animate = (): void => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };
}
