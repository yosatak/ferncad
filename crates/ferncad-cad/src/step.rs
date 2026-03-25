//! STEP file export via truck-stepio
//!
//! Converts `ShapeNode` to BREP solid and serializes to STEP format.

use truck_stepio::out::{CompleteStepDisplay, StepHeaderDescriptor, StepModel};

use ferncad_core::error::FernResult;
use ferncad_core::types::ShapeNode;

use crate::brep;

/// Export a `ShapeNode` to STEP format bytes
///
/// # Errors
///
/// Returns an error if BREP conversion or STEP serialization fails.
pub fn export_step_bytes(node: &ShapeNode) -> FernResult<Vec<u8>> {
    let solid = brep::shape_to_solid_for_export(node)?;
    let compressed = solid.compress();
    let step_model = StepModel::from(&compressed);
    let header = StepHeaderDescriptor {
        organization_system: "ferncad".to_string(),
        ..Default::default()
    };
    let complete = CompleteStepDisplay::new(step_model, header);
    let step_str = format!("{complete}");
    Ok(step_str.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_step_box() {
        let node = ShapeNode::Box {
            width: 10.0,
            depth: 10.0,
            height: 10.0,
        };
        let bytes = export_step_bytes(&node).unwrap();
        let content = String::from_utf8(bytes).unwrap();
        assert!(content.contains("ISO-10303-21"));
    }
}
