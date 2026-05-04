# ferncad Tutorial

A guide to 3D CAD modeling with ferncad's Lisp-inspired syntax.

## Getting Started

```bash
# Build
cargo build --workspace

# Run web viewer
cd crates/ferncad-wasm && wasm-pack build --target web
cd web && npm install && npm run dev
# Landing page: http://localhost:5173/
# Editor:       http://localhost:5173/app/
```

## Basic Shapes

All primitives are centered at the origin.

```lisp
;; Box
(box :width 20 :depth 15 :height 10)

;; Sphere
(sphere :radius 10)

;; Cylinder
(cylinder :radius 5 :height 20)

;; Cone
(cone :radius-bottom 8 :radius-top 0 :height 15)

;; Torus
(torus :radius-major 15 :radius-minor 3)

;; Regular polygon prism (hexagon)
(prism :sides 6 :radius 10 :height 5)
```

## CSG Operations

Combine shapes with boolean operations:

```lisp
;; Union: combine shapes
(union
  (box :width 20 :depth 20 :height 10)
  (cylinder :radius 5 :height 20))

;; Difference: subtract shapes
(difference
  (box :width 20 :depth 20 :height 20)
  (sphere :radius 12))

;; Intersection: keep only overlapping volume
(intersection
  (box :width 15 :depth 15 :height 15)
  (sphere :radius 10))
```

## Transforms

```lisp
;; Move
(translate :shape (box :width 10 :depth 10 :height 10)
           :by #v(20 0 0))

;; Rotate (angle in radians, or use #a for degrees)
(rotate :shape (box :width 10 :depth 10 :height 10)
        :axis :z
        :angle #a(45 :deg))

;; Scale
(scale :shape (sphere :radius 5)
       :factor 2.0)
```

## Variables and Functions

```lisp
;; Constants (CL convention: +name+)
(defvar +plate-width+ 50)
(defvar +plate-height+ 5)

;; Functions
(defun make-plate (w d h)
  (box :width w :depth d :height h))

(make-plate +plate-width+ +plate-width+ +plate-height+)
```

## Defining Parts (defpart)

Reusable parametric parts with metadata:

```lisp
(defpart bracket
  "L-shaped bracket"
  :meta (:category :structural
         :material :aluminum
         :description "Simple L bracket")
  :params ((width  :: length :default 30.0 :doc "bracket width")
           (height :: length :default 40.0 :doc "bracket height")
           (thickness :: length :default 3.0 :doc "material thickness"))
  :body
  (let* ((base (box :width width :depth width :height thickness))
         (wall (box :width thickness :depth width :height height))
         (wall-placed (translate :shape wall
                                 :by (list (/ (- width thickness) 2) 0 (/ height 2)))))
    (union base wall-placed)))

;; Use with default parameters
(bracket)

;; Override parameters
(bracket :width 50 :height 60 :thickness 5)
```

## Extrude and Revolve

Create shapes from 2D profiles:

```lisp
;; Extrude a polygon along Z axis
(extrude
  :profile (polygon (list 0 0) (list 20 0) (list 20 10) (list 10 10) (list 10 5) (list 0 5))
  :height 15)

;; Revolve a profile around Z axis (creates a vase shape)
(revolve
  :profile (polygon (list 5 0) (list 10 0) (list 12 10) (list 8 20) (list 5 20))
  :angle #a(360 :deg))
```

## Paths and Sweep

Sweep a 2D profile along a 3D path to create complex shapes like threads, pipes, and springs:

```lisp
;; Circle profile helper
(circle :radius 2 :segments 16)

;; Sweep a circle along a helix → spring/coil
(sweep :profile (circle :radius 0.5 :segments 12)
       :path    (helix :radius 5 :pitch 3 :turns 4)
       :segments 128)

;; Sweep along a circular arc → pipe bend
(sweep :profile (circle :radius 1 :segments 12)
       :path    (arc :radius 10 :angle (/ pi 2))
       :segments 32)

;; Sweep along a Bezier curve
(sweep :profile (polygon (list -1 0) (list 0 1) (list 1 0))
       :path    (bezier :points (list (list 0 0 0)
                                      (list 10 10 0)
                                      (list 20 0 10)))
       :segments 48)
```

### Screw Thread Example

