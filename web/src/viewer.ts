/**
 * Three.js 3D ビューア
 *
 * メッシュの表示、OrbitControls、ライティングを管理する。
 * アセンブリ時はパーツごとに色分けしたメッシュを表示する。
 */

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

/** パーツメッシュの情報 */
export interface PartMeshData {
  name: string;
  positions: Float32Array;
  normals: Float32Array;
  color: [number, number, number];
}

/** 3D ビューアクラス */
export class Viewer {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private partsGroup: THREE.Group;

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
    this.renderer.setSize(canvas.clientWidth, canvas.clientHeight);

    // ライティング
    this.scene.add(new THREE.AmbientLight(0x404040, 2));
    const dirLight1 = new THREE.DirectionalLight(0xffffff, 1.5);
    dirLight1.position.set(50, 80, 50);
    this.scene.add(dirLight1);
    const dirLight2 = new THREE.DirectionalLight(0x4488ff, 0.5);
    dirLight2.position.set(-30, -20, -50);
    this.scene.add(dirLight2);

    // グリッド + 軸
    const grid = new THREE.GridHelper(100, 20, 0x444466, 0x333355);
    grid.rotation.x = Math.PI / 2;
    this.scene.add(grid);
    this.scene.add(new THREE.AxesHelper(15));

    // OrbitControls
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.1;

    // パーツグループ
    this.partsGroup = new THREE.Group();
    this.scene.add(this.partsGroup);

    // リサイズ
    const resizeObserver = new ResizeObserver(() => {
      const w = canvas.clientWidth;
      const h = canvas.clientHeight;
      this.camera.aspect = w / h;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(w, h);
    });
    resizeObserver.observe(canvas);

    this.animate();
  }

  /** 単一メッシュを更新する（後方互換） */
  updateMesh(positions: Float32Array, normals: Float32Array): void {
    this.updateParts([{
      name: 'shape',
      positions,
      normals,
      color: [0.53, 0.53, 0.80],
    }]);
  }

  /** パーツごとのメッシュを更新する */
  updateParts(parts: PartMeshData[]): void {
    this.clearMesh();

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

      // ワイヤーフレーム
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

  /** メッシュをクリアする */
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

  private animate = (): void => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };
}
