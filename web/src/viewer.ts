/**
 * Three.js 3D ビューア
 *
 * メッシュの表示、OrbitControls、ライティングを管理する。
 */

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

/** 3D ビューアクラス */
export class Viewer {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private mesh: THREE.Mesh | null = null;
  private wireframe: THREE.LineSegments | null = null;

  constructor(canvas: HTMLCanvasElement) {
    // シーン
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x1a1a2e);

    // カメラ
    this.camera = new THREE.PerspectiveCamera(
      45,
      canvas.clientWidth / canvas.clientHeight,
      0.1,
      10000,
    );
    this.camera.position.set(40, 30, 40);
    this.camera.lookAt(0, 0, 0);

    // レンダラー
    this.renderer = new THREE.WebGLRenderer({
      canvas,
      antialias: true,
    });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setSize(canvas.clientWidth, canvas.clientHeight);

    // ライティング
    const ambientLight = new THREE.AmbientLight(0x404040, 2);
    this.scene.add(ambientLight);

    const dirLight1 = new THREE.DirectionalLight(0xffffff, 1.5);
    dirLight1.position.set(50, 80, 50);
    this.scene.add(dirLight1);

    const dirLight2 = new THREE.DirectionalLight(0x4488ff, 0.5);
    dirLight2.position.set(-30, -20, -50);
    this.scene.add(dirLight2);

    // グリッド
    const grid = new THREE.GridHelper(100, 20, 0x444466, 0x333355);
    grid.rotation.x = Math.PI / 2; // XY平面に配置
    this.scene.add(grid);

    // 軸ヘルパー
    const axes = new THREE.AxesHelper(15);
    this.scene.add(axes);

    // OrbitControls
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.1;
    this.controls.target.set(0, 0, 0);

    // リサイズ対応
    const resizeObserver = new ResizeObserver(() => {
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      this.camera.aspect = width / height;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(width, height);
    });
    resizeObserver.observe(canvas);

    // アニメーションループ
    this.animate();
  }

  /** メッシュを更新する */
  updateMesh(positions: Float32Array, normals: Float32Array): void {
    // 既存のメッシュを削除
    if (this.mesh) {
      this.scene.remove(this.mesh);
      this.mesh.geometry.dispose();
      (this.mesh.material as THREE.Material).dispose();
    }
    if (this.wireframe) {
      this.scene.remove(this.wireframe);
      this.wireframe.geometry.dispose();
      (this.wireframe.material as THREE.Material).dispose();
    }

    if (positions.length === 0) return;

    // ジオメトリ作成
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3));

    // メッシュ作成
    const material = new THREE.MeshStandardMaterial({
      color: 0x8888cc,
      metalness: 0.2,
      roughness: 0.6,
      side: THREE.DoubleSide,
      flatShading: true,
    });
    this.mesh = new THREE.Mesh(geometry, material);
    this.scene.add(this.mesh);

    // ワイヤーフレーム
    const edgesGeometry = new THREE.EdgesGeometry(geometry, 15);
    const edgesMaterial = new THREE.LineBasicMaterial({
      color: 0x444466,
      transparent: true,
      opacity: 0.3,
    });
    this.wireframe = new THREE.LineSegments(edgesGeometry, edgesMaterial);
    this.scene.add(this.wireframe);
  }

  /** メッシュをクリアする */
  clearMesh(): void {
    if (this.mesh) {
      this.scene.remove(this.mesh);
      this.mesh.geometry.dispose();
      (this.mesh.material as THREE.Material).dispose();
      this.mesh = null;
    }
    if (this.wireframe) {
      this.scene.remove(this.wireframe);
      this.wireframe.geometry.dispose();
      (this.wireframe.material as THREE.Material).dispose();
      this.wireframe = null;
    }
  }

  /** アニメーションループ */
  private animate = (): void => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };
}
