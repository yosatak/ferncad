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

## Web フロントエンド

```bash
cd web && npm install && npm run build
```

## コーディング規約

- エラーメッセージは日本語で、行番号・列番号・修正提案を含める
- パブリックな型・関数には `///` doc コメントを書く
- `unwrap()` は `tests/` 以外で使わない。`?` + `thiserror` を使う
- 数値計算には `f64` を使う（`f32` は精度不足）
- マジックナンバーは定数として定義する

## クレート構成

- `crates/ferncad-core` — Lexer / Parser / Evaluator / 型定義（WASM非依存）
- `crates/ferncad-cad` — CAD カーネル統合（truck, CSG, STL エクスポート）
- `crates/ferncad-wasm` — WASM バインディング（薄いラッパー）
- `crates/ferncad-cli` — CLI ツール

## 判断に迷ったとき

1. 設計書（docs/design.md）に記述がある → それに従う
2. Common Lisp の慣習に倣える → 倣う
3. 判断できない → 最も単純な実装を選び `// TODO: 要レビュー` を残す
