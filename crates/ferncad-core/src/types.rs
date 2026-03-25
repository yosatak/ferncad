//! ferncad value types and shape node definitions
//!
//! Defines all values handled by the evaluator and CSG tree node types.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::error::SourceLocation;

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
    /// CSG shape node
    Shape(Arc<ShapeNode>),
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

/// Default segment count
/// Default segment count (balanced with BSP CSG performance)
pub const DEFAULT_SEGMENTS: u32 = 16;

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

    /// Chamfer all edges of a shape
    Chamfer {
        shape: Arc<ShapeNode>,
        distance: f64,
    },
}
