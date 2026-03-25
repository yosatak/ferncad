# ferncad Language Reference

## Types

| Type | Description | Example |
|------|-------------|---------|
| `int` | Integer | `42` |
| `float` | Floating-point | `3.14` |
| `length` | Length in mm | `#u(10 :mm)` |
| `angle` | Angle in rad | `#a(90 :deg)` |
| `vec3` | 3D vector | `#v(1 0 0)` |
| `point3` | 3D point | `#p(0 0 0)` |
| `string` | String | `"hello"` |
| `keyword` | Keyword | `:radius` |
| `bool` | Boolean | `t`, `nil` |
| `shape` | CSG shape | `(box ...)` |
| `path` | Parametric 3D curve | `(helix ...)` |
| `list` | List | `(list 1 2 3)` |

## Special Forms

| Form | Syntax | Description |
|------|--------|-------------|
| `defvar` | `(defvar name value)` | Define a variable |
| `defun` | `(defun name (params) body...)` | Define a function |
| `defmacro` | `(defmacro name (params) body...)` | Define a macro |
| `defpart` | `(defpart name "doc" :meta ... :params ... :body ...)` | Define a part |
| `defmeta` | `(defmeta :title ... :version ...)` | File metadata |
| `let*` | `(let* ((var val) ...) body...)` | Sequential local bindings |
| `if` | `(if cond then else)` | Conditional |
| `cond` | `(cond (test1 body1) (test2 body2) ...)` | Multi-branch conditional |
| `lambda` | `(lambda (params) body...)` | Anonymous function |
| `quote` | `(quote expr)` | Return unevaluated |
| `quasiquote` | `` `expr `` | Template with unquote |
| `progn` | `(progn body...)` | Evaluate sequentially |
| `assembly` | `(assembly "name" body...)` | Multi-part assembly |
| `require` | `(require "module-name")` | Load a module |
| `export` | `(export name1 name2 ...)` | Export symbols |

## Quasiquote Syntax

| Syntax | Expansion | Description |
|--------|-----------|-------------|
| `` `expr `` | `(quasiquote expr)` | Template |
| `,expr` | `(unquote expr)` | Evaluate and insert |
| `,@expr` | `(unquote-splicing expr)` | Evaluate and splice list |

## Primitives

| Function | Arguments | Description |
|----------|-----------|-------------|
| `box` | `:width :depth :height` | Rectangular box |
| `cube` | `size` | Cube (equal sides) |
| `sphere` | `:radius` | Sphere |
| `cylinder` | `:radius :height` | Cylinder |
| `cone` | `:radius-bottom :radius-top :height` | Cone/frustum |
| `torus` | `:radius-major :radius-minor` | Torus |
| `prism` | `:sides :radius :height` | Regular polygon prism |

## Profile Operations

| Function | Arguments | Description |
|----------|-----------|-------------|
| `polygon` | `(list x y) ...` | Create 2D point list |
| `circle` | `:radius :segments` | Circle polygon approximation |
| `extrude` | `:profile :height` | Extrude 2D profile along Z |
| `revolve` | `:profile :angle :segments` | Revolve 2D profile around Z |

## Path Constructors

| Function | Arguments | Description |
|----------|-----------|-------------|
| `helix` | `:radius :pitch :turns` | Helical path (spiral) |
| `arc` | `:radius :angle` | Circular arc in XY plane |
| `bezier` | `:points` | Bezier curve through control points |

## Sweep / Loft Operations

| Function | Arguments | Description |
|----------|-----------|-------------|
| `sweep` | `:profile :path :segments` | Sweep 2D profile along a 3D path |
| `loft` | `:profiles :at :segments` | Interpolate between multiple 2D profiles |

## CSG Operations

| Function | Arguments | Description |
|----------|-----------|-------------|
| `union` | `shape1 shape2 ...` | Boolean union |
| `difference` | `base cutter1 ...` | Boolean subtraction |
| `intersection` | `shape1 shape2 ...` | Boolean intersection |

## Transforms

| Function | Arguments | Description |
|----------|-----------|-------------|
| `translate` | `:shape :by` | Move shape |
| `rotate` | `:shape :axis :angle` | Rotate shape |
| `scale` | `:shape :factor` | Scale shape |

## Edge Operations

| Function | Status | Description |
|----------|--------|-------------|
| `chamfer` | Partial | `:shape :distance` (placeholder) |
| `fillet` | Not supported | truck 0.6 limitation |
| `shell` | Not supported | truck 0.6 limitation |

## Arithmetic

`+`, `-`, `*`, `/`, `cos`, `sin`, `sqrt`, `pi`

## Comparison

`=`, `<`, `>`, `<=`, `>=`, `not`

## Assembly

| Function | Arguments | Description |
|----------|-----------|-------------|
| `place` | `:part :as :at` | Place a part in assembly |
| `mate` | `face1 face2` | Align two faces |
| `align-axis` | `axis1 axis2` | Align two axes |
| `face` | `:part-name :face-name` | Reference a face |
| `axis` | `:part-name :axis-name` | Reference an axis |

## Unit Literals

```lisp
#u(10 :mm)    ; 10 mm
#u(1 :inch)   ; 25.4 mm
#u(2 :cm)     ; 20 mm
#a(90 :deg)   ; π/2 rad
#a(1 :turn)   ; 2π rad
#v(1 0 0)     ; 3D vector
#p(10 20 30)  ; 3D point
```

## Global Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `pi` | 3.14159... | Mathematical constant |
| `*resolution*` | `16` | Default segment count for curved surfaces |

Set `*resolution*` to control mesh quality:

```lisp
(defvar *resolution* 64)  ; smoother meshes (higher = finer, slower)
```

## CLI

```bash
ferncad <input.fern>                         # Evaluate and print
ferncad <input.fern> --stl <out.stl>         # Export STL (assembly → per-part files)
ferncad <input.fern> --step <out.step>       # Export STEP
ferncad <input.fern> --segments 64 --stl ... # Override mesh resolution
```
