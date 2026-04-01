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
    // Pre-estimate capacity to avoid repeated reallocations
    use std::fmt::Write;
    let estimated_size: usize = parts.iter().map(|p| p.mesh.vertices.len() * 60 + 200).sum();
    let mut json = String::with_capacity(estimated_size);
    json.push_str("{\"parts\":[");
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let (positions, normals) = part.mesh.to_flat_arrays();
        write!(
            json,
            "{{\"name\":\"{}\",\"color\":[{},{},{}],\"spanStart\":{},\"spanEnd\":{},\"posLen\":{},\"norLen\":{}",
            part.name,
            part.color[0],
            part.color[1],
            part.color[2],
            part.span.start,
            part.span.end,
            positions.len(),
            normals.len()
        )
        .unwrap();

        json.push_str(",\"positions\":[");
        for (j, v) in positions.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            write!(json, "{v}").unwrap();
        }
        json.push_str("],\"normals\":[");
        for (j, v) in normals.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            write!(json, "{v}").unwrap();
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
        ferncad_core::types::Value::Shape(tracked) => {
            ferncad_cad::step::export_step_bytes(&tracked.node)
                .map_err(|e| JsValue::from_str(&format!("STEP export error: {e}")))
        }
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

/// Get completion items for the given source at cursor position.
///
/// Returns a JSON array of completion items with label, kind, detail, doc, and boost.
#[wasm_bindgen]
pub fn get_completions(source: &str, cursor_pos: usize) -> String {
    let items = ferncad_core::completion::get_completions(source, cursor_pos);
    ferncad_core::completion::completions_to_json(&items)
}

// ── Multi-file API ──────────────────────────────────────────────────

/// Parse a JSON object `{"filename": "source", ...}` into key-value pairs.
///
/// Minimal parser — handles string escapes (`\"`, `\\`, `\n`, `\t`) but no
/// nested objects or non-string values. Returns empty vec on malformed input.
fn parse_files_json(json: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let bytes = json.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // Skip to opening '{'
    while i < len && bytes[i] != b'{' {
        i += 1;
    }
    i += 1; // skip '{'

    loop {
        // Skip whitespace and commas
        while i < len
            && (bytes[i] == b' '
                || bytes[i] == b'\n'
                || bytes[i] == b'\r'
                || bytes[i] == b'\t'
                || bytes[i] == b',')
        {
            i += 1;
        }
        if i >= len || bytes[i] == b'}' {
            break;
        }

        // Parse key string
        if bytes[i] != b'"' {
            break;
        }
        let key = match parse_json_string(bytes, &mut i) {
            Some(s) => s,
            None => break,
        };

        // Skip ':'
        while i < len && bytes[i] != b':' {
            i += 1;
        }
        i += 1; // skip ':'

        // Skip whitespace
        while i < len
            && (bytes[i] == b' ' || bytes[i] == b'\n' || bytes[i] == b'\r' || bytes[i] == b'\t')
        {
            i += 1;
        }

        // Parse value string
        if i >= len || bytes[i] != b'"' {
            break;
        }
        let value = match parse_json_string(bytes, &mut i) {
            Some(s) => s,
            None => break,
        };

        result.push((key, value));
    }

    result
}

/// Parse a JSON string starting at `bytes[*pos]` (which must be `"`).
/// Advances `*pos` past the closing `"`. Returns `None` on error.
fn parse_json_string(bytes: &[u8], pos: &mut usize) -> Option<String> {
    let len = bytes.len();
    if *pos >= len || bytes[*pos] != b'"' {
        return None;
    }
    *pos += 1; // skip opening '"'
    let mut s = String::new();
    while *pos < len {
        let ch = bytes[*pos];
        if ch == b'"' {
            *pos += 1; // skip closing '"'
            return Some(s);
        }
        if ch == b'\\' && *pos + 1 < len {
            *pos += 1;
            match bytes[*pos] {
                b'"' => s.push('"'),
                b'\\' => s.push('\\'),
                b'n' => s.push('\n'),
                b't' => s.push('\t'),
                b'r' => s.push('\r'),
                b'/' => s.push('/'),
                other => {
                    s.push('\\');
                    s.push(other as char);
                }
            }
        } else {
            s.push(ch as char);
        }
        *pos += 1;
    }
    None // unterminated string
}

/// Create an evaluator with user modules registered from the files map.
fn make_evaluator_with_files(files: &[(String, String)]) -> ferncad_core::evaluator::Evaluator {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    for (name, source) in files {
        evaluator
            .module_loader_mut()
            .add_user_module(name.clone(), source.clone());
    }
    evaluator
}

