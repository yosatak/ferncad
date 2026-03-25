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

## Coding Conventions

- Error messages in English, include line/column numbers and suggestions where possible
- Public types and functions must have `///` doc comments
- No `unwrap()` outside `tests/` — use `?` + `thiserror`
- Use `f64` for numeric calculations (`f32` has insufficient precision)
- Define constants instead of magic numbers
- Booleans follow CL convention: `t`/`nil` are parsed as symbols by the Lexer and treated specially by the Evaluator

## Crate Structure

- `crates/ferncad-core` — Lexer (logos) / Parser (recursive descent) / Evaluator (tree-walk) / type definitions (Value, ShapeNode) — WASM-independent
- `crates/ferncad-cad` — CAD kernel: truck BREP (exact curves/surfaces), BSP CSG fallback, transforms, STL/STEP export, assembly mesh realization
- `crates/ferncad-wasm` — WASM bindings (evaluate, evaluate_parts, export_stl, export_step, check_syntax)
- `crates/ferncad-cli` — CLI tool
- `std/` — Standard library (.fern files, embedded via `include_str!` at build time)

## Implementation Status

### Phase 1 (complete)
- Lexer / Parser / Evaluator / type system
- Primitives: box, sphere, cylinder, cone, prism, torus
- CSG: union, difference, intersection (BSP tree-based)
- Transforms: translate, rotate, scale
- defpart (:meta, :params, :body)
- STL export
- WASM bindings + Web frontend (CodeMirror 6 + Three.js)

### Phase 2 (complete)
- `::` type annotation runtime checking (defpart parameter binding)
- defmeta special form
- defpart :faces / :axes declarations + face/axis builtins (FaceRef/AxisRef)
- assembly block + place + part instance management
- Constraints: mate, align-axis, fit, joint (direct transformation method)
- require/import module system (built-in library embedding)
- export declaration
- std/fasteners/m3-bolt.fern standard library
- Web UI: assembly tree display, per-part coloring

### BREP Migration (complete)
- truck BREP kernel for exact curved geometry (NURBS surfaces)
- All 6 primitives via BREP: box, sphere, cylinder, cone, prism, torus
- CSG via truck: union, intersection, difference (Solid::not + and)
- STEP export via truck-stepio
- BSP mesh fallback when truck boolean operations fail (catch_unwind)
- `segments` parameter preserved for backward compatibility but ignored by BREP

### Known Limitations
- Constraints use direct transformation (not a constraint solver)
- defmacro / quasiquote not yet implemented (Phase 3)
- require only works with built-in modules (file loading is CLI-only)
- truck boolean operations can fail on complex geometry; BSP fallback used automatically
- WASM binary is larger due to truck dependencies (~1MB vs ~240KB)

## Dependencies

- logos 0.16, thiserror 2
- wasm-bindgen 0.2, js-sys 0.3
- truck-modeling 0.6, truck-topology 0.6, truck-polymesh 0.6, truck-meshalgo 0.4, truck-shapeops 0.4, truck-stepio 0.3

## Decision Priority

1. Design doc (`docs/design.md`) has guidance → follow it
2. Common Lisp convention applies → follow it
3. Cannot decide → choose simplest implementation, leave `// TODO: needs review`
