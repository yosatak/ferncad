//! ferncad CAD kernel
//!
//! Provides BREP geometry (via truck), primitive generation, CSG operations,
//! transforms, and STL/STEP export.

pub mod assembly_realize;
pub mod brep;
pub mod bsp;
pub mod export;
pub mod mesh;
pub mod primitives;
pub mod realize;
pub mod step;
pub mod transform;
