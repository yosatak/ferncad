//! Completion support for ferncad editor
//!
//! Provides completion items for built-in functions, special forms,
//! keyword arguments, and user-defined symbols.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::types::Value;

/// Completion item kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    /// Built-in function
    Function,
    /// Keyword argument (e.g. :radius)
    Keyword,
    /// Constant value (e.g. pi)
    Constant,
    /// Special form (e.g. defvar, if)
    SpecialForm,
    /// User-defined variable (defvar)
    Variable,
    /// User-defined function (defun)
    UserFunction,
    /// Part definition (defpart)
    Part,
    /// Macro definition (defmacro)
    Macro,
}

impl CompletionKind {
    /// Convert to a string for JSON serialization
    fn as_str(&self) -> &'static str {
        match self {
            CompletionKind::Function => "function",
            CompletionKind::Keyword => "keyword",
            CompletionKind::Constant => "constant",
            CompletionKind::SpecialForm => "keyword",
            CompletionKind::Variable => "variable",
            CompletionKind::UserFunction => "function",
            CompletionKind::Part => "class",
            CompletionKind::Macro => "function",
        }
    }
}

/// A single completion item
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Display label
    pub label: String,
    /// Item kind
    pub kind: CompletionKind,
    /// Signature or type detail
    pub detail: String,
    /// Short documentation
    pub doc: String,
    /// Usage example (code snippet)
    pub example: String,
    /// Priority boost (higher = more prominent)
    pub boost: i32,
}

/// Keyword argument metadata
struct KwargMeta {
    name: &'static str,
    required: bool,
    doc: &'static str,
}

/// Static builtin completion entry
struct BuiltinEntry {
    label: &'static str,
    kind: CompletionKind,
    detail: &'static str,
    doc: &'static str,
    example: &'static str,
}

