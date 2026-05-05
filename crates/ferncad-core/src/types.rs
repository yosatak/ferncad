//! ferncad value types and shape node definitions
//!
//! Defines all values handled by the evaluator and CSG tree node types.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::error::{SourceLocation, SourceSpan};

/// Value type returned by the evaluator
#[derive(Debug, Clone)]
pub enum Value {
    /// Integer
    Int(i64),
    /// Floating-point number
    Float(f64),
    /// Length (normalized to mm)
    Length(f64),
    /// Angle (normalized to rad)
    Angle(f64),
    /// 3D vector
    Vec3([f64; 3]),
    /// 3D point
    Point3([f64; 3]),
    /// String
    Str(String),
    /// Symbol (exists as a value only when quoted)
    Symbol(String),
    /// Keyword
    Keyword(String),
    /// Boolean (`t` = true, `nil` = false)
    Bool(bool),
    /// Nil value
    Nil,
    /// List
    List(Vec<Value>),
    /// CSG shape node (with source location tracking)
    Shape(Arc<TrackedShape>),
    /// Built-in function
    BuiltinFn(BuiltinFnDef),
    /// User-defined function (closure)
    Lambda(Arc<LambdaDef>),
    /// Part definition
    PartDef(Arc<PartDef>),
    /// Face reference
    FaceRef(crate::face::FaceRef),
    /// Axis reference
    AxisRef(crate::face::AxisRef),
    /// Assembly definition
    Assembly(Arc<crate::assembly::AssemblyDef>),
    /// Macro definition
    Macro(Arc<MacroDef>),
    /// Path curve
    Path(Arc<PathNode>),
    /// Memoized callable: same arguments return the same Arc, cooperating with
    /// the realize-stage pointer-identity cache so recursive shape builders
    /// share their tessellated output.
    Memoized(Arc<MemoizedFn>),
}

