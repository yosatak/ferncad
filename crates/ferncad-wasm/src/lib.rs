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

/// Check syntax and return diagnostics as JSON.
///
/// Returns `{"diagnostics":[]}` on success, or
/// `{"diagnostics":[{"line":N,"col":N,"message":"...","severity":"error"}]}` on error.
#[wasm_bindgen]
pub fn check_syntax(source: &str) -> String {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    match evaluator.eval_source(source) {
        Ok(_) => r#"{"diagnostics":[]}"#.to_string(),
        Err(e) => {
            let (line, col) = extract_error_location(&e);
            let msg = e.to_string().replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                r#"{{"diagnostics":[{{"line":{line},"col":{col},"message":"{msg}","severity":"error"}}]}}"#
            )
        }
    }
}

/// Extract line/column from a FernError
fn extract_error_location(e: &ferncad_core::error::FernError) -> (usize, usize) {
    use ferncad_core::error::FernError;
    match e {
        FernError::LexError { loc, .. }
        | FernError::UnmatchedOpenParen { loc }
        | FernError::UnmatchedCloseParen { loc }
        | FernError::ParseError { loc, .. }
        | FernError::EvalError { loc, .. }
        | FernError::UndefinedVariable { loc, .. }
        | FernError::TypeError { loc, .. } => (loc.line, loc.col),
        FernError::CadError { .. } => (0, 0),
    }
}

/// Evaluate ferncad source code and return STEP format data.
///
/// Uses the truck BREP kernel for exact geometry representation.
#[wasm_bindgen]
pub fn export_step(source: &str) -> Result<Vec<u8>, JsValue> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator
        .eval_source(source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    match result {
        ferncad_core::types::Value::Shape(node) => ferncad_cad::step::export_step_bytes(&node)
            .map_err(|e| JsValue::from_str(&format!("STEP export error: {e}"))),
        _ => Err(JsValue::from_str("STEP export requires a shape expression")),
    }
}

/// Extract defpart parameter specs from source code as JSON.
///
/// Returns JSON: `{"parts":[{"name":"...","params":[{"name":"...","type":"...","default":N,"doc":"..."}]}]}`
#[wasm_bindgen]
pub fn extract_params(source: &str) -> String {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    if evaluator.eval_source(source).is_err() {
        return r#"{"parts":[]}"#.to_string();
    }

    let env = evaluator.global_env();
    let env_ref = env.borrow();
    let mut json = String::from(r#"{"parts":["#);
    let mut first_part = true;

    for (name, value) in env_ref.bindings() {
        if let ferncad_core::types::Value::PartDef(part_def) = value {
            if !first_part {
                json.push(',');
            }
            first_part = false;
            json.push_str(&format!(r#"{{"name":"{}","params":["#, name));
            let mut first_param = true;
            for param in &part_def.params {
                if !first_param {
                    json.push(',');
                }
                first_param = false;
                let type_str = param.type_annotation.as_deref().unwrap_or("number");
                let default_val = param
                    .default
                    .as_ref()
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0);
                let doc = param.doc.as_deref().unwrap_or("");
                let doc_escaped = doc.replace('\\', "\\\\").replace('"', "\\\"");
                json.push_str(&format!(
                    r#"{{"name":"{}","type":"{type_str}","default":{default_val},"doc":"{doc_escaped}"}}"#,
                    param.name
                ));
            }
            json.push_str("]}");
        }
    }

    json.push_str("]}");
    json
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