/// All built-in completions (functions, constants, special forms)
static BUILTIN_COMPLETIONS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    let entries: &[BuiltinEntry] = &[
        // Arithmetic
        BuiltinEntry { label: "+", kind: CompletionKind::Function, detail: "(+ a b ...)", doc: "Add numbers", example: "(+ 1 2 3) ; => 6" },
        BuiltinEntry { label: "-", kind: CompletionKind::Function, detail: "(- a b ...)", doc: "Subtract numbers or negate", example: "(- 10 3) ; => 7\n(- 5) ; => -5" },
        BuiltinEntry { label: "*", kind: CompletionKind::Function, detail: "(* a b ...)", doc: "Multiply numbers", example: "(* 2 3 4) ; => 24" },
        BuiltinEntry { label: "/", kind: CompletionKind::Function, detail: "(/ a b ...)", doc: "Divide numbers", example: "(/ 10 2) ; => 5" },
        // Comparison
        BuiltinEntry { label: "=", kind: CompletionKind::Function, detail: "(= a b ...)", doc: "Equality comparison", example: "(= 1 1) ; => t" },
        BuiltinEntry { label: "<", kind: CompletionKind::Function, detail: "(< a b ...)", doc: "Less than", example: "(< 1 2 3) ; => t" },
        BuiltinEntry { label: ">", kind: CompletionKind::Function, detail: "(> a b ...)", doc: "Greater than", example: "(> 3 2 1) ; => t" },
        BuiltinEntry { label: "<=", kind: CompletionKind::Function, detail: "(<= a b ...)", doc: "Less than or equal", example: "(<= 1 1 2) ; => t" },
        BuiltinEntry { label: ">=", kind: CompletionKind::Function, detail: "(>= a b ...)", doc: "Greater than or equal", example: "(>= 3 3 1) ; => t" },
        // Logic
        BuiltinEntry { label: "not", kind: CompletionKind::Function, detail: "(not x)", doc: "Logical negation", example: "(not nil) ; => t" },
        // List
        BuiltinEntry { label: "list", kind: CompletionKind::Function, detail: "(list a b ...)", doc: "Create a list", example: "(list 1 2 3) ; => (1 2 3)" },
        BuiltinEntry { label: "cons", kind: CompletionKind::Function, detail: "(cons item list)", doc: "Prepend item to list", example: "(cons 1 (list 2 3)) ; => (1 2 3)" },
        BuiltinEntry { label: "car", kind: CompletionKind::Function, detail: "(car list)", doc: "First element", example: "(car (list 1 2 3)) ; => 1" },
        BuiltinEntry { label: "cdr", kind: CompletionKind::Function, detail: "(cdr list)", doc: "Tail (all but first)", example: "(cdr (list 1 2 3)) ; => (2 3)" },
        BuiltinEntry { label: "append", kind: CompletionKind::Function, detail: "(append list1 list2 ...)", doc: "Concatenate lists", example: "(append (list 1 2) (list 3 4)) ; => (1 2 3 4)" },
        BuiltinEntry { label: "nth", kind: CompletionKind::Function, detail: "(nth n list)", doc: "Element at index n (0-based)", example: "(nth 1 (list 10 20 30)) ; => 20" },
        BuiltinEntry { label: "length", kind: CompletionKind::Function, detail: "(length list)", doc: "Number of elements", example: "(length (list 1 2 3)) ; => 3" },
        BuiltinEntry { label: "reverse", kind: CompletionKind::Function, detail: "(reverse list)", doc: "Reversed list", example: "(reverse (list 1 2 3)) ; => (3 2 1)" },
        BuiltinEntry { label: "last", kind: CompletionKind::Function, detail: "(last list)", doc: "Last element", example: "(last (list 1 2 3)) ; => 3" },
        // Math
        BuiltinEntry { label: "cos", kind: CompletionKind::Function, detail: "(cos x)", doc: "Cosine (radians)", example: "(cos 0) ; => 1.0" },
        BuiltinEntry { label: "sin", kind: CompletionKind::Function, detail: "(sin x)", doc: "Sine (radians)", example: "(sin (/ pi 2)) ; => 1.0" },
        BuiltinEntry { label: "tan", kind: CompletionKind::Function, detail: "(tan x)", doc: "Tangent (radians)", example: "(tan (/ pi 4)) ; => 1.0" },
        BuiltinEntry { label: "acos", kind: CompletionKind::Function, detail: "(acos x)", doc: "Arc cosine", example: "(acos 1) ; => 0.0" },
        BuiltinEntry { label: "atan", kind: CompletionKind::Function, detail: "(atan x)", doc: "Arc tangent", example: "(atan 1) ; => 0.785..." },
        BuiltinEntry { label: "atan2", kind: CompletionKind::Function, detail: "(atan2 y x)", doc: "Two-argument arc tangent", example: "(atan2 1 1) ; => 0.785..." },
        BuiltinEntry { label: "sqrt", kind: CompletionKind::Function, detail: "(sqrt x)", doc: "Square root", example: "(sqrt 9) ; => 3.0" },
        BuiltinEntry { label: "abs", kind: CompletionKind::Function, detail: "(abs x)", doc: "Absolute value", example: "(abs -5) ; => 5.0" },
        BuiltinEntry { label: "mod", kind: CompletionKind::Function, detail: "(mod a b)", doc: "Modulo (remainder)", example: "(mod 10 3) ; => 1.0" },
        BuiltinEntry { label: "expt", kind: CompletionKind::Function, detail: "(expt base power)", doc: "Exponentiation", example: "(expt 2 10) ; => 1024.0" },
        BuiltinEntry { label: "floor", kind: CompletionKind::Function, detail: "(floor x)", doc: "Round down", example: "(floor 3.7) ; => 3.0" },
        BuiltinEntry { label: "ceil", kind: CompletionKind::Function, detail: "(ceil x)", doc: "Round up", example: "(ceil 3.2) ; => 4.0" },
        BuiltinEntry { label: "min", kind: CompletionKind::Function, detail: "(min a b ...)", doc: "Minimum value", example: "(min 3 1 2) ; => 1.0" },
        BuiltinEntry { label: "max", kind: CompletionKind::Function, detail: "(max a b ...)", doc: "Maximum value", example: "(max 3 1 2) ; => 3.0" },
        // Type predicates
        BuiltinEntry { label: "numberp", kind: CompletionKind::Function, detail: "(numberp x)", doc: "Is x a number?", example: "(numberp 42) ; => t" },
        BuiltinEntry { label: "listp", kind: CompletionKind::Function, detail: "(listp x)", doc: "Is x a list?", example: "(listp (list 1 2)) ; => t" },
        BuiltinEntry { label: "nilp", kind: CompletionKind::Function, detail: "(nilp x)", doc: "Is x nil?", example: "(nilp nil) ; => t" },
        BuiltinEntry { label: "stringp", kind: CompletionKind::Function, detail: "(stringp x)", doc: "Is x a string?", example: "(stringp \"hi\") ; => t" },
        // String
        BuiltinEntry { label: "format", kind: CompletionKind::Function, detail: "(format template args...)", doc: "Format string (~a any, ~d int, ~f float, ~% newline)", example: "(format \"r=~a\" 10) ; => \"r=10\"" },
        // Constants
        BuiltinEntry { label: "pi", kind: CompletionKind::Constant, detail: "3.14159...", doc: "Mathematical constant pi", example: "(* 2 pi) ; full circle in radians" },
        BuiltinEntry { label: "*resolution*", kind: CompletionKind::Constant, detail: "16", doc: "Global segment count for curved shapes", example: "(defvar *resolution* 32) ; higher quality" },
        // Debug
        BuiltinEntry { label: "print", kind: CompletionKind::Function, detail: "(print x ...)", doc: "Print values to console", example: "(print \"radius =\" r)" },
        BuiltinEntry { label: "macroexpand-1", kind: CompletionKind::Function, detail: "(macroexpand-1 form)", doc: "Expand a macro once", example: "(macroexpand-1 '(my-macro x))" },
        // CAD Primitives
        BuiltinEntry { label: "box", kind: CompletionKind::Function, detail: "(box :width W :depth D :height H)", doc: "Create a box", example: "(box :width 20 :depth 15 :height 10)" },
        BuiltinEntry { label: "cube", kind: CompletionKind::Function, detail: "(cube size)", doc: "Create a cube (all sides equal)", example: "(cube 20)" },
        BuiltinEntry { label: "sphere", kind: CompletionKind::Function, detail: "(sphere :radius R [:segments N])", doc: "Create a sphere", example: "(sphere :radius 10)" },
        BuiltinEntry { label: "cylinder", kind: CompletionKind::Function, detail: "(cylinder :radius R :height H [:segments N])", doc: "Create a cylinder", example: "(cylinder :radius 5 :height 20)" },
        BuiltinEntry { label: "cone", kind: CompletionKind::Function, detail: "(cone :radius-bottom R :height H [:radius-top R2] [:segments N])", doc: "Create a cone or frustum", example: "(cone :radius-bottom 10 :radius-top 3 :height 15)" },
        BuiltinEntry { label: "prism", kind: CompletionKind::Function, detail: "(prism :sides N :radius R :height H)", doc: "Create a regular prism", example: "(prism :sides 6 :radius 10 :height 20)" },
        BuiltinEntry { label: "torus", kind: CompletionKind::Function, detail: "(torus :radius-major R1 :radius-minor R2 [:segments N])", doc: "Create a torus", example: "(torus :radius-major 15 :radius-minor 3)" },
        // CSG
        BuiltinEntry { label: "union", kind: CompletionKind::Function, detail: "(union shape1 shape2 ...)", doc: "Boolean union of shapes", example: "(union\n  (box :width 20 :depth 20 :height 20)\n  (sphere :radius 12))" },
        BuiltinEntry { label: "difference", kind: CompletionKind::Function, detail: "(difference base cutter ...)", doc: "Boolean subtraction", example: "(difference\n  (box :width 20 :depth 20 :height 20)\n  (sphere :radius 12))" },
        BuiltinEntry { label: "intersection", kind: CompletionKind::Function, detail: "(intersection shape1 shape2 ...)", doc: "Boolean intersection", example: "(intersection\n  (box :width 20 :depth 20 :height 20)\n  (sphere :radius 14))" },
        // Transforms
        BuiltinEntry { label: "translate", kind: CompletionKind::Function, detail: "(translate :shape S :by #v(x y z))", doc: "Move a shape", example: "(translate\n  :shape (sphere :radius 5)\n  :by #v(10 0 0))" },
        BuiltinEntry { label: "rotate", kind: CompletionKind::Function, detail: "(rotate :shape S :axis :x/:y/:z :angle A)", doc: "Rotate a shape", example: "(rotate\n  :shape (box :width 10 :depth 10 :height 10)\n  :axis :z :angle #a(45 :deg))" },
        BuiltinEntry { label: "scale", kind: CompletionKind::Function, detail: "(scale :shape S :factor F | :x X :y Y :z Z)", doc: "Scale a shape uniformly or per-axis", example: "(scale\n  :shape (cube 10)\n  :factor 2)" },
        // Unit conversion
        BuiltinEntry { label: "to-mm", kind: CompletionKind::Function, detail: "(to-mm value)", doc: "Convert to millimeters", example: "(to-mm #u(1 :inch)) ; => 25.4" },
        BuiltinEntry { label: "to-rad", kind: CompletionKind::Function, detail: "(to-rad value)", doc: "Convert to radians", example: "(to-rad #a(180 :deg)) ; => 3.14159..." },
        // Profiles
        BuiltinEntry { label: "polygon", kind: CompletionKind::Function, detail: "(polygon (list x y) ...)", doc: "Create a 2D polygon from points", example: "(polygon\n  (list 0 0) (list 10 0)\n  (list 10 10) (list 0 10))" },
        BuiltinEntry { label: "circle", kind: CompletionKind::Function, detail: "(circle :radius R [:segments N])", doc: "Create a circular polygon", example: "(circle :radius 5 :segments 32)" },
        BuiltinEntry { label: "extrude", kind: CompletionKind::Function, detail: "(extrude :profile P :height H)", doc: "Extrude a 2D profile along Z", example: "(extrude\n  :profile (circle :radius 5)\n  :height 20)" },
        BuiltinEntry { label: "revolve", kind: CompletionKind::Function, detail: "(revolve :profile P [:angle A] [:segments N])", doc: "Revolve a 2D profile around Z", example: "(revolve\n  :profile (polygon (list 5 0) (list 10 0) (list 10 5))\n  :angle #a(360 :deg))" },
        // Paths
        BuiltinEntry { label: "helix", kind: CompletionKind::Function, detail: "(helix :radius R :pitch P :turns N)", doc: "Create a helical path", example: "(helix :radius 10 :pitch 5 :turns 3)" },
        BuiltinEntry { label: "arc", kind: CompletionKind::Function, detail: "(arc :radius R [:angle A])", doc: "Create a circular arc path", example: "(arc :radius 20 :angle #a(180 :deg))" },
        BuiltinEntry { label: "bezier", kind: CompletionKind::Function, detail: "(bezier :points (list ...))", doc: "Create a Bezier curve path", example: "(bezier :points (list\n  #v(0 0 0) #v(10 10 5) #v(20 0 10)))" },
        // Sweep/Loft
        BuiltinEntry { label: "sweep", kind: CompletionKind::Function, detail: "(sweep :profile P :path PATH [:segments N])", doc: "Sweep profile along path", example: "(sweep\n  :profile (circle :radius 2)\n  :path (helix :radius 10 :pitch 5 :turns 3))" },
        BuiltinEntry { label: "loft", kind: CompletionKind::Function, detail: "(loft :profiles (P1 P2 ...) :at (Z1 Z2 ...) [:segments N])", doc: "Loft between profiles", example: "(loft\n  :profiles (list\n    (circle :radius 10)\n    (circle :radius 5))\n  :at (list 0 20))" },
        // Edge operations
        BuiltinEntry { label: "chamfer", kind: CompletionKind::Function, detail: "(chamfer :shape S :distance D)", doc: "Chamfer all edges", example: "(chamfer\n  :shape (cube 20)\n  :distance 2)" },
        BuiltinEntry { label: "fillet", kind: CompletionKind::Function, detail: "(fillet :shape S :radius R)", doc: "Fillet edges (not yet supported in truck 0.6)", example: "" },
        BuiltinEntry { label: "shell", kind: CompletionKind::Function, detail: "(shell :shape S :thickness T)", doc: "Shell a solid (not yet supported in truck 0.6)", example: "" },
        // Assembly
        BuiltinEntry { label: "place", kind: CompletionKind::Function, detail: "(place :part P :as :name [:at #p(x y z)])", doc: "Place a part instance in assembly", example: "(place :part my-bolt :as :bolt1 :at #p(0 0 10))" },
        BuiltinEntry { label: "face", kind: CompletionKind::Function, detail: "(face :instance :face-name)", doc: "Reference a face on a part instance", example: "(face :bolt1 :head-top)" },
        BuiltinEntry { label: "axis", kind: CompletionKind::Function, detail: "(axis :instance :axis-name)", doc: "Reference an axis on a part instance", example: "(axis :bolt1 :center)" },
        BuiltinEntry { label: "mate", kind: CompletionKind::Function, detail: "(mate face-ref1 face-ref2)", doc: "Constrain two faces to meet", example: "(mate\n  (face :bolt1 :head-bottom)\n  (face :plate1 :top))" },
        BuiltinEntry { label: "align-axis", kind: CompletionKind::Function, detail: "(align-axis axis-ref1 axis-ref2)", doc: "Align two axes", example: "(align-axis\n  (axis :bolt1 :center)\n  (axis :plate1 :hole))" },
        BuiltinEntry { label: "fit", kind: CompletionKind::Function, detail: "(fit :shaft S :hole H)", doc: "Shaft-hole fit constraint", example: "(fit\n  :shaft (axis :bolt1 :center)\n  :hole (axis :plate1 :hole))" },
        BuiltinEntry { label: "joint", kind: CompletionKind::Function, detail: "(joint [:type :fixed] [:parts ()])", doc: "Define a joint", example: "(joint :type :fixed :parts (list :bolt1 :plate1))" },
        // Special forms
        BuiltinEntry { label: "defvar", kind: CompletionKind::SpecialForm, detail: "(defvar name value)", doc: "Define a global variable", example: "(defvar my-radius 10)" },
        BuiltinEntry { label: "defun", kind: CompletionKind::SpecialForm, detail: "(defun name (params) body ...)", doc: "Define a function", example: "(defun double (x)\n  (* x 2))" },
        BuiltinEntry { label: "defpart", kind: CompletionKind::SpecialForm, detail: "(defpart name \"doc\" :params (...) :body (...))", doc: "Define a parametric part", example: "(defpart my-bolt \"A bolt\"\n  :params ((length :: length :default 16 :doc \"Bolt length\"))\n  :body ((cylinder :radius 3 :height length)))" },
        BuiltinEntry { label: "defmacro", kind: CompletionKind::SpecialForm, detail: "(defmacro name (params) body ...)", doc: "Define a macro", example: "(defmacro when (test body)\n  `(if ,test ,body nil))" },
        BuiltinEntry { label: "defmeta", kind: CompletionKind::SpecialForm, detail: "(defmeta :key value ...)", doc: "Define metadata", example: "(defmeta :name \"My Model\" :author \"user\")" },
        BuiltinEntry { label: "assembly", kind: CompletionKind::SpecialForm, detail: "(assembly \"name\" \"doc\" body ...)", doc: "Define an assembly", example: "(assembly \"bracket\"\n  \"A bracket assembly\"\n  (place :part bolt :as :b1)\n  (place :part plate :as :p1))" },
        BuiltinEntry { label: "let*", kind: CompletionKind::SpecialForm, detail: "(let* ((name value) ...) body ...)", doc: "Sequential local bindings", example: "(let* ((r 10)\n       (h (* r 2)))\n  (cylinder :radius r :height h))" },
        BuiltinEntry { label: "if", kind: CompletionKind::SpecialForm, detail: "(if condition then else)", doc: "Conditional expression", example: "(if (> r 10)\n  (sphere :radius r)\n  (cube r))" },
        BuiltinEntry { label: "cond", kind: CompletionKind::SpecialForm, detail: "(cond (test1 expr1) (test2 expr2) ...)", doc: "Multi-way conditional", example: "(cond\n  ((= n 3) (prism :sides 3 :radius r :height h))\n  ((= n 4) (cube r))\n  (t (cylinder :radius r :height h)))" },
        BuiltinEntry { label: "when", kind: CompletionKind::SpecialForm, detail: "(when condition body ...)", doc: "Execute body if condition is truthy", example: "(when (> r 10)\n  (print \"large radius\")\n  (sphere :radius r))" },
        BuiltinEntry { label: "unless", kind: CompletionKind::SpecialForm, detail: "(unless condition body ...)", doc: "Execute body if condition is falsy", example: "(unless (nilp x)\n  (print x))" },
        BuiltinEntry { label: "and", kind: CompletionKind::SpecialForm, detail: "(and expr ...)", doc: "Short-circuit logical and", example: "(and (> x 0) (< x 100))" },
        BuiltinEntry { label: "or", kind: CompletionKind::SpecialForm, detail: "(or expr ...)", doc: "Short-circuit logical or", example: "(or default-value (compute-value))" },
        BuiltinEntry { label: "setf", kind: CompletionKind::SpecialForm, detail: "(setf name value)", doc: "Mutate an existing variable", example: "(defvar x 0)\n(setf x 42)" },
        BuiltinEntry { label: "dotimes", kind: CompletionKind::SpecialForm, detail: "(dotimes (var count) body ...)", doc: "Loop from 0 to count-1", example: "(dotimes (i 10)\n  (print i))" },
        BuiltinEntry { label: "dolist", kind: CompletionKind::SpecialForm, detail: "(dolist (var list) body ...)", doc: "Iterate over list elements", example: "(dolist (x (list 1 2 3))\n  (print x))" },
        BuiltinEntry { label: "mapcar", kind: CompletionKind::SpecialForm, detail: "(mapcar fn list)", doc: "Apply function to each element", example: "(mapcar (lambda (x) (* x 2)) (list 1 2 3))" },
        BuiltinEntry { label: "reduce", kind: CompletionKind::SpecialForm, detail: "(reduce fn list [initial])", doc: "Fold over list elements", example: "(reduce + (list 1 2 3 4)) ; => 10" },
        BuiltinEntry { label: "remove-if", kind: CompletionKind::SpecialForm, detail: "(remove-if predicate list)", doc: "Remove elements matching predicate", example: "(remove-if (lambda (x) (> x 3)) (list 1 2 3 4 5))" },
        BuiltinEntry { label: "apply", kind: CompletionKind::SpecialForm, detail: "(apply fn args-list)", doc: "Apply function to argument list", example: "(apply + (list 1 2 3)) ; => 6" },
        BuiltinEntry { label: "lambda", kind: CompletionKind::SpecialForm, detail: "(lambda (params) body ...)", doc: "Anonymous function", example: "(defvar scale-shape\n  (lambda (s factor)\n    (scale :shape s :factor factor)))" },
        BuiltinEntry { label: "progn", kind: CompletionKind::SpecialForm, detail: "(progn expr1 expr2 ...)", doc: "Evaluate expressions sequentially", example: "(progn\n  (print \"building...\")\n  (cube 20))" },
        BuiltinEntry { label: "begin", kind: CompletionKind::SpecialForm, detail: "(begin expr1 expr2 ...)", doc: "Evaluate expressions sequentially", example: "(begin\n  (defvar x 10)\n  (cube x))" },
        BuiltinEntry { label: "quote", kind: CompletionKind::SpecialForm, detail: "(quote expr)", doc: "Return expression without evaluating", example: "(quote (+ 1 2)) ; => (+ 1 2)" },
        BuiltinEntry { label: "quasiquote", kind: CompletionKind::SpecialForm, detail: "(quasiquote expr)", doc: "Template with unquote", example: "`(cylinder :radius ,r :height ,h)" },
        BuiltinEntry { label: "require", kind: CompletionKind::SpecialForm, detail: "(require \"module\")", doc: "Load a module", example: "(require \"fasteners/m3-bolt\")" },
        BuiltinEntry { label: "export", kind: CompletionKind::SpecialForm, detail: "(export name)", doc: "Export a symbol", example: "(export my-part)" },
    ];

    entries
        .iter()
        .map(|e| CompletionItem {
            label: e.label.to_string(),
            kind: e.kind,
            detail: e.detail.to_string(),
            doc: e.doc.to_string(),
            example: e.example.to_string(),
            boost: 0,
        })
        .collect()
});

