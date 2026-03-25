# ferncad Tutorial

A guide to 3D CAD modeling with ferncad's Lisp-inspired syntax.

## Getting Started

```bash
# Build
cargo build --workspace

# Run web viewer
cd crates/ferncad-wasm && wasm-pack build --target web
cd web && npm install && npm run dev
# Open http://localhost:5173
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
```
