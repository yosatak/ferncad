# CLAUDE.md

ferncad — A Rust project for 3D CAD modeling with Common Lisp-inspired syntax.

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## WASM Build

```bash
cd crates/ferncad-wasm && wasm-pack build --target web
```

WASM package is generated at `crates/ferncad-wasm/pkg/`. `web/pkg` is a symlink to it.

## Web Frontend

```bash
cd web && npm install && npm run build
```

Dev server: `cd web && npm run dev` → `localhost:5173`

## Crate Structure

- `crates/ferncad-core` — Lexer (logos) / Parser (recursive descent) / Evaluator (tree-walk) / type definitions (Value, ShapeNode) — WASM-independent
- `crates/ferncad-cad` — CAD kernel: truck BREP (exact curves/surfaces), BSP CSG fallback, transforms, STL/STEP export, assembly mesh realization
- `crates/ferncad-wasm` — WASM bindings (evaluate, evaluate_parts, export_stl, export_step, check_syntax)
- `crates/ferncad-cli` — CLI tool
- `std/` — Standard library (.fern files, embedded via `include_str!` at build time)

## Coding Conventions

- Error messages in English, include line/column numbers and suggestions where possible
- Public types and functions must have `///` doc comments
- No `unwrap()` outside `tests/` — use `?` + `thiserror`
- Use `f64` for numeric calculations (`f32` has insufficient precision)
- Define constants instead of magic numbers
- Booleans follow CL convention: `t`/`nil` are parsed as symbols by the Lexer and treated specially by the Evaluator

## Documentation

- 機能や動作を変更したら、関連ドキュメント (`docs/tutorial.md`, `docs/reference.md`) を同じ変更の中で必ず更新する
- CLAUDE.md自体もルールや注意事項が変わった場合に更新する（工程記録はgit logに任せる）

## Design Pitfalls — よくあるハマりどころ

- **truck boolean (CSG)** は複雑な形状で失敗しうる → `catch_unwind` + BSP mesh fallback を必ず経由させる
- **sweep のフレーム選択**: ヘリックスパスには解析的ラジアルフレームを使う（RMF はドリフトしてねじ山が歪む）。その他のパス (arc, bezier) にはRMFで良い
- **BSP union で共面 (coplanar) な面** があるとアーティファクトが出る → メッシュ同士の接触面にわずかなオーバーラップを持たせる
- **fillet / shell** は truck 0.6 では未サポート。エラーメッセージで案内する
- **Constraints** は直接変換方式（ソルバーなし）。複雑な拘束連鎖は対応できない

## Decision Priority

1. Design doc (`docs/design.md`) has guidance → follow it
2. Common Lisp convention applies → follow it
3. Cannot decide → choose simplest implementation, leave `// TODO: needs review`

## Dependencies

- logos 0.16, thiserror 2
- wasm-bindgen 0.2, js-sys 0.3
- truck-modeling 0.6, truck-topology 0.6, truck-polymesh 0.6, truck-meshalgo 0.4, truck-shapeops 0.4, truck-stepio 0.3