/// Keyword argument table: function name → list of keyword args
static KWARG_TABLE: LazyLock<HashMap<&'static str, Vec<KwargMeta>>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(
        "box",
        vec![
            KwargMeta {
                name: ":width",
                required: true,
                doc: "Width (X axis)",
            },
            KwargMeta {
                name: ":depth",
                required: true,
                doc: "Depth (Y axis)",
            },
            KwargMeta {
                name: ":height",
                required: true,
                doc: "Height (Z axis)",
            },
        ],
    );
    m.insert(
        "sphere",
        vec![
            KwargMeta {
                name: ":radius",
                required: true,
                doc: "Sphere radius",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Tessellation segments",
            },
        ],
    );
    m.insert(
        "cylinder",
        vec![
            KwargMeta {
                name: ":radius",
                required: true,
                doc: "Cylinder radius",
            },
            KwargMeta {
                name: ":height",
                required: true,
                doc: "Cylinder height",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Tessellation segments",
            },
        ],
    );
    m.insert(
        "cone",
        vec![
            KwargMeta {
                name: ":radius-bottom",
                required: true,
                doc: "Bottom radius",
            },
            KwargMeta {
                name: ":height",
                required: true,
                doc: "Cone height",
            },
            KwargMeta {
                name: ":radius-top",
                required: false,
                doc: "Top radius (0 for a point)",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Tessellation segments",
            },
        ],
    );
    m.insert(
        "prism",
        vec![
            KwargMeta {
                name: ":sides",
                required: true,
                doc: "Number of sides",
            },
            KwargMeta {
                name: ":radius",
                required: true,
                doc: "Circumscribed radius",
            },
            KwargMeta {
                name: ":height",
                required: true,
                doc: "Prism height",
            },
        ],
    );
    m.insert(
        "torus",
        vec![
            KwargMeta {
                name: ":radius-major",
                required: true,
                doc: "Distance from center to tube center",
            },
            KwargMeta {
                name: ":radius-minor",
                required: true,
                doc: "Tube radius",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Tessellation segments",
            },
        ],
    );
    m.insert(
        "translate",
        vec![
            KwargMeta {
                name: ":shape",
                required: true,
                doc: "Shape to translate",
            },
            KwargMeta {
                name: ":by",
                required: true,
                doc: "Translation vector #v(x y z)",
            },
        ],
    );
    m.insert(
        "rotate",
        vec![
            KwargMeta {
                name: ":shape",
                required: true,
                doc: "Shape to rotate",
            },
            KwargMeta {
                name: ":axis",
                required: true,
                doc: "Rotation axis (:x, :y, :z, or #v(...))",
            },
            KwargMeta {
                name: ":angle",
                required: true,
                doc: "Rotation angle (radians)",
            },
        ],
    );
    m.insert(
        "scale",
        vec![
            KwargMeta {
                name: ":shape",
                required: true,
                doc: "Shape to scale",
            },
            KwargMeta {
                name: ":factor",
                required: false,
                doc: "Uniform scale factor",
            },
            KwargMeta {
                name: ":x",
                required: false,
                doc: "X scale factor",
            },
            KwargMeta {
                name: ":y",
                required: false,
                doc: "Y scale factor",
            },
            KwargMeta {
                name: ":z",
                required: false,
                doc: "Z scale factor",
            },
        ],
    );
    m.insert(
        "circle",
        vec![
            KwargMeta {
                name: ":radius",
                required: true,
                doc: "Circle radius",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Number of polygon segments",
            },
        ],
    );
    m.insert(
        "extrude",
        vec![
            KwargMeta {
                name: ":profile",
                required: true,
                doc: "2D polygon profile",
            },
            KwargMeta {
                name: ":height",
                required: true,
                doc: "Extrusion height",
            },
        ],
    );
    m.insert(
        "revolve",
        vec![
            KwargMeta {
                name: ":profile",
                required: true,
                doc: "2D polygon profile",
            },
            KwargMeta {
                name: ":angle",
                required: false,
                doc: "Revolution angle (default 2*pi)",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Number of segments",
            },
        ],
    );
    m.insert(
        "helix",
        vec![
            KwargMeta {
                name: ":radius",
                required: true,
                doc: "Helix radius",
            },
            KwargMeta {
                name: ":pitch",
                required: true,
                doc: "Vertical distance per turn",
            },
            KwargMeta {
                name: ":turns",
                required: true,
                doc: "Number of turns",
            },
        ],
    );
    m.insert(
        "arc",
        vec![
            KwargMeta {
                name: ":radius",
                required: true,
                doc: "Arc radius",
            },
            KwargMeta {
                name: ":angle",
                required: false,
                doc: "Arc angle (default 2*pi)",
            },
        ],
    );
    m.insert(
        "bezier",
        vec![KwargMeta {
            name: ":points",
            required: true,
            doc: "List of 3D control points",
        }],
    );
    m.insert(
        "sweep",
        vec![
            KwargMeta {
                name: ":profile",
                required: true,
                doc: "2D polygon profile",
            },
            KwargMeta {
                name: ":path",
                required: true,
                doc: "Path curve",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Number of segments",
            },
        ],
    );
    m.insert(
        "loft",
        vec![
            KwargMeta {
                name: ":profiles",
                required: true,
                doc: "List of 2D profiles",
            },
            KwargMeta {
                name: ":at",
                required: true,
                doc: "List of Z positions",
            },
            KwargMeta {
                name: ":segments",
                required: false,
                doc: "Number of segments",
            },
        ],
    );
    m.insert(
        "chamfer",
        vec![
            KwargMeta {
                name: ":shape",
                required: true,
                doc: "Shape to chamfer",
            },
            KwargMeta {
                name: ":distance",
                required: true,
                doc: "Chamfer distance",
            },
        ],
    );
    m.insert(
        "place",
        vec![
            KwargMeta {
                name: ":part",
                required: true,
                doc: "Part definition to place",
            },
            KwargMeta {
                name: ":as",
                required: true,
                doc: "Instance name (keyword)",
            },
            KwargMeta {
                name: ":at",
                required: false,
                doc: "Position #p(x y z)",
            },
        ],
    );
    m.insert(
        "fit",
        vec![
            KwargMeta {
                name: ":shaft",
                required: true,
                doc: "Shaft axis reference",
            },
            KwargMeta {
                name: ":hole",
                required: true,
                doc: "Hole axis reference",
            },
        ],
    );
    m.insert(
        "joint",
        vec![
            KwargMeta {
                name: ":type",
                required: false,
                doc: "Joint type (default :fixed)",
            },
            KwargMeta {
                name: ":parts",
                required: false,
                doc: "List of parts",
            },
        ],
    );

    m
});

