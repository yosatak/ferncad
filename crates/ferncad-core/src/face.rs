//! 面・軸の参照型定義
//!
//! defpart の `:faces` / `:axes` 宣言と、アセンブリ制約で使う参照型。

/// パーツの名前付き面の仕様
#[derive(Debug, Clone, PartialEq)]
pub struct FaceSpec {
    /// 面の名前
    pub name: String,
    /// ドキュメント文字列
    pub doc: Option<String>,
    /// 面の法線ベクトルのヒント（明示指定時）
    pub normal: Option<[f64; 3]>,
}

/// パーツの名前付き軸の仕様
#[derive(Debug, Clone, PartialEq)]
pub struct AxisSpec {
    /// 軸の名前
    pub name: String,
    /// ドキュメント文字列
    pub doc: Option<String>,
    /// 軸の方向ベクトルのヒント
    pub direction: Option<[f64; 3]>,
}

/// 面への参照（アセンブリ制約で使用）
#[derive(Debug, Clone, PartialEq)]
pub struct FaceRef {
    /// パーツインスタンスの名前
    pub instance_name: String,
    /// 面の名前
    pub face_name: String,
}

/// 軸への参照（アセンブリ制約で使用）
#[derive(Debug, Clone, PartialEq)]
pub struct AxisRef {
    /// パーツインスタンスの名前
    pub instance_name: String,
    /// 軸の名前
    pub axis_name: String,
}