impl Value {
    /// Get as a number (includes Int to f64 conversion)
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::Float(f) => Some(*f),
            Value::Length(f) => Some(*f),
            Value::Angle(f) => Some(*f),
            _ => None,
        }
    }

    /// Evaluate as a boolean (CL-compliant: only nil and Bool(false) are falsy)
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    /// Check whether a value conforms to a type annotation name
    ///
    /// Numeric types (int, float) can be implicitly converted to length or angle.
    pub fn matches_type(&self, type_name: &str) -> bool {
        match type_name {
            "int" => matches!(self, Value::Int(_)),
            "float" => matches!(self, Value::Float(_) | Value::Int(_)),
            "number" => self.as_number().is_some(),
            "length" => matches!(self, Value::Length(_) | Value::Float(_) | Value::Int(_)),
            "angle" => matches!(self, Value::Angle(_) | Value::Float(_) | Value::Int(_)),
            "vec3" => matches!(self, Value::Vec3(_)),
            "point3" => matches!(self, Value::Point3(_)),
            "string" => matches!(self, Value::Str(_)),
            "keyword" => matches!(self, Value::Keyword(_)),
            "bool" => matches!(self, Value::Bool(_)),
            "shape" => matches!(self, Value::Shape(_)),
            "path" => matches!(self, Value::Path(_)),
            "list" => matches!(self, Value::List(_)),
            _ => true, // Unknown type names always match (Phase 2 safety measure)
        }
    }

    /// Return the type name
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "integer",
            Value::Float(_) => "float",
            Value::Length(_) => "length",
            Value::Angle(_) => "angle",
            Value::Vec3(_) => "vector",
            Value::Point3(_) => "point",
            Value::Str(_) => "string",
            Value::Symbol(_) => "symbol",
            Value::Keyword(_) => "keyword",
            Value::Bool(_) => "boolean",
            Value::Nil => "nil",
            Value::List(_) => "list",
            Value::Shape(_) => "shape",
            Value::BuiltinFn(_) => "builtin",
            Value::Lambda(_) => "function",
            Value::PartDef(_) => "part",
            Value::FaceRef(_) => "face-ref",
            Value::AxisRef(_) => "axis-ref",
            Value::Assembly(_) => "assembly",
            Value::Macro(_) => "macro",
            Value::Path(_) => "path",
            Value::Memoized(_) => "memoized",
        }
    }

    /// Project this value into a hashable key for `memoize` caches.
    ///
    /// Floats are keyed by bit pattern; reference-counted values (Shape, Path,
    /// Lambda, Macro, PartDef, Assembly, Memoized) are keyed by `Arc::as_ptr`.
    /// Built-in functions key by name.
    pub fn to_memo_key(&self) -> MemoKey {
        match self {
            Value::Int(n) => MemoKey::Int(*n),
            Value::Float(v) => MemoKey::FloatBits(v.to_bits()),
            Value::Length(v) => MemoKey::FloatBits(v.to_bits()),
            Value::Angle(v) => MemoKey::FloatBits(v.to_bits()),
            Value::Bool(b) => MemoKey::Bool(*b),
            Value::Nil => MemoKey::Nil,
            Value::Str(s) => MemoKey::Str(s.clone()),
            Value::Symbol(s) => MemoKey::Sym(s.clone()),
            Value::Keyword(k) => MemoKey::Kw(k.clone()),
            Value::Vec3(v) => MemoKey::Vec3([v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]),
            Value::Point3(v) => MemoKey::Point3([v[0].to_bits(), v[1].to_bits(), v[2].to_bits()]),
            Value::List(items) => MemoKey::List(items.iter().map(Value::to_memo_key).collect()),
            Value::Shape(s) => MemoKey::Ptr(Arc::as_ptr(s) as usize),
            Value::Path(p) => MemoKey::Ptr(Arc::as_ptr(p) as usize),
            Value::Lambda(l) => MemoKey::Ptr(Arc::as_ptr(l) as usize),
            Value::Macro(m) => MemoKey::Ptr(Arc::as_ptr(m) as usize),
            Value::PartDef(p) => MemoKey::Ptr(Arc::as_ptr(p) as usize),
            Value::Assembly(a) => MemoKey::Ptr(Arc::as_ptr(a) as usize),
            Value::Memoized(m) => MemoKey::Ptr(Arc::as_ptr(m) as usize),
            Value::BuiltinFn(def) => MemoKey::Builtin(def.name.clone()),
            Value::FaceRef(r) => MemoKey::List(vec![
                MemoKey::Sym(r.instance_name.clone()),
                MemoKey::Sym(r.face_name.clone()),
            ]),
            Value::AxisRef(r) => MemoKey::List(vec![
                MemoKey::Sym(r.instance_name.clone()),
                MemoKey::Sym(r.axis_name.clone()),
            ]),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Length(v) => write!(f, "{v}mm"),
            Value::Angle(v) => write!(f, "{v}rad"),
            Value::Vec3(v) => write!(f, "#v({} {} {})", v[0], v[1], v[2]),
            Value::Point3(v) => write!(f, "#p({} {} {})", v[0], v[1], v[2]),
            Value::Str(s) => write!(f, "\"{s}\""),
            Value::Symbol(s) => write!(f, "{s}"),
            Value::Keyword(k) => write!(f, ":{k}"),
            Value::Bool(true) => write!(f, "t"),
            Value::Bool(false) => write!(f, "nil"),
            Value::Nil => write!(f, "nil"),
            Value::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Value::Shape(_) => write!(f, "<shape>"),
            Value::BuiltinFn(def) => write!(f, "<builtin:{}>", def.name),
            Value::Lambda(_) => write!(f, "<lambda>"),
            Value::PartDef(def) => write!(f, "<part:{}>", def.name),
            Value::FaceRef(r) => write!(f, "<face:{}:{}>", r.instance_name, r.face_name),
            Value::AxisRef(r) => write!(f, "<axis:{}:{}>", r.instance_name, r.axis_name),
            Value::Assembly(a) => write!(f, "<assembly:{}>", a.name),
            Value::Macro(m) => write!(f, "<macro:{}>", m.name),
            Value::Path(_) => write!(f, "<path>"),
            Value::Memoized(_) => write!(f, "<memoized>"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => {
                (*a as f64) == *b
            }
            (Value::Length(a), Value::Length(b)) => a == b,
            (Value::Angle(a), Value::Angle(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            (Value::Keyword(a), Value::Keyword(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Vec3(a), Value::Vec3(b)) => a == b,
            (Value::Point3(a), Value::Point3(b)) => a == b,
            (Value::FaceRef(a), Value::FaceRef(b)) => a == b,
            (Value::AxisRef(a), Value::AxisRef(b)) => a == b,
            (Value::Path(a), Value::Path(b)) => a == b,
            _ => false,
        }
    }
}

/// Built-in function definition
#[derive(Clone)]
pub struct BuiltinFnDef {
    /// Function name
    pub name: String,
    /// Implementation
    pub func: fn(&[Value], &SourceLocation) -> Result<Value, crate::error::FernError>,
}

impl fmt::Debug for BuiltinFnDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BuiltinFn({})", self.name)
    }
}

/// User-defined function
#[derive(Debug, Clone)]
pub struct LambdaDef {
    /// Parameter name list
    pub params: Vec<String>,
    /// Function body (list of S-expressions)
    pub body: Vec<Value>,
    /// Environment ID at definition time (for closures)
    pub env_id: usize,
}

/// Memoized callable: wraps another `Value` (typically a `Lambda` or
/// `BuiltinFn`) and caches results keyed by argument values. Designed so the
/// underlying callable's pure shape builders return the same `Arc<ShapeNode>`
/// for identical inputs, letting the realize-stage pointer cache reuse work.
#[derive(Debug)]
pub struct MemoizedFn {
    /// The wrapped callable
    pub inner: Value,
    /// Cache from argument-vector keys to results
    pub cache: Mutex<HashMap<Vec<MemoKey>, Value>>,
}

/// Hashable projection of a `Value` for memoize cache keys.
///
/// Floats are keyed by bit pattern (so `1.0` and `1.0` collide but NaN never
/// does). Reference-counted variants (Shape, Path, Lambda) are keyed by
/// `Arc::as_ptr` — same Arc → same key, distinct Arcs → distinct keys, even
/// if they were structurally equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MemoKey {
    /// Integer literal
    Int(i64),
    /// Float / Length / Angle keyed by bit pattern
    FloatBits(u64),
    /// Boolean
    Bool(bool),
    /// Nil
    Nil,
    /// String
    Str(String),
    /// Symbol name
    Sym(String),
    /// Keyword name
    Kw(String),
    /// Vec3 keyed by bit patterns
    Vec3([u64; 3]),
    /// Point3 keyed by bit patterns
    Point3([u64; 3]),
    /// Reference-counted Arc address (for Shape, Path, Lambda, etc.)
    Ptr(usize),
    /// Built-in function name
    Builtin(String),
    /// Recursive list
    List(Vec<MemoKey>),
}