/// Set of all builtin names (for filtering user-defined symbols)
static BUILTIN_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    BUILTIN_COMPLETIONS
        .iter()
        .map(|c| c.label.as_str())
        .collect()
});

/// Result of scanning for the enclosing function at cursor position
struct EnclosingContext {
    /// Name of the enclosing function (if any)
    function_name: Option<String>,
    /// Set of keyword arguments already used in the current call
    used_keywords: HashSet<String>,
}

/// Find the enclosing function call at the given cursor position.
///
/// Scans backward from cursor, tracking parenthesis depth to find the
/// nearest unmatched `(` and the symbol immediately after it.
/// Also collects keywords already used in that call.
fn find_enclosing_context(source: &str, cursor_pos: usize) -> EnclosingContext {
    let bytes = source.as_bytes();
    let end = cursor_pos.min(bytes.len());

    let mut depth: i32 = 0;
    let mut i = end;
    let mut in_string = false;
    let mut used_keywords = HashSet::new();

    // First pass: collect keywords at current depth (before adjusting for enclosing paren)
    // We'll redo this after finding the enclosing paren

    // Scan backward to find the unmatched open paren
    while i > 0 {
        i -= 1;
        let ch = bytes[i];

        // Handle string literals (simplified: scan backward for unescaped quote)
        if ch == b'"' {
            // Check if this quote is escaped
            let mut backslashes = 0;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                in_string = !in_string;
            }
            continue;
        }

        if in_string {
            continue;
        }

        // Skip comments (scan backward to check if we're in a comment)
        // Simple heuristic: if there's a `;` on the same line before us, skip
        // (This is imperfect but sufficient for completion purposes)

        if ch == b')' {
            depth += 1;
        } else if ch == b'(' {
            if depth == 0 {
                // Found the unmatched open paren — read the function name after it
                let after = &source[i + 1..end];
                let fn_name = after
                    .trim_start()
                    .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
                    .next();

                if let Some(name) = fn_name {
                    if !name.is_empty() {
                        // Collect used keywords between this paren and cursor
                        collect_keywords(&source[i + 1..end], &mut used_keywords);

                        return EnclosingContext {
                            function_name: Some(name.to_string()),
                            used_keywords,
                        };
                    }
                }

                return EnclosingContext {
                    function_name: None,
                    used_keywords: HashSet::new(),
                };
            }
            depth -= 1;
        }
    }

    EnclosingContext {
        function_name: None,
        used_keywords: HashSet::new(),
    }
}

