//! Face and axis reference type definitions
//!
//! Defines `:faces` / `:axes` declarations for defpart and reference types used in assembly constraints.

/// Named face specification for a part
#[derive(Debug, Clone, PartialEq)]
pub struct FaceSpec {
    /// Face name
    pub name: String,
    /// Documentation string
    pub doc: Option<String>,
    /// Normal vector hint (when explicitly specified)
    pub normal: Option<[f64; 3]>,
}

/// Named axis specification for a part
#[derive(Debug, Clone, PartialEq)]
pub struct AxisSpec {
    /// Axis name
    pub name: String,
    /// Documentation string
    pub doc: Option<String>,
    /// Direction vector hint
    pub direction: Option<[f64; 3]>,
}

/// Face reference (used in assembly constraints)
#[derive(Debug, Clone, PartialEq)]
pub struct FaceRef {
    /// Part instance name
    pub instance_name: String,
    /// Face name
    pub face_name: String,
}

/// Axis reference (used in assembly constraints)
#[derive(Debug, Clone, PartialEq)]
pub struct AxisRef {
    /// Part instance name
    pub instance_name: String,
    /// Axis name
    pub axis_name: String,
}