/// Macro definition
#[derive(Debug, Clone)]
pub struct MacroDef {
    /// Macro name
    pub name: String,
    /// Parameter name list
    pub params: Vec<String>,
    /// Macro body (quasiquote template or expressions)
    pub body: Vec<Value>,
    /// Environment ID at definition time
    pub env_id: usize,
}

/// Part definition
#[derive(Debug, Clone)]
pub struct PartDef {
    /// Part name
    pub name: String,
    /// Documentation string
    pub docstring: String,
    /// Metadata
    pub meta: HashMap<String, Value>,
    /// Parameter specifications
    pub params: Vec<ParamSpec>,
    /// Named face declarations
    pub faces: Vec<crate::face::FaceSpec>,
    /// Named axis declarations
    pub axes: Vec<crate::face::AxisSpec>,
    /// Body (S-expressions for lazy evaluation)
    pub body: Vec<Value>,
    /// Environment ID at definition time
    pub env_id: usize,
}

/// Part parameter specification
#[derive(Debug, Clone)]
pub struct ParamSpec {
    /// Parameter name
    pub name: String,
    /// Type annotation (optional)
    pub type_annotation: Option<String>,
    /// Default value (optional)
    pub default: Option<Value>,
    /// Documentation string (optional)
    pub doc: Option<String>,
}

/// Default segment count (balanced with BSP CSG performance)
pub const DEFAULT_SEGMENTS: u32 = 16;

/// Path node — a parametric curve in 3D space
#[derive(Debug, Clone, PartialEq)]
pub enum PathNode {
    /// Helix: constant radius, constant pitch
    Helix { radius: f64, pitch: f64, turns: f64 },
    /// Circular arc in XY plane
    Arc { radius: f64, angle_rad: f64 },
    /// Bezier curve through control points
    Bezier { points: Vec<[f64; 3]> },
}

impl PathNode {
    /// Evaluate position at parameter t in [0, 1]
    pub fn position(&self, t: f64) -> [f64; 3] {
        match self {
            PathNode::Helix {
                radius,
                pitch,
                turns,
            } => {
                let theta = 2.0 * std::f64::consts::PI * turns * t;
                [
                    radius * theta.cos(),
                    radius * theta.sin(),
                    pitch * turns * t,
                ]
            }
            PathNode::Arc { radius, angle_rad } => {
                let theta = angle_rad * t;
                [radius * theta.cos(), radius * theta.sin(), 0.0]
            }
            PathNode::Bezier { points } => de_casteljau(points, t),
        }
    }

    /// Evaluate tangent vector (unnormalized) at parameter t in [0, 1]
    pub fn tangent(&self, t: f64) -> [f64; 3] {
        match self {
            PathNode::Helix {
                radius,
                pitch,
                turns,
            } => {
                let omega = 2.0 * std::f64::consts::PI * turns;
                let theta = omega * t;
                [
                    -radius * omega * theta.sin(),
                    radius * omega * theta.cos(),
                    pitch * turns,
                ]
            }
            PathNode::Arc { radius, angle_rad } => {
                let theta = angle_rad * t;
                [
                    -radius * angle_rad * theta.sin(),
                    radius * angle_rad * theta.cos(),
                    0.0,
                ]
            }
            PathNode::Bezier { points } => bezier_tangent(points, t),
        }
    }
}