/// Collect keyword arguments (`:name`) used at depth 0 in the given source fragment
fn collect_keywords(fragment: &str, keywords: &mut HashSet<String>) {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let bytes = fragment.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i];

        if ch == b'"' {
            let mut backslashes = 0;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                in_string = !in_string;
            }
            i += 1;
            continue;
        }

        if in_string {
            i += 1;
            continue;
        }

        if ch == b'(' {
            depth += 1;
        } else if ch == b')' {
            depth -= 1;
        } else if ch == b':' && depth == 0 {
            // Read the keyword name
            let start = i;
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_')
            {
                i += 1;
            }
            if i > start + 1 {
                let kw = &fragment[start..i];
                keywords.insert(kw.to_string());
            }
            continue;
        }

        i += 1;
    }
}

/// Collect user-defined symbols from the evaluator environment
fn collect_user_symbols(env: &crate::env::Env) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    for (name, value) in env.bindings() {
        // Skip builtins
        if BUILTIN_NAMES.contains(name.as_str()) {
            continue;
        }

        let (kind, detail, doc) = match value {
            Value::Lambda(lambda) => {
                let params = lambda.params.join(" ");
                (
                    CompletionKind::UserFunction,
                    format!("({name} {params})"),
                    "User-defined function".to_string(),
                )
            }
            Value::PartDef(part) => {
                let params: Vec<String> = part
                    .params
                    .iter()
                    .map(|p| {
                        let mut s = p.name.clone();
                        if let Some(ref t) = p.type_annotation {
                            s.push_str(" :: ");
                            s.push_str(t);
                        }
                        s
                    })
                    .collect();
                let detail = if params.is_empty() {
                    format!("(defpart {name})")
                } else {
                    format!("(defpart {name} ({}))", params.join(", "))
                };
                let doc = if part.docstring.is_empty() {
                    "User-defined part".to_string()
                } else {
                    part.docstring.clone()
                };
                (CompletionKind::Part, detail, doc)
            }
            Value::Macro(mac) => {
                let params = mac.params.join(" ");
                (
                    CompletionKind::Macro,
                    format!("({name} {params})"),
                    "User-defined macro".to_string(),
                )
            }
            Value::BuiltinFn(_) => continue,
            _ => {
                let type_name = value.type_name();
                let detail = format!("{type_name}: {value}");
                (
                    CompletionKind::Variable,
                    detail,
                    "User-defined variable".to_string(),
                )
            }
        };

        items.push(CompletionItem {
            label: name.clone(),
            kind,
            detail,
            doc,
            example: String::new(),
            boost: -1, // Lower priority than builtins
        });
    }

    items
}

