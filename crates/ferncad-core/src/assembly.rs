//! Assembly and constraint type definitions
//!
//! Data structures for placing multiple parts and positioning them via face/axis constraints.

use std::collections::HashMap;
use std::sync::Arc;

use crate::face::{AxisRef, FaceRef};
use crate::types::{PartDef, TrackedShape, Value};

/// Assembly definition
#[derive(Debug, Clone)]
pub struct AssemblyDef {
    /// Assembly name
    pub name: String,
    /// Documentation string
    pub docstring: String,
    /// Placed parts
    pub parts: Vec<PartInstance>,
    /// Constraint list
    pub constraints: Vec<Constraint>,
}

/// Part instance (a placed part within an assembly)
#[derive(Debug, Clone)]
pub struct PartInstance {
    /// Name within the assembly
    pub name: String,
    /// Reference to part definition
    pub part_def: Arc<PartDef>,
    /// Parameter values
    pub params: HashMap<String, Value>,
    /// Shape node with source span (evaluated)
    pub shape: Option<Arc<TrackedShape>>,
    /// 4x4 transform matrix (column-major)
    pub transform: [f64; 16],
    /// Color (RGB 0-1)
    pub color: [f64; 3],
}

impl PartInstance {
    /// Create an instance initialized with an identity transform
    pub fn new(name: String, part_def: Arc<PartDef>, params: HashMap<String, Value>) -> Self {
        Self {
            name,
            part_def,
            params,
            shape: None,
            transform: identity_matrix(),
            color: [0.7, 0.7, 0.7],
        }
    }

    /// Apply a translation
    pub fn translate(&mut self, offset: [f64; 3]) {
        self.transform[12] += offset[0];
        self.transform[13] += offset[1];
        self.transform[14] += offset[2];
    }
}

/// Constraint types
#[derive(Debug, Clone)]
pub enum Constraint {
    /// Mate faces (flush contact)
    Mate {
        face1: FaceRef,
        face2: FaceRef,
        offset: f64,
    },
    /// Align axes
    AlignAxis { axis1: AxisRef, axis2: AxisRef },
    /// Fit
    Fit {
        shaft: FaceRef,
        hole: FaceRef,
        clearance: f64,
        fit_type: FitType,
    },
    /// Joint
    Joint {
        joint_type: JointType,
        parts: Vec<String>,
    },
}

/// Fit types
#[derive(Debug, Clone, PartialEq)]
pub enum FitType {
    /// Clearance fit
    Clearance,
    /// Interference fit
    Interference,
    /// Transition fit
    Transition,
}

/// Joint types
#[derive(Debug, Clone, PartialEq)]
pub enum JointType {
    /// Fixed
    Fixed,
    /// Revolute joint
    Revolute,
    /// Prismatic joint
    Prismatic,
}

/// Return a 4x4 identity matrix
pub fn identity_matrix() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, // col 0
        0.0, 1.0, 0.0, 0.0, // col 1
        0.0, 0.0, 1.0, 0.0, // col 2
        0.0, 0.0, 0.0, 1.0, // col 3
    ]
}

/// Part color palette (for auto-assignment)
const PART_COLORS: &[[f64; 3]] = &[
    [0.53, 0.53, 0.80], // blue-violet
    [0.80, 0.53, 0.53], // red
    [0.53, 0.80, 0.53], // green
    [0.80, 0.73, 0.53], // yellow
    [0.53, 0.73, 0.80], // cyan
    [0.73, 0.53, 0.80], // purple
    [0.80, 0.60, 0.53], // orange
    [0.53, 0.80, 0.73], // turquoise
];

/// Get a color for a given part index
pub fn part_color(index: usize) -> [f64; 3] {
    PART_COLORS[index % PART_COLORS.len()]
}
