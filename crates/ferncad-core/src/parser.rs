//! ferncad 構文解析器（Parser）
//!
//! トークン列から S 式ツリー（`Value`）を生成する再帰下降パーサー。

use logos::Span;

use crate::error::{FernError, FernResult, SourceLocation};
use crate::lexer::{span_to_location, tokenize, SpannedToken, Token};
use crate::types::Value;

/// パーサーの状態
struct Parser<'a> {
    tokens: Vec<SpannedToken>,
    source: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    /// 新しいパーサーを作成する
    fn new(tokens: Vec<SpannedToken>, source: &'a str) -> Self {
        Self {
            tokens,
            source,
            pos: 0,
        }
    }

    /// 現在のトークンを参照する
    fn peek(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }

    /// 現在のトークンを消費して次へ進む。インデックスを返す。
    fn advance(&mut self) -> Option<usize> {
        if self.pos < self.tokens.len() {
            let idx = self.pos;
            self.pos += 1;
            Some(idx)
        } else {
            None
        }
    }

    /// 指定位置のトークンを参照する
    fn token_at(&self, idx: usize) -> &Token {
        &self.tokens[idx].token
    }

    /// 指定位置のスパンを参照する
    fn span_at(&self, idx: usize) -> &Span {
        &self.tokens[idx].span
    }

    /// 現在位置のソースロケーションを取得する
    fn current_location(&self) -> SourceLocation {
        if let Some(st) = self.tokens.get(self.pos) {
            span_to_location(self.source, &st.span)
        } else if let Some(st) = self.tokens.last() {
            span_to_location(self.source, &st.span)
        } else {
            SourceLocation { line: 1, col: 1 }
        }
    }

    /// 指定位置のソースロケーションを取得する
    fn location_at(&self, idx: usize) -> SourceLocation {
        span_to_location(self.source, self.span_at(idx))
    }

    /// 単一の S 式をパースする
    fn parse_expr(&mut self) -> FernResult<Value> {
        let idx = self.advance().ok_or_else(|| FernError::ParseError {
            loc: self.current_location(),
            message: "予期しない入力の終了です".to_string(),
        })?;

        match self.token_at(idx).clone() {
            Token::LParen => self.parse_list(idx),
            Token::RParen => {
                let loc = self.location_at(idx);
                Err(FernError::UnmatchedCloseParen { loc })
            }
            Token::Int(n) => Ok(Value::Int(n)),
            Token::Float(f) => Ok(Value::Float(f)),
            Token::Str(s) => Ok(Value::Str(s)),
            Token::Symbol(s) => match s.as_str() {
                "t" => Ok(Value::Bool(true)),
                "nil" => Ok(Value::Nil),
                _ => Ok(Value::Symbol(s)),
            },
            Token::Keyword(k) => Ok(Value::Keyword(k)),
            Token::Minus => Ok(Value::Symbol("-".to_string())),
            Token::Slash => Ok(Value::Symbol("/".to_string())),
            Token::TypeAnnotation => Ok(Value::Symbol("::".to_string())),
            Token::UnitPrefix => self.parse_special_literal("u"),
            Token::AnglePrefix => self.parse_special_literal("a"),
            Token::Vec3Prefix => self.parse_special_literal("v"),
            Token::Point3Prefix => self.parse_special_literal("p"),
        }
    }

    /// リスト（開き括弧の後）をパースする
    fn parse_list(&mut self, open_pos: usize) -> FernResult<Value> {
        let mut items = Vec::new();

        loop {
            match self.peek() {
                Some(st) if st.token == Token::RParen => {
                    self.advance(); // `)` を消費
                    return Ok(Value::List(items));
                }
                Some(_) => {
                    let item = self.parse_expr()?;
                    items.push(item);
                }
                None => {
                    let loc = self.location_at(open_pos);
                    return Err(FernError::UnmatchedOpenParen { loc });
                }
            }
        }
    }

    /// 特殊リテラル `#u(...)`, `#a(...)`, `#v(...)`, `#p(...)` をパースする
    fn parse_special_literal(&mut self, kind: &str) -> FernResult<Value> {
        let loc = self.current_location();
        match self.peek() {
            Some(st) if st.token == Token::LParen => {
                self.advance(); // `(` を消費
            }
            _ => {
                return Err(FernError::ParseError {
                    loc,
                    message: format!("#{kind} の後に `(` が必要です。例: #{kind}(値 ...)"),
                });
            }
        }

        match kind {
            "u" => self.parse_unit_literal(),
            "a" => self.parse_angle_literal(),
            "v" => self.parse_vec3_literal(),
            "p" => self.parse_point3_literal(),
            _ => unreachable!(),
        }
    }

    /// `#u(値 :単位)` をパースする
    fn parse_unit_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let value_expr = self.parse_expr()?;
        let value = value_expr
            .as_number()
            .ok_or_else(|| FernError::ParseError {
                loc: loc.clone(),
                message: "単位リテラル `#u` の第1引数は数値が必要です".to_string(),
            })?;

        let unit_loc = self.current_location();
        let unit_expr = self.parse_expr()?;
        let unit = match &unit_expr {
            Value::Keyword(k) => k.clone(),
            _ => {
                return Err(FernError::ParseError {
                    loc: unit_loc,
                    message: "単位リテラル `#u` の第2引数はキーワード（例: :mm）が必要です"
                        .to_string(),
                });
            }
        };

        // 閉じ括弧
        self.expect_rparen("単位リテラル #u")?;

        // mm への変換
        let mm_value = convert_to_mm(value, &unit)
            .map_err(|msg| FernError::ParseError { loc, message: msg })?;

        Ok(Value::Length(mm_value))
    }

    /// `#a(値 :単位)` をパースする
    fn parse_angle_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let value_expr = self.parse_expr()?;
        let value = value_expr
            .as_number()
            .ok_or_else(|| FernError::ParseError {
                loc: loc.clone(),
                message: "角度リテラル `#a` の第1引数は数値が必要です".to_string(),
            })?;

        let unit_loc = self.current_location();
        let unit_expr = self.parse_expr()?;
        let unit = match &unit_expr {
            Value::Keyword(k) => k.clone(),
            _ => {
                return Err(FernError::ParseError {
                    loc: unit_loc,
                    message: "角度リテラル `#a` の第2引数はキーワード（例: :deg）が必要です"
                        .to_string(),
                });
            }
        };

        // 閉じ括弧
        self.expect_rparen("角度リテラル #a")?;

        // rad への変換
        let rad_value = convert_to_rad(value, &unit)
            .map_err(|msg| FernError::ParseError { loc, message: msg })?;

        Ok(Value::Angle(rad_value))
    }

    /// `#v(x y z)` をパースする
    fn parse_vec3_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let x = self.parse_number_component("ベクトル #v", "x", &loc)?;
        let y = self.parse_number_component("ベクトル #v", "y", &loc)?;
        let z = self.parse_number_component("ベクトル #v", "z", &loc)?;
        self.expect_rparen("ベクトルリテラル #v")?;
        Ok(Value::Vec3([x, y, z]))
    }

    /// `#p(x y z)` をパースする
    fn parse_point3_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let x = self.parse_number_component("点 #p", "x", &loc)?;
        let y = self.parse_number_component("点 #p", "y", &loc)?;
        let z = self.parse_number_component("点 #p", "z", &loc)?;
        self.expect_rparen("点リテラル #p")?;
        Ok(Value::Point3([x, y, z]))
    }

    /// 数値コンポーネントをパースするヘルパー
    fn parse_number_component(
        &mut self,
        context: &str,
        component: &str,
        loc: &SourceLocation,
    ) -> FernResult<f64> {
        let expr = self.parse_expr()?;
        expr.as_number().ok_or_else(|| FernError::ParseError {
            loc: loc.clone(),
            message: format!("{context} の {component} 成分は数値が必要です"),
        })
    }

    /// 閉じ括弧を期待して消費する
    fn expect_rparen(&mut self, context: &str) -> FernResult<()> {
        let loc = self.current_location();
        match self.peek() {
            Some(st) if st.token == Token::RParen => {
                self.advance();
                Ok(())
            }
            _ => Err(FernError::ParseError {
                loc,
                message: format!("{context} の閉じ括弧 `)` が必要です"),
            }),
        }
    }
}