/// Get all completion items for the given source at the cursor position.
///
/// Combines static builtin completions, context-dependent keyword
/// completions, and user-defined symbols from partial evaluation.
pub fn get_completions(source: &str, cursor_pos: usize) -> Vec<CompletionItem> {
    get_completions_with_files(source, cursor_pos, &[])
}

/// Like `get_completions`, but with project file context for module resolution.
///
/// `files` is a list of `(filename, source)` pairs representing the project files.
/// These are registered as user modules so that `require` can resolve them.
pub fn get_completions_with_files(
    source: &str,
    cursor_pos: usize,
    files: &[(String, String)],
) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = BUILTIN_COMPLETIONS.clone();

    // Phase 2: Context-dependent keyword argument completions
    let ctx = find_enclosing_context(source, cursor_pos);
    if let Some(ref fn_name) = ctx.function_name {
        if let Some(kwargs) = KWARG_TABLE.get(fn_name.as_str()) {
            for kw in kwargs {
                // Skip already-used keywords
                if ctx.used_keywords.contains(kw.name) {
                    continue;
                }
                let req_marker = if kw.required { " (required)" } else { "" };
                items.push(CompletionItem {
                    label: kw.name.to_string(),
                    kind: CompletionKind::Keyword,
                    detail: format!("{}{req_marker}", kw.name),
                    doc: kw.doc.to_string(),
                    example: String::new(),
                    boost: 10, // High priority for keyword args in context
                });
            }
        }
    }

    // Phase 3: User-defined symbols from partial evaluation
    let truncated = find_complete_prefix(source, cursor_pos);
    if !truncated.is_empty() {
        let mut evaluator = crate::evaluator::Evaluator::new();
        // Register project files as user modules
        for (name, src) in files {
            evaluator
                .module_loader_mut()
                .add_user_module(name.clone(), src.clone());
        }
        // Evaluate each top-level form individually, ignoring errors
        if let Ok(exprs) = crate::parser::parse_with_spans(&truncated) {
            for (expr, span) in &exprs {
                evaluator.set_call_span(span.clone());
                let _ = evaluator.eval(expr, &std::rc::Rc::clone(evaluator.global_env()));
            }
        }
        let user_items = collect_user_symbols(&evaluator.global_env().borrow());
        items.extend(user_items);
    }

    // Add user file names as module completion items
    for (name, _) in files {
        items.push(CompletionItem {
            label: name.clone(),
            kind: CompletionKind::Variable,
            detail: format!("(require \"{name}\")"),
            doc: "Project file".to_string(),
            example: format!("(require \"{name}\")"),
            boost: 5,
        });
    }

    items
}

