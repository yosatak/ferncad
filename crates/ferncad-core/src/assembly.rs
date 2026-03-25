//! アセンブリ・制約の型定義
//!
//! 複数パーツを配置し、面・軸制約で位置決めするデータ構造。

use std::collections::HashMap;
use std::sync::Arc;

use crate::face::{AxisRef, FaceRef};
use crate::types::{PartDef, Value};

/// アセンブリ定義
#[derive(Debug, Clone)]
pub struct AssemblyDef {
    /// アセンブリ名
    pub name: String,
    /// ドキュメント文字列
    pub docstring: String,
    /// 配置されたパーツ
    pub parts: Vec<PartInstance>,
    /// 制約リスト
    pub constraints: Vec<Constraint>,
}

/// パーツインスタンス（アセンブリ内の配置済みパーツ）
#[derive(Debug, Clone)]
pub struct PartInstance {
    /// アセンブリ内での名前
    pub name: String,
    /// パーツ定義への参照
    pub part_def: Arc<PartDef>,
    /// パラメータ値
    pub params: HashMap<String, Value>,
    /// 形状ノード（評価済み）
    pub shape: Option<Arc<crate::types::ShapeNode>>,
    /// 4x4 変換行列（列優先）
    pub transform: [f64; 16],
    /// 色（RGB 0-1）
    pub color: [f64; 3],
}

impl PartInstance {
    /// 単位行列で初期化された変換を持つインスタンスを作成する
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

    /// 平行移動を適用する
    pub fn translate(&mut self, offset: [f64; 3]) {
        self.transform[12] += offset[0];
        self.transform[13] += offset[1];
        self.transform[14] += offset[2];
    }
}

/// 制約の種類
#[derive(Debug, Clone)]
pub enum Constraint {
    /// 面を合わせる（密着）
    Mate {
        face1: FaceRef,
        face2: FaceRef,
        offset: f64,
    },
    /// 軸を揃える
    AlignAxis { axis1: AxisRef, axis2: AxisRef },
    /// はめ合い
    Fit {
        shaft: FaceRef,
        hole: FaceRef,
        clearance: f64,
        fit_type: FitType,
    },
    /// ジョイント
    Joint {
        joint_type: JointType,
        parts: Vec<String>,
    },
}

/// はめ合いの種類
#[derive(Debug, Clone, PartialEq)]
pub enum FitType {
    /// すきまばめ
    Clearance,
    /// しまりばめ
    Interference,
    /// 中間ばめ
    Transition,
}

/// ジョイントの種類
#[derive(Debug, Clone, PartialEq)]
pub enum JointType {
    /// 固定
    Fixed,
    /// 回転ジョイント
    Revolute,
    /// 直線ジョイント
    Prismatic,
}

/// 4x4 単位行列を返す
pub fn identity_matrix() -> [f64; 16] {
    [
        1.0, 0.0, 0.0, 0.0, // col 0
        0.0, 1.0, 0.0, 0.0, // col 1
        0.0, 0.0, 1.0, 0.0, // col 2
        0.0, 0.0, 0.0, 1.0, // col 3
    ]
}

/// パーツ色パレット（自動割り当て用）
const PART_COLORS: &[[f64; 3]] = &[
    [0.53, 0.53, 0.80], // 青紫
    [0.80, 0.53, 0.53], // 赤系
    [0.53, 0.80, 0.53], // 緑系
    [0.80, 0.73, 0.53], // 黄系
    [0.53, 0.73, 0.80], // 水色
    [0.73, 0.53, 0.80], // 紫系
    [0.80, 0.60, 0.53], // オレンジ系
    [0.53, 0.80, 0.73], // ターコイズ
];

/// パーツインデックスから色を取得する
pub fn part_color(index: usize) -> [f64; 3] {
    PART_COLORS[index % PART_COLORS.len()]
}
