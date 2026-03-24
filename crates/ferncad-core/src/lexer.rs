//! ferncad lexer
//!
//! Uses the `logos` crate to produce a token stream.
//! Supports Common Lisp-inspired syntax.

use logos::{Logos, Span};

use crate::error::{offset_to_location, FernError, FernResult, SourceLocation};

/// Token type
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
#[logos(skip(r";[^\n]*", allow_greedy = true))]
pub enum Token {
    // === Parentheses ===
    /// Open parenthesis `(`
    #[token("(")]
    LParen,

    /// Close parenthesis `)`
    #[token(")")]
    RParen,

    // === Special prefixes ===
    /// Unit literal prefix `#u`
    #[token("#u")]
    UnitPrefix,

    /// Angle literal prefix `#a`
    #[token("#a")]
    AnglePrefix,

    /// Vector literal prefix `#v`
    #[token("#v")]
    Vec3Prefix,

    /// Point literal prefix `#p`
    #[token("#p")]
    Point3Prefix,

    // === Type annotation ===
    /// Type annotation operator `::`
    #[token("::")]
    TypeAnnotation,

    // === Literals ===
    /// Floating-point literal
    #[regex(r"-?[0-9]+\.[0-9]*([eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    /// Integer literal
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok(), priority = 2)]
    Int(i64),

    /// String literal
    #[regex(r#""([^"\\]|\\.)*""#, parse_string)]
    Str(String),

    // === Keywords and symbols ===
    /// Keyword (identifier starting with `:`)
    #[regex(r":[a-zA-Z][a-zA-Z0-9\-_/]*", |lex| lex.slice()[1..].to_string())]
    Keyword(String),

    /// Symbol (identifier)
    /// Common Lisp conventions: `+const+`, `*var*`, `name-with-dash`, `predicate?`
    /// Comparison operators `=`, `<`, `>`, `<=`, `>=` are also treated as symbols
    #[regex(r"[a-zA-Z_+*!?<>=][a-zA-Z0-9_+*\-/!?.<>=]*", |lex| lex.slice().to_string())]
    Symbol(String),

    // === Arithmetic operators (as standalone symbols) ===
    /// Standalone `-` (minus operator)
    #[token("-", priority = 1)]
    Minus,

    /// Standalone `/` (division operator)
    #[token("/", priority = 1)]
    Slash,
}

/// Parse a string literal (strip surrounding double quotes)
fn parse_string(lex: &logos::Lexer<Token>) -> Option<String> {
    let s = lex.slice();
    // Strip surrounding double quotes
    Some(s[1..s.len() - 1].to_string())
}

/// Token with its location information
#[derive(Debug, Clone)]
pub struct SpannedToken {
    /// Token
    pub token: Token,
    /// Byte range in source code
    pub span: Span,
}

/// Convert source code to a token stream
///
/// # Errors
///
/// Returns `FernError::LexError` if the source contains invalid tokens.
pub fn tokenize(source: &str) -> FernResult<Vec<SpannedToken>> {
    let mut tokens = Vec::new();
    let mut lexer = Token::lexer(source);

    while let Some(result) = lexer.next() {
        let span = lexer.span();
        match result {
            Ok(token) => {
                tokens.push(SpannedToken { token, span });
            }
            Err(()) => {
                let loc = offset_to_location(source, span.start);
                let bad_token = &source[span.start..span.end];
                return Err(FernError::LexError {
                    loc,
                    token: bad_token.to_string(),
                });
            }
        }
    }

    Ok(tokens)
}