/// Find the largest prefix of source (up to cursor_pos) that consists of
/// complete top-level forms (balanced parentheses).
fn find_complete_prefix(source: &str, cursor_pos: usize) -> String {
    let end = cursor_pos.min(source.len());
    let bytes = &source.as_bytes()[..end];

    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut last_complete_end = 0;
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i];

        if ch == b'"' {
            let mut backslashes = 0;
            let mut j = i;
            while j > 0 && bytes[j - 1] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                in_string = !in_string;
            }
            i += 1;
            continue;
        }

        if in_string {
            i += 1;
            continue;
        }

        // Skip line comments
        if ch == b';' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if ch == b'(' {
            depth += 1;
        } else if ch == b')' {
            depth -= 1;
            if depth == 0 {
                last_complete_end = i + 1;
            }
        }

        i += 1;
    }

    source[..last_complete_end].to_string()
}

/// Serialize completion items to JSON string
pub fn completions_to_json(items: &[CompletionItem]) -> String {
    let mut json = String::from("[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str(&format!(
            "\"label\":{},\"kind\":{},\"detail\":{},\"doc\":{},\"example\":{},\"boost\":{}",
            json_escape(&item.label),
            json_escape(item.kind.as_str()),
            json_escape(&item.detail),
            json_escape(&item.doc),
            json_escape(&item.example),
            item.boost,
        ));
        json.push('}');
    }
    json.push(']');
    json
}

