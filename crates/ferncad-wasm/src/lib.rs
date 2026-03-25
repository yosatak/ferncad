//! ferncad WASM バインディング
//!
//! ブラウザ向けの薄いラッパー。
//! `wasm_bindgen` でエクスポートする API を最小限に保つ。

use wasm_bindgen::prelude::*;

use ferncad_cad::export;
use ferncad_cad::realize;

/// ferncad ソースコードを評価し、メッシュデータ（positions + normals の Float32Array）を返す
///
/// 戻り値は `Float32Array` で、前半が positions、後半が normals。
/// `positions.length / 3` が頂点数、`positions.length / 9` が三角形数。
#[wasm_bindgen]
pub fn evaluate(source: &str) -> Result<js_sys::Float32Array, JsValue> {
    let mesh = realize::eval_and_realize(source).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let (positions, normals) = mesh.to_flat_arrays();

    // positions と normals を連結して1つの Float32Array で返す
    let mut combined = Vec::with_capacity(positions.len() + normals.len());
    combined.extend_from_slice(&positions);
    combined.extend_from_slice(&normals);

    Ok(js_sys::Float32Array::from(&combined[..]))
}

/// メッシュデータの頂点数を返す（positions の長さ / 3）
/// JS 側で positions と normals を分割するための情報
#[wasm_bindgen]
pub fn get_positions_length(source: &str) -> Result<usize, JsValue> {
    let mesh = realize::eval_and_realize(source).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let (positions, _) = mesh.to_flat_arrays();
    Ok(positions.len())
}

/// ferncad ソースコードを評価し、STL バイナリを返す
#[wasm_bindgen]
pub fn export_stl(source: &str) -> Result<Vec<u8>, JsValue> {
    let mesh = realize::eval_and_realize(source).map_err(|e| JsValue::from_str(&e.to_string()))?;

    export::export_stl_bytes(&mesh)
        .map_err(|e| JsValue::from_str(&format!("STLエクスポートエラー: {e}")))
}

/// ferncad ソースコードを評価し、STEP バイナリを返す
///
/// BREP 対応の形状のみエクスポート可能（Box, Cylinder, Cone, Prism, Union, Intersection）。
/// Sphere, Torus, Difference は現在未対応。
#[wasm_bindgen]
pub fn export_step(source: &str) -> Result<Vec<u8>, JsValue> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator
        .eval_source(source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    match result {
        ferncad_core::types::Value::Shape(node) => ferncad_cad::step::export_step_bytes(&node)
            .map_err(|e| JsValue::from_str(&e.to_string())),
        _ => Err(JsValue::from_str("STEP エクスポートには形状が必要です")),
    }
}

/// ferncad ソースコードを評価し、パーツ別メッシュデータを JSON で返す
///
/// アセンブリの場合はパーツごとに分割されたメッシュ・色情報を返す。
/// 単一形状の場合は1パーツとして返す。
///
/// 戻り値 JSON: `{ "parts": [{ "name": "...", "positions": [...], "normals": [...], "color": [r,g,b] }] }`
#[wasm_bindgen]
pub fn evaluate_parts(source: &str) -> Result<String, JsValue> {
    let parts = ferncad_cad::assembly_realize::eval_and_realize_parts(source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // JSON を手動構築（serde なしで軽量に）
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

        // 位置データ
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

/// ferncad ソースコードの構文チェックのみを行う
///
/// エラーがある場合はエラーメッセージを返す。成功時は空文字列。
#[wasm_bindgen]
pub fn check_syntax(source: &str) -> String {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    match evaluator.eval_source(source) {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

/// ferncad ソースコードを評価し、結果の文字列表現を返す
#[wasm_bindgen]
pub fn eval_to_string(source: &str) -> Result<String, JsValue> {
    let mut evaluator = ferncad_core::evaluator::Evaluator::new();
    let result = evaluator
        .eval_source(source)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(format!("{result}"))
}