```lisp
(defvar +pitch+ 0.5)
(defvar +major-r+ 1.5)
(defvar +minor-r+ 1.221)

(let* ((length 10)
       (turns (/ length +pitch+))
       (tooth-h (- +major-r+ +minor-r+))
       (thread-profile (polygon
         (list 0.0 (- 0 (/ +pitch+ 4)))
         (list tooth-h 0.0)
         (list 0.0 (/ +pitch+ 4))))
       (thread (sweep :profile thread-profile
                      :path (helix :radius +minor-r+
                                   :pitch +pitch+
                                   :turns turns)
                      :segments (* turns 24)))
       (shaft (cylinder :radius +minor-r+ :height length))
       (shaft-up (translate :shape shaft
                            :by (list 0 0 (/ length 2)))))
  (union shaft-up thread))
```

## Loft

Interpolate between multiple 2D profiles at different Z positions:

```lisp
;; Transition from circle to square
(loft :profiles (list (circle :radius 5 :segments 32)
                      (polygon (list -3 -3) (list 3 -3)
                               (list 3 3) (list -3 3)))
      :at (list 0 20)
      :segments 16)

;; Bottle shape: three circular cross-sections
(loft :profiles (list (circle :radius 4 :segments 24)
                      (circle :radius 5 :segments 24)
                      (circle :radius 2 :segments 24))
      :at (list 0 10 30)
      :segments 16)
```

## Mesh Resolution

Control the fineness of curved surfaces:

```lisp
;; Set at the top of your .fern file
(defvar *resolution* 64)  ; default is 16
```

Or from the CLI:
```bash
ferncad model.fern --segments 64 --stl output.stl
```

Higher values produce smoother meshes but take longer to compute. Recommended:
- `16` — fast preview
- `32` — standard quality
- `64+` — 3D printing

## Macros

Define code transformations with `defmacro` and quasiquote:

```lisp
;; "when" macro (like Common Lisp)
(defmacro when (condition &rest body)
  `(if ,condition (progn ,@body)))

;; "repeat-around" macro: place shapes in a circular pattern
(defmacro with-offset (shape x y z)
  `(translate :shape ,shape :by (list ,x ,y ,z)))
```

## Assemblies

Combine multiple parts into assemblies:

```lisp
(require "ferncad-std/m3-bolt")

(assembly "plate-with-bolts"
  (place :part (box :width 40 :depth 40 :height 5) :as :plate)
  (place :part (m3-bolt :length 12) :as :bolt-1
         :at #p(10 10 5))
  (place :part (m3-bolt :length 12) :as :bolt-2
         :at #p(-10 -10 5)))
```

## Multi-File Projects

The web UI supports multi-file projects. Use the file explorer (left sidebar)
to create, rename, and delete `.fern` files within a project.

Files can reference each other with `require`:

```lisp
;; utils.fern
(defun plate (w d h)
  (box :width w :depth d :height h))

;; main.fern  (entry point — always evaluated first)
(require "utils")
(plate 40 40 5)
```

`require` resolves names in this order:
1. Project files (exact name, then with `.fern` suffix)
2. Built-in standard library modules

Projects are stored in the browser via IndexedDB. Use the toolbar buttons
to create, save, and open projects.

## Standard Library

Available modules (use with `require`):

| Module | Description |
|--------|-------------|
| `ferncad-std/m3-bolt` | M3 hex socket cap screw (JIS B 1176) |
| `ferncad-std/m3-nut` | M3 hex nut (JIS B 1181) |
| `ferncad-std/m4-bolt` | M4 hex socket cap screw |
| `ferncad-std/m5-bolt` | M5 hex socket cap screw |
| `ferncad-std/flat-washer` | Flat washer (JIS B 1256) |
| `ferncad-std/spring-washer` | Spring lock washer (JIS B 1251) |

## Export

- **STL**: Triangulated mesh for 3D printing
- **STEP**: Exact BREP geometry for CAD software exchange

Use the toolbar buttons or CLI:
```bash
ferncad input.fern --stl output.stl
ferncad input.fern --step output.step
ferncad input.fern --segments 64 --stl output.stl  # high quality
```

Assembly files automatically export per-part STL files:
```bash
ferncad assembly.fern --stl output.stl
# → output-bolt.stl, output-nut.stl, ...
```
