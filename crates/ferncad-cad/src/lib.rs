//! ferncad CAD カーネル
//!
//! プリミティブ生成・CSG 演算・変換・STL エクスポート・BREP パイプラインを提供する。

pub mod brep;
pub mod bsp;
pub mod export;
pub mod mesh;
pub mod primitives;
pub mod realize;
pub mod transform;
