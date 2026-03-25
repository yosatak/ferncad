//! ferncad 値型・形状ノード定義
//!
//! 評価器が扱うすべての値と、CSG ツリーのノード型を定義する。

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::error::SourceLocation;

/// 評価器が返す値の型
#[derive(Debug, Clone)]
pub enum Value {
    /// 整数
    Int(i64),
    /// 浮動小数点数
    Float(f64),
    /// 長さ（mm 換算で正規化済み）
    Length(f64),
    /// 角度（rad 換算で正規化済み）
    Angle(f64),
    /// 3D ベクトル
    Vec3([f64; 3]),
    /// 3D 点
    Point3([f64; 3]),
    /// 文字列
    Str(String),
    /// シンボル（引用時のみ値として存在）
    Symbol(String),
    /// キーワード
    Keyword(String),
    /// ブール値（`t` = true, `nil` = false）
    Bool(bool),
    /// 空値
    Nil,
    /// リスト
    List(Vec<Value>),
    /// CSG 形状ノード
    Shape(Arc<ShapeNode>),
    /// 組み込み関数
    BuiltinFn(BuiltinFnDef),
    /// ユーザー定義関数（クロージャ）
    Lambda(Arc<LambdaDef>),
    /// パーツ定義
    PartDef(Arc<PartDef>),
    /// 面への参照
    FaceRef(crate::face::FaceRef),
    /// 軸への参照
    AxisRef(crate::face::AxisRef),
}

impl Value {
    /// 数値として取得する（Int → f64 変換を含む）
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::Float(f) => Some(*f),
            Value::Length(f) => Some(*f),
            Value::Angle(f) => Some(*f),
            _ => None,
        }
    }

    /// 真偽値として評価する（CL 準拠: nil と Bool(false) のみ偽）
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    /// 型アノテーション名に対して値が適合するか検査する
    ///
    /// 数値型（int, float）は length や angle にも暗黙変換可能とする。
    pub fn matches_type(&self, type_name: &str) -> bool {
        match type_name {
            "int" => matches!(self, Value::Int(_)),
            "float" => matches!(self, Value::Float(_) | Value::Int(_)),
            "number" => self.as_number().is_some(),
            "length" => matches!(self, Value::Length(_) | Value::Float(_) | Value::Int(_)),
            "angle" => matches!(self, Value::Angle(_) | Value::Float(_) | Value::Int(_)),
            "vec3" => matches!(self, Value::Vec3(_)),
            "point3" => matches!(self, Value::Point3(_)),
            "string" => matches!(self, Value::Str(_)),
            "keyword" => matches!(self, Value::Keyword(_)),
            "bool" => matches!(self, Value::Bool(_)),
            "shape" => matches!(self, Value::Shape(_)),
            "list" => matches!(self, Value::List(_)),
            _ => true, // 不明な型名は常に適合（Phase 2 の安全策）
        }
    }

    /// 型名を日本語で返す
    pub fn type_name_ja(&self) -> &'static str {
        match self {
            Value::Int(_) => "整数",
            Value::Float(_) => "浮動小数点数",
            Value::Length(_) => "長さ",
            Value::Angle(_) => "角度",
            Value::Vec3(_) => "ベクトル",
            Value::Point3(_) => "点",
            Value::Str(_) => "文字列",
            Value::Symbol(_) => "シンボル",
            Value::Keyword(_) => "キーワード",
            Value::Bool(_) => "ブール値",
            Value::Nil => "nil",
            Value::List(_) => "リスト",
            Value::Shape(_) => "形状",
            Value::BuiltinFn(_) => "組み込み関数",
            Value::Lambda(_) => "関数",
            Value::PartDef(_) => "パーツ定義",
            Value::FaceRef(_) => "面参照",
            Value::AxisRef(_) => "軸参照",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Length(v) => write!(f, "{v}mm"),
            Value::Angle(v) => write!(f, "{v}rad"),
            Value::Vec3(v) => write!(f, "#v({} {} {})", v[0], v[1], v[2]),
            Value::Point3(v) => write!(f, "#p({} {} {})", v[0], v[1], v[2]),
            Value::Str(s) => write!(f, "\"{s}\""),
            Value::Symbol(s) => write!(f, "{s}"),
            Value::Keyword(k) => write!(f, ":{k}"),
            Value::Bool(true) => write!(f, "t"),
            Value::Bool(false) => write!(f, "nil"),
            Value::Nil => write!(f, "nil"),
            Value::List(items) => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Value::Shape(_) => write!(f, "<shape>"),
            Value::BuiltinFn(def) => write!(f, "<builtin:{}>", def.name),
            Value::Lambda(_) => write!(f, "<lambda>"),
            Value::PartDef(def) => write!(f, "<part:{}>", def.name),
            Value::FaceRef(r) => write!(f, "<face:{}:{}>", r.instance_name, r.face_name),
            Value::AxisRef(r) => write!(f, "<axis:{}:{}>", r.instance_name, r.axis_name),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) | (Value::Float(b), Value::Int(a)) => {
                (*a as f64) == *b
            }
            (Value::Length(a), Value::Length(b)) => a == b,
            (Value::Angle(a), Value::Angle(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Symbol(a), Value::Symbol(b)) => a == b,
            (Value::Keyword(a), Value::Keyword(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Nil, Value::Nil) => true,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Vec3(a), Value::Vec3(b)) => a == b,
            (Value::Point3(a), Value::Point3(b)) => a == b,
            (Value::FaceRef(a), Value::FaceRef(b)) => a == b,
            (Value::AxisRef(a), Value::AxisRef(b)) => a == b,
            _ => false,
        }
    }
}