/// De Casteljau algorithm for evaluating a Bezier curve at parameter t
fn de_casteljau(points: &[[f64; 3]], t: f64) -> [f64; 3] {
    let mut work: Vec<[f64; 3]> = points.to_vec();
    let n = work.len();
    for level in 1..n {
        for i in 0..n - level {
            work[i] = [
                work[i][0] * (1.0 - t) + work[i + 1][0] * t,
                work[i][1] * (1.0 - t) + work[i + 1][1] * t,
                work[i][2] * (1.0 - t) + work[i + 1][2] * t,
            ];
        }
    }
    work[0]
}

/// Bezier hodograph (derivative) at parameter t
fn bezier_tangent(points: &[[f64; 3]], t: f64) -> [f64; 3] {
    let n = points.len();
    if n < 2 {
        return [0.0, 0.0, 1.0];
    }
    // Hodograph control points: n-1 points, each = degree * (P[i+1] - P[i])
    let degree = (n - 1) as f64;
    let hodograph: Vec<[f64; 3]> = (0..n - 1)
        .map(|i| {
            [
                degree * (points[i + 1][0] - points[i][0]),
                degree * (points[i + 1][1] - points[i][1]),
                degree * (points[i + 1][2] - points[i][2]),
            ]
        })
        .collect();
    de_casteljau(&hodograph, t)
}

/// Shape node with source location tracking for editor↔viewer highlighting
#[derive(Debug, Clone)]
pub struct TrackedShape {
    /// The underlying shape node (reference-counted for cheap cloning)
    pub node: Arc<ShapeNode>,
    /// Source span (byte range) of the expression that created this shape
    pub span: SourceSpan,
}

impl TrackedShape {
    /// Create a TrackedShape with a dummy span
    pub fn untracked(node: ShapeNode) -> Self {
        Self {
            node: Arc::new(node),
            span: SourceSpan::dummy(),
        }
    }
}

impl PartialEq for TrackedShape {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node // Span intentionally excluded from equality
    }
}

/// CSG tree node (immutable, reference-counted)
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeNode {
    // === Primitives ===
    /// Box
    Box { width: f64, depth: f64, height: f64 },
    /// Sphere
    Sphere { radius: f64, segments: u32 },
    /// Cylinder
    Cylinder {
        radius: f64,
        height: f64,
        segments: u32,
    },
    /// Cone
    Cone {
        radius_bottom: f64,
        radius_top: f64,
        height: f64,
        segments: u32,
    },
    /// Torus
    Torus {
        radius_major: f64,
        radius_minor: f64,
        segments: u32,
    },
    /// Prism
    Prism {
        sides: u32,
        radius: f64,
        height: f64,
    },

    // === CSG Operations ===
    /// Union
    Union { children: Vec<Arc<ShapeNode>> },
    /// Difference (subtract others from the first element)
    Difference {
        base: Arc<ShapeNode>,
        cutters: Vec<Arc<ShapeNode>>,
    },
    /// Intersection
    Intersection { children: Vec<Arc<ShapeNode>> },

    // === Transforms ===
    /// Translation
    Translate {
        shape: Arc<ShapeNode>,
        offset: [f64; 3],
    },
    /// Rotation (axis + angle in rad)
    Rotate {
        shape: Arc<ShapeNode>,
        axis: [f64; 3],
        angle_rad: f64,
    },
    /// Scale
    Scale {
        shape: Arc<ShapeNode>,
        factors: [f64; 3],
    },

    // === Profile operations ===
    /// Extrude a 2D profile along Z axis
    Extrude { profile: Vec<[f64; 2]>, height: f64 },
    /// Revolve a 2D profile (in XZ plane) around Z axis
    Revolve {
        profile: Vec<[f64; 2]>,
        angle_rad: f64,
        segments: u32,
    },

    // === Sweep/Loft operations ===
    /// Sweep a 2D profile along a 3D path
    Sweep {
        profile: Vec<[f64; 2]>,
        path: Arc<PathNode>,
        segments: u32,
    },
    /// Loft between multiple 2D profiles at specified Z positions
    Loft {
        profiles: Vec<Vec<[f64; 2]>>,
        positions: Vec<f64>,
        segments: u32,
    },

    // === Edge operations ===
    /// Chamfer all edges of a shape
    Chamfer {
        shape: Arc<ShapeNode>,
        distance: f64,
    },
}
