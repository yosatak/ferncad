# CLAUDE.md

ferncad — Common Lisp インスパイアの構文で 3D CAD モデリングを行う Rust プロジェクト。

## ビルド・テスト

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## WASM ビルド

```bash
cd crates/ferncad-wasm && wasm-pack build --target web
```

WASM パッケージは `crates/ferncad-wasm/pkg/` に生成される。`web/pkg` はそこへのシンボリックリンク。

## Web フロントエンド

```bash
cd web && npm install && npm run build
```

開発サーバー: `cd web && npm run dev` → `localhost:5173`

## コーディング規約

- エラーメッセージは日本語で、行番号・列番号・修正提案を含める
- パブリックな型・関数には `///` doc コメントを書く
- `unwrap()` は `tests/` 以外で使わない。`?` + `thiserror` を使う
- 数値計算には `f64` を使う（`f32` は精度不足）
- マジックナンバーは定数として定義する
- ブール値は CL 準拠: `t`/`nil` はシンボルとして Lexer でパースし、Evaluator で特別扱い

## クレート構成

- `crates/ferncad-core` — Lexer (logos) / Parser (再帰下降) / Evaluator (ツリーウォーク) / 型定義 (Value, ShapeNode)（WASM非依存）
- `crates/ferncad-cad` — CAD カーネル: プリミティブ生成、BSP CSG、truck BREP パイプライン、変換、STL/STEP エクスポート、アセンブリメッシュ化
- `crates/ferncad-wasm` — WASM バインディング（evaluate, evaluate_parts, export_stl, export_step, check_syntax）
- `crates/ferncad-cli` — CLI ツール
- `std/` — 標準ライブラリ（.fern ファイル、ビルド時に include_str! で埋め込み）

## 実装状態

### Phase 1 (完了)
- Lexer / Parser / Evaluator / 型定義
- プリミティブ: box, sphere, cylinder, cone, prism, torus
- CSG: union, difference, intersection (BSP ツリーベース)
- 変換: translate, rotate, scale
- defpart (:meta, :params, :body)
- STL エクスポート
- WASM バインディング + Web フロントエンド (CodeMirror 6 + Three.js)

### Phase 2 (完了)
- truck BREP パイプライン (Box/Cylinder/Cone/Prism の BREP 変換、テッセレーション)
- :: 型アノテーションの実行時チェック (defpart パラメータバインド時)
- defmeta スペシャルフォーム
- defpart :faces / :axes 宣言 + face/axis 組み込み関数 (FaceRef/AxisRef)
- assembly ブロック + place + パーツインスタンス管理
- 制約: mate, align-axis, fit, joint（直接変換方式）
- STEP エクスポート (truck-stepio 経由、BREP 対応形状のみ)
- require/import モジュールシステム（標準ライブラリ埋め込み）
- export 宣言
- std/fasteners/m3-bolt.fern 標準ライブラリ
- Web UI: アセンブリツリー表示、パーツ色分け、STEP ダウンロードボタン

### Phase 2 既知の制限
- 球/トーラスの BREP 変換は未対応（STL では動作）
- BREP difference は未対応（BSP CSG でカバー）
- 制約は直接変換方式（制約ソルバーではない）
- defmacro / quasiquote は未実装（Phase 3）
- require はビルトインモジュールのみ（ファイル読み込みは CLI のみ予定）

## 依存クレートバージョン

- logos 0.16, thiserror 2, ordered-float 5
- truck-modeling 0.6, truck-shapeops 0.4, truck-meshalgo 0.4, truck-stepio 0.3
- wasm-bindgen 0.2, js-sys 0.3

## 判断に迷ったとき

1. 設計書（docs/design.md）に記述がある → それに従う
2. Common Lisp の慣習に倣える → 倣う
3. 判断できない → 最も単純な実装を選び `// TODO: 要レビュー` を残す
