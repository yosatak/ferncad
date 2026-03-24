//! ferncad WASM bindings
//!
//! Thin wrapper exposing the ferncad engine to the browser via `wasm_bindgen`.

use wasm_bindgen::prelude::*;

use ferncad_cad::export;
use ferncad_cad::realize;

/// Evaluate ferncad source code and return mesh data as a Float32Array.
///
/// The returned array contains positions followed by normals (each 3 floats per vertex).
/// `result.length / 2` gives the positions length; `positions.length / 9` gives the triangle count.
#[wasm_bindgen]
pub fn evaluate(source: &str) -> Result<js_sys::Float32Array, JsValue> {
    let mesh = realize::eval_and_realize(source).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let (positions, normals) = mesh.to_flat_arrays();

    let mut combined = Vec::with_capacity(positions.len() + normals.len());
    combined.extend_from_slice(&positions);
    combined.extend_from_slice(&normals);

    Ok(js_sys::Float32Array::from(&combined[..]))
}

/// Evaluate ferncad source code and return binary STL data.
#[wasm_bindgen]
pub fn export_stl(source: &str) -> Result<Vec<u8>, JsValue> {
    let mesh = realize::eval_and_realize(source).map_err(|e| JsValue::from_str(&e.to_string()))?;

    export::export_stl_bytes(&mesh)
        .map_err(|e| JsValue::from_str(&format!("STL export error: {e}")))
}

/// Evaluate ferncad source and return per-part mesh data as JSON.
///
/// For assemblies, returns separate meshes with colors for each part.
/// For single shapes, returns one part named "shape".
///
/// JSON format: `{ "parts": [{ "name": "...", "positions": [...], "normals": [...], "color": [r,g,b] }] }`
#[wasm_bindgen]
pub fn evaluate_parts(source: &str) -> Result<String, JsValue> {
    let parts = ferncad_cad::assembly_realize::eval_and_realize_parts(source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Manual JSON construction (lightweight, no serde dependency)
    let mut json = String::from("{\"parts\":[");
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let (positions, normals) = part.mesh.to_flat_arrays();
        json.push_str(&format!(
            "{{\"name\":\"{}\",\"color\":[{},{},{}],\"posLen\":{},\"norLen\":{}",
            part.name,
            part.color[0],
            part.color[1],
            part.color[2],
            positions.len(),
            normals.len()
        ));

        json.push_str(",\"positions\":[");
        for (j, v) in positions.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            json.push_str(&format!("{v}"));
        }
        json.push_str("],\"normals\":[");
        for (j, v) in normals.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            json.push_str(&format!("{v}"));
        }
        json.push_str("]}");
    }
    json.push_str("]}");
    Ok(json)
}

/// Check syntax only. Returns an error message string, or empty string on success.
#[wasm_bindgen]
pub fn check_syntax(source: &str) -> String {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    match evaluator.eval_source(source) {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

/// Evaluate ferncad source and return the string representation of the result.
#[wasm_bindgen]
pub fn eval_to_string(source: &str) -> Result<String, JsValue> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator
        .eval_source(source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(format!("{result}"))
}
