//! ferncad parser
//!
//! A recursive descent parser that produces an S-expression tree (`Value`) from a token stream.

use logos::Span;

use crate::error::{FernError, FernResult, SourceLocation, SourceSpan};
use crate::lexer::{span_to_location, tokenize, SpannedToken, Token};
use crate::types::Value;

/// Parser state
struct Parser<'a> {
    tokens: Vec<SpannedToken>,
    source: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    /// Create a new parser
    fn new(tokens: Vec<SpannedToken>, source: &'a str) -> Self {
        Self {
            tokens,
            source,
            pos: 0,
        }
    }

    /// Peek at the current token
    fn peek(&self) -> Option<&SpannedToken> {
        self.tokens.get(self.pos)
    }

    /// Consume the current token and advance. Returns the index.
    fn advance(&mut self) -> Option<usize> {
        if self.pos < self.tokens.len() {
            let idx = self.pos;
            self.pos += 1;
            Some(idx)
        } else {
            None
        }
    }

    /// Get a reference to the token at the given position
    fn token_at(&self, idx: usize) -> &Token {
        &self.tokens[idx].token
    }

    /// Get a reference to the span at the given position
    fn span_at(&self, idx: usize) -> &Span {
        &self.tokens[idx].span
    }

    /// Get the source location at the current position
    fn current_location(&self) -> SourceLocation {
        if let Some(st) = self.tokens.get(self.pos) {
            span_to_location(self.source, &st.span)
        } else if let Some(st) = self.tokens.last() {
            span_to_location(self.source, &st.span)
        } else {
            SourceLocation { line: 1, col: 1 }
        }
    }

    /// Get the source location at the given position
    fn location_at(&self, idx: usize) -> SourceLocation {
        span_to_location(self.source, self.span_at(idx))
    }

    /// Parse a single S-expression with its source span
    fn parse_expr_spanned(&mut self) -> FernResult<(Value, Span)> {
        let start = self.peek().map(|st| st.span.start).unwrap_or(0);
        let value = self.parse_expr()?;
        let end = if self.pos > 0 {
            self.span_at(self.pos - 1).end
        } else {
            start
        };
        Ok((value, start..end))
    }

    /// Parse a single S-expression
    fn parse_expr(&mut self) -> FernResult<Value> {
        let idx = self.advance().ok_or_else(|| FernError::ParseError {
            loc: self.current_location(),
            message: "unexpected end of input".to_string(),
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
            Token::Ampersand => {
                // &rest, &key etc. — combine & with next symbol
                if let Some(st) = self.peek() {
                    if let Token::Symbol(s) = &st.token {
                        let combined = format!("&{s}");
                        self.advance();
                        return Ok(Value::Symbol(combined));
                    }
                }
                Ok(Value::Symbol("&".to_string()))
            }
            Token::TypeAnnotation => Ok(Value::Symbol("::".to_string())),
            Token::UnitPrefix => self.parse_special_literal("u"),
            Token::AnglePrefix => self.parse_special_literal("a"),
            Token::Vec3Prefix => self.parse_special_literal("v"),
            Token::Point3Prefix => self.parse_special_literal("p"),
            // Quasiquote reader macros
            Token::Backquote => {
                let expr = self.parse_expr()?;
                Ok(Value::List(vec![
                    Value::Symbol("quasiquote".to_string()),
                    expr,
                ]))
            }
            Token::CommaAt => {
                let expr = self.parse_expr()?;
                Ok(Value::List(vec![
                    Value::Symbol("unquote-splicing".to_string()),
                    expr,
                ]))
            }
            Token::Comma => {
                let expr = self.parse_expr()?;
                Ok(Value::List(vec![
                    Value::Symbol("unquote".to_string()),
                    expr,
                ]))
            }
        }
    }

    /// Parse a list (after the opening parenthesis)
    fn parse_list(&mut self, open_pos: usize) -> FernResult<Value> {
        let mut items = Vec::new();

        loop {
            match self.peek() {
                Some(st) if st.token == Token::RParen => {
                    self.advance(); // consume `)`
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

    /// Parse special literals `#u(...)`, `#a(...)`, `#v(...)`, `#p(...)`
    fn parse_special_literal(&mut self, kind: &str) -> FernResult<Value> {
        let loc = self.current_location();
        match self.peek() {
            Some(st) if st.token == Token::LParen => {
                self.advance(); // consume `(`
            }
            _ => {
                return Err(FernError::ParseError {
                    loc,
                    message: format!(
                        "#{kind} must be followed by `(`. Example: #{kind}(value ...)"
                    ),
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

    /// Parse `#u(value :unit)`
    fn parse_unit_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let value_expr = self.parse_expr()?;
        let value = value_expr
            .as_number()
            .ok_or_else(|| FernError::ParseError {
                loc: loc.clone(),
                message: "unit literal `#u` requires a number as the first argument".to_string(),
            })?;

        let unit_loc = self.current_location();
        let unit_expr = self.parse_expr()?;
        let unit = match &unit_expr {
            Value::Keyword(k) => k.clone(),
            _ => {
                return Err(FernError::ParseError {
                    loc: unit_loc,
                    message:
                        "unit literal `#u` requires a keyword as the second argument (e.g. :mm)"
                            .to_string(),
                });
            }
        };

        // closing parenthesis
        self.expect_rparen("unit literal #u")?;

        // convert to mm
        let mm_value = convert_to_mm(value, &unit)
            .map_err(|msg| FernError::ParseError { loc, message: msg })?;

        Ok(Value::Length(mm_value))
    }

    /// Parse `#a(value :unit)`
    fn parse_angle_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let value_expr = self.parse_expr()?;
        let value = value_expr
            .as_number()
            .ok_or_else(|| FernError::ParseError {
                loc: loc.clone(),
                message: "angle literal `#a` requires a number as the first argument".to_string(),
            })?;

        let unit_loc = self.current_location();
        let unit_expr = self.parse_expr()?;
        let unit = match &unit_expr {
            Value::Keyword(k) => k.clone(),
            _ => {
                return Err(FernError::ParseError {
                    loc: unit_loc,
                    message:
                        "angle literal `#a` requires a keyword as the second argument (e.g. :deg)"
                            .to_string(),
                });
            }
        };

        // closing parenthesis
        self.expect_rparen("angle literal #a")?;

        // convert to rad
        let rad_value = convert_to_rad(value, &unit)
            .map_err(|msg| FernError::ParseError { loc, message: msg })?;

        Ok(Value::Angle(rad_value))
    }

    /// Parse `#v(x y z)`
    fn parse_vec3_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let x = self.parse_number_component("vector #v", "x", &loc)?;
        let y = self.parse_number_component("vector #v", "y", &loc)?;
        let z = self.parse_number_component("vector #v", "z", &loc)?;
        self.expect_rparen("vector literal #v")?;
        Ok(Value::Vec3([x, y, z]))
    }

    /// Parse `#p(x y z)`
    fn parse_point3_literal(&mut self) -> FernResult<Value> {
        let loc = self.current_location();
        let x = self.parse_number_component("point #p", "x", &loc)?;
        let y = self.parse_number_component("point #p", "y", &loc)?;
        let z = self.parse_number_component("point #p", "z", &loc)?;
        self.expect_rparen("point literal #p")?;
        Ok(Value::Point3([x, y, z]))
    }

    /// Helper to parse a numeric component
    fn parse_number_component(
        &mut self,
        context: &str,
        component: &str,
        loc: &SourceLocation,
    ) -> FernResult<f64> {
        let expr = self.parse_expr()?;
        expr.as_number().ok_or_else(|| FernError::ParseError {
            loc: loc.clone(),
            message: format!("{context} {component} component must be a number"),
        })
    }

    /// Expect and consume a closing parenthesis
    fn expect_rparen(&mut self, context: &str) -> FernResult<()> {
        let loc = self.current_location();
        match self.peek() {
            Some(st) if st.token == Token::RParen => {
                self.advance();
                Ok(())
            }
            _ => Err(FernError::ParseError {
                loc,
                message: format!("{context} requires a closing `)`"),
            }),
        }
    }
}

/// Convert length units to mm
fn convert_to_mm(value: f64, unit: &str) -> Result<f64, String> {
    /// 1 inch = 25.4 mm
    const INCH_TO_MM: f64 = 25.4;
    /// 1 foot = 304.8 mm
    const FOOT_TO_MM: f64 = 304.8;

    match unit {
        "mm" => Ok(value),
        "cm" => Ok(value * 10.0),
        "m" => Ok(value * 1000.0),
        "inch" => Ok(value * INCH_TO_MM),
        "ft" => Ok(value * FOOT_TO_MM),
        _ => Err(format!(
            "unsupported length unit `:{unit}`. Supported units: :mm, :cm, :m, :inch, :ft"
        )),
    }
}

/// Convert angle units to rad
fn convert_to_rad(value: f64, unit: &str) -> Result<f64, String> {
    match unit {
        "rad" => Ok(value),
        "deg" => Ok(value * std::f64::consts::PI / 180.0),
        "turn" => Ok(value * 2.0 * std::f64::consts::PI),
        _ => Err(format!(
            "unsupported angle unit `:{unit}`. Supported units: :rad, :deg, :turn"
        )),
    }
}

/// Parse source code into a list of S-expressions
///
/// # Errors
///
/// Returns `FernError` if the source contains syntax errors.
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

/// Parse source code into S-expressions with source spans
///
/// Returns each top-level expression paired with its byte range in the source.
///
/// # Errors
///
/// Returns `FernError` if the source contains syntax errors.
pub fn parse_with_spans(source: &str) -> FernResult<Vec<(Value, SourceSpan)>> {
    let tokens = tokenize(source)?;
    let mut parser = Parser::new(tokens, source);
    let mut exprs = Vec::new();

    while parser.peek().is_some() {
        let (expr, span) = parser.parse_expr_spanned()?;
        exprs.push((
            expr,
            SourceSpan {
                start: span.start,
                end: span.end,
            },
        ));
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
            other => panic!("expected List, got {other:?}"),
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
            other => panic!("expected List, got {other:?}"),
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
            other => panic!("expected Length, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_unit_inch_to_mm() {
        let result = parse("#u(1.0 :inch)").unwrap();
        match &result[0] {
            Value::Length(v) => assert!((v - 25.4).abs() < 1e-10),
            other => panic!("expected Length, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_angle_literal() {
        let result = parse("#a(90 :deg)").unwrap();
        match &result[0] {
            Value::Angle(v) => assert!((v - std::f64::consts::FRAC_PI_2).abs() < 1e-10),
            other => panic!("expected Angle, got {other:?}"),
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
            other => panic!("expected List, got {other:?}"),
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
            other => panic!("unexpected error: {other:?}"),
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
            other => panic!("expected List, got {other:?}"),
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
            other => panic!("expected List, got {other:?}"),
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
            "failed to parse MVP code: {:?}",
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
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_with_spans_atom() {
        let result = parse_with_spans("42").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, Value::Int(42));
        assert_eq!(result[0].1.start, 0);
        assert_eq!(result[0].1.end, 2);
    }

    #[test]
    fn test_parse_with_spans_list() {
        let source = "(+ 1 2)";
        let result = parse_with_spans(source).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1.start, 0);
        assert_eq!(result[0].1.end, 7);
    }

    #[test]
    fn test_parse_with_spans_multiple() {
        let source = "(defvar x 1) (box :width 10 :depth 10 :height 10)";
        let result = parse_with_spans(source).unwrap();
        assert_eq!(result.len(), 2);
        // First expression
        assert_eq!(result[0].1.start, 0);
        assert_eq!(result[0].1.end, 12);
        // Second expression
        assert_eq!(result[1].1.start, 13);
        assert_eq!(result[1].1.end, 49);
        // Verify the spans match the source text
        assert_eq!(&source[result[0].1.start..result[0].1.end], "(defvar x 1)");
        assert_eq!(
            &source[result[1].1.start..result[1].1.end],
            "(box :width 10 :depth 10 :height 10)"
        );
    }
}