/// Helper to get source location from a byte offset
pub fn span_to_location(source: &str, span: &Span) -> SourceLocation {
    offset_to_location(source, span.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to extract only token types
    fn token_types(source: &str) -> Vec<Token> {
        tokenize(source)
            .unwrap()
            .into_iter()
            .map(|st| st.token)
            .collect()
    }

    #[test]
    fn test_basic_expression() {
        let tokens = token_types("(+ 1 2)");
        assert_eq!(
            tokens,
            vec![
                Token::LParen,
                Token::Symbol("+".to_string()),
                Token::Int(1),
                Token::Int(2),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_float_literal() {
        let tokens = token_types("3.14 -0.5 1.0e10");
        assert_eq!(
            tokens,
            vec![Token::Float(3.14), Token::Float(-0.5), Token::Float(1.0e10),]
        );
    }

    #[test]
    fn test_string_literal() {
        let tokens = token_types(r#""hello world""#);
        assert_eq!(tokens, vec![Token::Str("hello world".to_string())]);
    }

    #[test]
    fn test_keyword() {
        let tokens = token_types(":radius :mm :top");
        assert_eq!(
            tokens,
            vec![
                Token::Keyword("radius".to_string()),
                Token::Keyword("mm".to_string()),
                Token::Keyword("top".to_string()),
            ]
        );
    }

    #[test]
    fn test_cl_convention_symbols() {
        let tokens = token_types("+m3-diameter+ *default-tolerance*");
        assert_eq!(
            tokens,
            vec![
                Token::Symbol("+m3-diameter+".to_string()),
                Token::Symbol("*default-tolerance*".to_string()),
            ]
        );
    }

    #[test]
    fn test_special_prefixes() {
        let tokens = token_types("#u #a #v #p");
        assert_eq!(
            tokens,
            vec![
                Token::UnitPrefix,
                Token::AnglePrefix,
                Token::Vec3Prefix,
                Token::Point3Prefix,
            ]
        );
    }

    #[test]
    fn test_type_annotation() {
        let tokens = token_types("(x :: length)");
        assert_eq!(
            tokens,
            vec![
                Token::LParen,
                Token::Symbol("x".to_string()),
                Token::TypeAnnotation,
                Token::Symbol("length".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_comment_skip() {
        let tokens = token_types("; this is a comment\n(+ 1 2)");
        assert_eq!(
            tokens,
            vec![
                Token::LParen,
                Token::Symbol("+".to_string()),
                Token::Int(1),
                Token::Int(2),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_t_nil_as_symbols() {
        let tokens = token_types("t nil");
        assert_eq!(
            tokens,
            vec![
                Token::Symbol("t".to_string()),
                Token::Symbol("nil".to_string()),
            ]
        );
    }

    #[test]
    fn test_negative_int() {
        let tokens = token_types("-42");
        assert_eq!(tokens, vec![Token::Int(-42)]);
    }

    #[test]
    fn test_minus_operator() {
        let tokens = token_types("(- 10 3)");
        assert_eq!(
            tokens,
            vec![
                Token::LParen,
                Token::Minus,
                Token::Int(10),
                Token::Int(3),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_slash_operator() {
        let tokens = token_types("(/ 10 2)");
        assert_eq!(
            tokens,
            vec![
                Token::LParen,
                Token::Slash,
                Token::Int(10),
                Token::Int(2),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_unit_literal_tokens() {
        let tokens = token_types("#u(10 :mm)");
        assert_eq!(
            tokens,
            vec![
                Token::UnitPrefix,
                Token::LParen,
                Token::Int(10),
                Token::Keyword("mm".to_string()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_location_tracking() {
        let source = "(\n  + 1 2)";
        let tokens = tokenize(source).unwrap();
        // '+' is at line 2, column 3
        let plus_loc = span_to_location(source, &tokens[1].span);
        assert_eq!(plus_loc.line, 2);
        assert_eq!(plus_loc.col, 3);
    }

    #[test]
    fn test_lex_error() {
        let result = tokenize("(+ 1 @)");
        assert!(result.is_err());
        match result.unwrap_err() {
            FernError::LexError { loc, token } => {
                assert_eq!(token, "@");
                assert_eq!(loc.line, 1);
                assert_eq!(loc.col, 6);
            }
            _ => panic!("unexpected error type"),
        }
    }

    #[test]
    fn test_defpart_tokens() {
        let source = r#"(defpart my-part "test" :meta (:category :test))"#;
        let tokens = token_types(source);
        assert_eq!(tokens[0], Token::LParen);
        assert_eq!(tokens[1], Token::Symbol("defpart".to_string()));
        assert_eq!(tokens[2], Token::Symbol("my-part".to_string()));
        assert_eq!(tokens[3], Token::Str("test".to_string()));
        assert_eq!(tokens[4], Token::Keyword("meta".to_string()));
    }

    #[test]
    fn test_complex_defpart() {
        // Verify that the MVP completion criteria code can be tokenized
        let source = r#"(defpart my-part
  "テスト用パーツ"
  :meta (:category :test :material :steel :description "test")
  :params ((size :: length :default 10.0 :doc "サイズ"))
  :body
  (difference
    (box :width size :depth size :height size)
    (sphere :radius (/ size 3))))"#;
        let result = tokenize(source);
        assert!(
            result.is_ok(),
            "failed to tokenize MVP code example: {:?}",
            result.err()
        );
    }
}
