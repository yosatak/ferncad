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
- `crates/ferncad-cad` — CAD カーネル: プリミティブ生成、BSP CSG (union/difference/intersection)、変換、STL エクスポート、ShapeNode→TriMesh 変換
- `crates/ferncad-wasm` — WASM バインディング（薄いラッパー: evaluate, export_stl, check_syntax）
- `crates/ferncad-cli` — CLI ツール

## 実装状態

### Phase 1 (完了)
- Lexer / Parser / Evaluator / 型定義
- プリミティブ: box, sphere, cylinder, cone, prism, torus
- CSG: union, difference, intersection (BSP ツリーベース)
- 変換: translate, rotate, scale
- defpart (:meta, :params, :body)
- STL エクスポート
- WASM バインディング + Web フロントエンド (CodeMirror 6 + Three.js)

### Phase 1 既知の制限
- BSP CSG はセグメント数が多いと遅い（デフォルト 16 に制限）
- defpart の :faces / :axes は未実装
- defmacro / quasiquote は未実装
- :: 型アノテーションはパースのみ（型チェックなし）

## 依存クレートバージョン

- logos 0.16, thiserror 2, ordered-float 5
- wasm-bindgen 0.2, js-sys 0.3
- truck は Phase 1 未使用（Phase 2 以降で検討）

## 判断に迷ったとき

1. 設計書（docs/design.md）に記述がある → それに従う
2. Common Lisp の慣習に倣える → 倣う
3. 判断できない → 最も単純な実装を選び `// TODO: 要レビュー` を残す