/// 組み込み関数の定義
#[derive(Clone)]
pub struct BuiltinFnDef {
    /// 関数名
    pub name: String,
    /// 実装
    pub func: fn(&[Value], &SourceLocation) -> Result<Value, crate::error::FernError>,
}

impl fmt::Debug for BuiltinFnDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BuiltinFn({})", self.name)
    }
}

/// ユーザー定義関数
#[derive(Debug, Clone)]
pub struct LambdaDef {
    /// パラメータ名リスト
    pub params: Vec<String>,
    /// 関数本体（S式のリスト）
    pub body: Vec<Value>,
    /// 定義時の環境 ID（クロージャ用）
    pub env_id: usize,
}

/// パーツ定義
#[derive(Debug, Clone)]
pub struct PartDef {
    /// パーツ名
    pub name: String,
    /// ドキュメント文字列
    pub docstring: String,
    /// メタデータ
    pub meta: HashMap<String, Value>,
    /// パラメータ仕様
    pub params: Vec<ParamSpec>,
    /// 名前付き面の宣言
    pub faces: Vec<crate::face::FaceSpec>,
    /// 名前付き軸の宣言
    pub axes: Vec<crate::face::AxisSpec>,
    /// 本体（遅延評価用 S 式）
    pub body: Vec<Value>,
    /// 定義時の環境 ID
    pub env_id: usize,
}

/// パーツパラメータの仕様
#[derive(Debug, Clone)]
pub struct ParamSpec {
    /// パラメータ名
    pub name: String,
    /// 型アノテーション（オプション）
    pub type_annotation: Option<String>,
    /// デフォルト値（オプション）
    pub default: Option<Value>,
    /// ドキュメント文字列（オプション）
    pub doc: Option<String>,
}

/// デフォルトのセグメント数
/// デフォルトのセグメント数（BSP CSG のパフォーマンスとのバランス）
pub const DEFAULT_SEGMENTS: u32 = 16;

/// CSG ツリーのノード（イミュータブル・参照カウント）
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeNode {
    // === プリミティブ ===
    /// 直方体
    Box { width: f64, depth: f64, height: f64 },
    /// 球
    Sphere { radius: f64, segments: u32 },
    /// 円柱
    Cylinder {
        radius: f64,
        height: f64,
        segments: u32,
    },
    /// 円錐
    Cone {
        radius_bottom: f64,
        radius_top: f64,
        height: f64,
        segments: u32,
    },
    /// トーラス
    Torus {
        radius_major: f64,
        radius_minor: f64,
        segments: u32,
    },
    /// 多角柱
    Prism {
        sides: u32,
        radius: f64,
        height: f64,
    },

    // === CSG 演算 ===
    /// 合算
    Union { children: Vec<Arc<ShapeNode>> },
    /// 差分（第1要素から他を差し引く）
    Difference {
        base: Arc<ShapeNode>,
        cutters: Vec<Arc<ShapeNode>>,
    },
    /// 交差
    Intersection { children: Vec<Arc<ShapeNode>> },

    // === 変換 ===
    /// 移動
    Translate {
        shape: Arc<ShapeNode>,
        offset: [f64; 3],
    },
    /// 回転（軸 + 角度 rad）
    Rotate {
        shape: Arc<ShapeNode>,
        axis: [f64; 3],
        angle_rad: f64,
    },
    /// スケール
    Scale {
        shape: Arc<ShapeNode>,
        factors: [f64; 3],
    },
}