/// 長さ単位を mm に変換する
fn convert_to_mm(value: f64, unit: &str) -> Result<f64, String> {
    /// 1 インチ = 25.4 mm
    const INCH_TO_MM: f64 = 25.4;
    /// 1 フィート = 304.8 mm
    const FOOT_TO_MM: f64 = 304.8;

    match unit {
        "mm" => Ok(value),
        "cm" => Ok(value * 10.0),
        "m" => Ok(value * 1000.0),
        "inch" => Ok(value * INCH_TO_MM),
        "ft" => Ok(value * FOOT_TO_MM),
        _ => Err(format!(
            "未対応の長さ単位 `:{unit}` です。対応単位: :mm, :cm, :m, :inch, :ft"
        )),
    }
}

/// 角度単位を rad に変換する
fn convert_to_rad(value: f64, unit: &str) -> Result<f64, String> {
    match unit {
        "rad" => Ok(value),
        "deg" => Ok(value * std::f64::consts::PI / 180.0),
        "turn" => Ok(value * 2.0 * std::f64::consts::PI),
        _ => Err(format!(
            "未対応の角度単位 `:{unit}` です。対応単位: :rad, :deg, :turn"
        )),
    }
}

/// ソースコードを S 式のリストにパースする
///
/// # Errors
///
/// 構文エラーがある場合、`FernError` を返す。
pub fn parse(source: &str) -> FernResult<Vec<Value>> {
    let tokens = tokenize(source)?;
    let mut parser = Parser::new(tokens, source);
    let mut exprs = Vec::new();

    while parser.peek().is_some() {
        let expr = parser.parse_expr()?;
        exprs.push(expr);
    }

    Ok(exprs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_list() {
        let result = parse("(+ 1 2)").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Value::List(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Symbol("+".to_string()));
                assert_eq!(items[1], Value::Int(1));
                assert_eq!(items[2], Value::Int(2));
            }
            other => panic!("期待: List、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_nested_list() {
        let result = parse("(+ (* 2 3) 4)").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Value::List(items) => {
                assert_eq!(items.len(), 3);
                assert!(matches!(&items[1], Value::List(_)));
            }
            other => panic!("期待: List、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_empty_list() {
        let result = parse("()").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Value::List(vec![]));
    }

    #[test]
    fn test_parse_t_nil() {
        let result = parse("t nil").unwrap();
        assert_eq!(result[0], Value::Bool(true));
        assert_eq!(result[1], Value::Nil);
    }

    #[test]
    fn test_parse_unit_literal() {
        let result = parse("#u(25.4 :mm)").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Value::Length(v) => assert!((v - 25.4).abs() < 1e-10),
            other => panic!("期待: Length、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_unit_inch_to_mm() {
        let result = parse("#u(1.0 :inch)").unwrap();
        match &result[0] {
            Value::Length(v) => assert!((v - 25.4).abs() < 1e-10),
            other => panic!("期待: Length、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_angle_literal() {
        let result = parse("#a(90 :deg)").unwrap();
        match &result[0] {
            Value::Angle(v) => assert!((v - std::f64::consts::FRAC_PI_2).abs() < 1e-10),
            other => panic!("期待: Angle、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_vec3() {
        let result = parse("#v(1.0 2.0 3.0)").unwrap();
        assert_eq!(result[0], Value::Vec3([1.0, 2.0, 3.0]));
    }

    #[test]
    fn test_parse_point3() {
        let result = parse("#p(10 20 30)").unwrap();
        assert_eq!(result[0], Value::Point3([10.0, 20.0, 30.0]));
    }

    #[test]
    fn test_parse_type_annotation() {
        let result = parse("(x :: length)").unwrap();
        match &result[0] {
            Value::List(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Symbol("x".to_string()));
                assert_eq!(items[1], Value::Symbol("::".to_string()));
                assert_eq!(items[2], Value::Symbol("length".to_string()));
            }
            other => panic!("期待: List、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_keywords() {
        let result = parse(":radius :mm").unwrap();
        assert_eq!(result[0], Value::Keyword("radius".to_string()));
        assert_eq!(result[1], Value::Keyword("mm".to_string()));
    }

    #[test]
    fn test_unmatched_open_paren() {
        let result = parse("(+ 1 2");
        assert!(result.is_err());
        match result.unwrap_err() {
            FernError::UnmatchedOpenParen { loc } => {
                assert_eq!(loc.line, 1);
                assert_eq!(loc.col, 1);
            }
            other => panic!("想定外のエラー: {other:?}"),
        }
    }

    #[test]
    fn test_unmatched_close_paren() {
        let result = parse(")");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FernError::UnmatchedCloseParen { .. }
        ));
    }

    #[test]
    fn test_parse_defvar() {
        let result = parse("(defvar +m3-diameter+ 3.0)").unwrap();
        match &result[0] {
            Value::List(items) => {
                assert_eq!(items[0], Value::Symbol("defvar".to_string()));
                assert_eq!(items[1], Value::Symbol("+m3-diameter+".to_string()));
                assert_eq!(items[2], Value::Float(3.0));
            }
            other => panic!("期待: List、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_multiple_expressions() {
        let result = parse("(defvar x 1) (defvar y 2)").unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_minus_operator() {
        let result = parse("(- 10 3)").unwrap();
        match &result[0] {
            Value::List(items) => {
                assert_eq!(items[0], Value::Symbol("-".to_string()));
                assert_eq!(items[1], Value::Int(10));
                assert_eq!(items[2], Value::Int(3));
            }
            other => panic!("期待: List、実際: {other:?}"),
        }
    }

    #[test]
    fn test_parse_defpart_mvp() {
        let source = r#"(defpart my-part
  "テスト用パーツ"
  :meta (:category :test :material :steel :description "test")
  :params ((size :: length :default 10.0 :doc "サイズ"))
  :body
  (difference
    (box :width size :depth size :height size)
    (sphere :radius (/ size 3))))"#;
        let result = parse(source);
        assert!(
            result.is_ok(),
            "MVP コードのパースに失敗: {:?}",
            result.err()
        );
        let exprs = result.unwrap();
        assert_eq!(exprs.len(), 1);
        match &exprs[0] {
            Value::List(items) => {
                assert_eq!(items[0], Value::Symbol("defpart".to_string()));
                assert_eq!(items[1], Value::Symbol("my-part".to_string()));
                assert_eq!(items[2], Value::Str("テスト用パーツ".to_string()));
            }
            other => panic!("期待: List、実際: {other:?}"),
        }
    }
}