/// Escape a string for JSON
fn json_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 2);
    result.push('"');
    for ch in s.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_completions_not_empty() {
        let items = &*BUILTIN_COMPLETIONS;
        assert!(items.len() > 50);
    }

    #[test]
    fn test_find_enclosing_function_simple() {
        let source = "(cylinder :radius ";
        let ctx = find_enclosing_context(source, source.len());
        assert_eq!(ctx.function_name, Some("cylinder".to_string()));
    }

    #[test]
    fn test_find_enclosing_function_nested() {
        let source = "(translate :shape (cylinder :radius ";
        let ctx = find_enclosing_context(source, source.len());
        assert_eq!(ctx.function_name, Some("cylinder".to_string()));
    }

    #[test]
    fn test_used_keywords_collected() {
        let source = "(cylinder :radius 10 :height 20 :";
        let ctx = find_enclosing_context(source, source.len());
        assert_eq!(ctx.function_name, Some("cylinder".to_string()));
        assert!(ctx.used_keywords.contains(":radius"));
        assert!(ctx.used_keywords.contains(":height"));
    }

    #[test]
    fn test_find_complete_prefix() {
        let source = "(defvar x 10)\n(cylinder :radius ";
        let prefix = find_complete_prefix(source, source.len());
        assert_eq!(prefix, "(defvar x 10)");
    }

    #[test]
    fn test_get_completions_with_user_symbols() {
        let source = "(defvar my-radius 10)\n(cylinder :radius ";
        let items = get_completions(source, source.len());
        assert!(items.iter().any(|i| i.label == "my-radius"));
        assert!(items.iter().any(|i| i.label == "cylinder"));
        // :radius should be excluded (already used), :height and :segments offered
        assert!(items.iter().any(|i| i.label == ":height"));
        assert!(!items.iter().any(|i| i.label == ":radius"));
    }

    #[test]
    fn test_json_escape() {
        assert_eq!(json_escape("hello"), "\"hello\"");
        assert_eq!(json_escape("he\"llo"), "\"he\\\"llo\"");
        assert_eq!(json_escape("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn test_completions_to_json_format() {
        let items = vec![CompletionItem {
            label: "test".to_string(),
            kind: CompletionKind::Function,
            detail: "(test)".to_string(),
            doc: "A test".to_string(),
            example: "(test 1 2)".to_string(),
            boost: 0,
        }];
        let json = completions_to_json(&items);
        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains("\"label\":\"test\""));
    }
}
