# CLAUDE.md

ferncad — A Rust project for 3D CAD modeling with Common Lisp-inspired syntax.

## Toolchain — Run via Docker

ホストには Rust ツールチェイン（cargo / rustc / wasm-pack 等）が**入っていない**。すべて `ferncad-dev` Docker イメージ経由で実行すること。リポジトリ直下の `execute_docker` ヘルパー（インタラクティブシェル用）と同等の起動方法を使う：

```bash
docker run --rm -v "$(pwd)":/workspace -w /workspace ferncad-dev <command>
```

ワンショットで叩く例：

```bash
docker run --rm -v "$(pwd)":/workspace -w /workspace ferncad-dev cargo fmt --all -- --check
docker run --rm -v "$(pwd)":/workspace -w /workspace ferncad-dev cargo clippy --workspace -- -D warnings
docker run --rm -v "$(pwd)":/workspace -w /workspace ferncad-dev cargo test --workspace
```

## Build & Test (inside docker)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## WASM Build (inside docker)

```bash
cd crates/ferncad-wasm && wasm-pack build --target web
```

WASM package is generated at `crates/ferncad-wasm/pkg/`. `web/pkg` is a symlink to it.

## Web Frontend

```bash
cd web && npm install && npm run build
```

Dev server: `cd web && npm run dev` → `localhost:5173`

- `/` — Landing page (`web/index.html`, entry `web/src/lp/main.ts`)
- `/app/` — Editor (`web/app/index.html`, entry `web/src/main.ts`)

`web/scripts/prebuild-meshes.mjs` runs as part of `npm run build` and shells
out to `cargo run -p ferncad-cli -- ... --mesh-json` to render the LP
hero/sample geometry into `web/public/lp-mesh/`. The output is gitignored;
both CI and Deploy workflows have the Rust toolchain installed before
`npm run build` so the prebuild succeeds out of the box.

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
- **extrude のキャップ三角形分割**: fan 分割は凸ポリゴン専用。歯車のような凹ポリゴンには ear-clipping を使う（`primitives.rs` の `ear_clip_triangulate`）。fan 分割に戻すと歯車の歯の谷で三角形がポリゴン外部を横切る
- **歯車プロファイルの走査方向**: CCW（反時計回り）走査が前提。1歯のプロファイルは「左フランク(負角度, 下→上) → 歯先 → 右フランク(正角度, 上→下)」の順。逆にすると歯底円弧と歯の接続点が歯の反対側にジャンプし斜め歯になる
- **歯底-ベース円の遷移**: インボリュート曲線はベース円 (r_base) から始まるが、歯底円 (r_dedendum) はベース円より小さい場合がある。遷移は歯底円弧の最終点 (r_ded) → フランク先頭 (r_base) の直接接続で暗黙的に生まれるため、明示的な遷移ポイントを追加してはならない。追加すると同一座標の重複頂点が生まれ、ear-clipping やBREP テッセレーションで歪みが発生する
- **realize パイプライン**: `realize()` は BREP 経路を優先する。BREP テッセレーションは元のポリゴン頂点とは異なるメッシュを生成するため、メッシュ頂点のインデックスで元ポリゴンを復元しようとしてはならない。プロファイル検証にはangular binningでの radial profile を使う

## Decision Priority

1. Design doc (`docs/design.md`) has guidance → follow it
2. Common Lisp convention applies → follow it
3. Cannot decide → choose simplest implementation, leave `// TODO: needs review`

## Dependencies

- logos 0.16, thiserror 2
- wasm-bindgen 0.2, js-sys 0.3
- truck-modeling 0.6, truck-topology 0.6, truck-polymesh 0.6, truck-meshalgo 0.4, truck-shapeops 0.4, truck-stepio 0.3