/// Evaluate with project files context and return per-part mesh data as JSON.
///
/// `files_json` is `{"filename.fern": "source...", ...}`.
/// The `main_file` key is looked up in the files map and evaluated as the entry point.
#[wasm_bindgen]
pub fn evaluate_parts_with_files(main_file: &str, files_json: &str) -> Result<String, JsValue> {
    let files = parse_files_json(files_json);
    let main_source = files
        .iter()
        .find(|(name, _)| name == main_file)
        .map(|(_, src)| src.as_str())
        .ok_or_else(|| {
            JsValue::from_str(&format!("main file '{main_file}' not found in files map"))
        })?
        .to_string();

    let parts =
        ferncad_cad::assembly_realize::eval_and_realize_parts_with_files(&main_source, &files)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Manual JSON construction with pre-allocated buffer
    use std::fmt::Write;
    let estimated_size: usize = parts.iter().map(|p| p.mesh.vertices.len() * 60 + 200).sum();
    let mut json = String::with_capacity(estimated_size);
    json.push_str("{\"parts\":[");
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        let (positions, normals) = part.mesh.to_flat_arrays();
        write!(
            json,
            "{{\"name\":\"{}\",\"color\":[{},{},{}],\"spanStart\":{},\"spanEnd\":{},\"posLen\":{},\"norLen\":{}",
            part.name,
            part.color[0],
            part.color[1],
            part.color[2],
            part.span.start,
            part.span.end,
            positions.len(),
            normals.len()
        )
        .unwrap();
        json.push_str(",\"positions\":[");
        for (j, v) in positions.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            write!(json, "{v}").unwrap();
        }
        json.push_str("],\"normals\":[");
        for (j, v) in normals.iter().enumerate() {
            if j > 0 {
                json.push(',');
            }
            write!(json, "{v}").unwrap();
        }
        json.push_str("]}");
    }
    json.push_str("]}");
    Ok(json)
}

/// Check syntax with project files context.
#[wasm_bindgen]
pub fn check_syntax_with_files(source: &str, files_json: &str) -> String {
    let files = parse_files_json(files_json);
    let mut evaluator = make_evaluator_with_files(&files);
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

/// Export STL with project files context.
#[wasm_bindgen]
pub fn export_stl_with_files(main_file: &str, files_json: &str) -> Result<Vec<u8>, JsValue> {
    let files = parse_files_json(files_json);
    let main_source = files
        .iter()
        .find(|(name, _)| name == main_file)
        .map(|(_, src)| src.as_str())
        .ok_or_else(|| JsValue::from_str(&format!("main file '{main_file}' not found")))?
        .to_string();

    let mut evaluator = make_evaluator_with_files(&files);
    let result = evaluator
        .eval_source(&main_source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Realize the mesh for STL export
    let parts = match result {
        ferncad_core::types::Value::Assembly(assembly) => {
            ferncad_cad::assembly_realize::realize_assembly_with_resolution(
                &assembly,
                evaluator.resolution(),
            )
            .map_err(|e| JsValue::from_str(&e.to_string()))?
        }
        ferncad_core::types::Value::Shape(tracked) => {
            let mesh = ferncad_cad::realize::realize_with_resolution(
                &tracked.node,
                evaluator.resolution(),
            )
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
            vec![ferncad_cad::assembly_realize::PartMesh {
                name: "shape".to_string(),
                mesh,
                color: [0.53, 0.53, 0.80],
                span: tracked.span.clone(),
            }]
        }
        _ => {
            return Err(JsValue::from_str(
                "STL export requires a shape or assembly expression",
            ))
        }
    };

    if parts.len() == 1 {
        export::export_stl_bytes(&parts[0].mesh)
            .map_err(|e| JsValue::from_str(&format!("STL export error: {e}")))
    } else {
        // Multiple parts — merge into one mesh for export
        let mut combined = parts[0].mesh.clone();
        for part in &parts[1..] {
            combined.merge(&part.mesh);
        }
        export::export_stl_bytes(&combined)
            .map_err(|e| JsValue::from_str(&format!("STL export error: {e}")))
    }
}

/// Export STEP with project files context.
#[wasm_bindgen]
pub fn export_step_with_files(main_file: &str, files_json: &str) -> Result<Vec<u8>, JsValue> {
    let files = parse_files_json(files_json);
    let main_source = files
        .iter()
        .find(|(name, _)| name == main_file)
        .map(|(_, src)| src.as_str())
        .ok_or_else(|| JsValue::from_str(&format!("main file '{main_file}' not found")))?
        .to_string();

    let mut evaluator = make_evaluator_with_files(&files);
    let result = evaluator
        .eval_source(&main_source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    match result {
        ferncad_core::types::Value::Shape(tracked) => {
            ferncad_cad::step::export_step_bytes(&tracked.node)
                .map_err(|e| JsValue::from_str(&format!("STEP export error: {e}")))
        }
        _ => Err(JsValue::from_str("STEP export requires a shape expression")),
    }
}

/// Extract params with project files context.
#[wasm_bindgen]
pub fn extract_params_with_files(main_file: &str, files_json: &str) -> String {
    let files = parse_files_json(files_json);
    let main_source = match files.iter().find(|(name, _)| name == main_file) {
        Some((_, src)) => src.clone(),
        None => return r#"{"parts":[]}"#.to_string(),
    };

    let mut evaluator = make_evaluator_with_files(&files);
    if evaluator.eval_source(&main_source).is_err() {
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

/// Get completions with project files context.
#[wasm_bindgen]
pub fn get_completions_with_files(source: &str, cursor_pos: usize, files_json: &str) -> String {
    let files = parse_files_json(files_json);
    let items = ferncad_core::completion::get_completions_with_files(source, cursor_pos, &files);
    ferncad_core::completion::completions_to_json(&items)
}
