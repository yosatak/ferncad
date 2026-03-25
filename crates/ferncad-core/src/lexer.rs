//! ferncad 字句解析器（Lexer）
//!
//! `logos` クレートを使いトークン列を生成する。
//! Common Lisp インスパイアの構文に対応。

use logos::{Logos, Span};

use crate::error::{offset_to_location, FernError, FernResult, SourceLocation};

/// トークン型
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
#[logos(skip(r";[^\n]*", allow_greedy = true))]
pub enum Token {
    // === 括弧 ===
    /// 開き括弧 `(`
    #[token("(")]
    LParen,

    /// 閉じ括弧 `)`
    #[token(")")]
    RParen,

    // === 特殊プレフィックス ===
    /// 単位リテラルプレフィックス `#u`
    #[token("#u")]
    UnitPrefix,

    /// 角度リテラルプレフィックス `#a`
    #[token("#a")]
    AnglePrefix,

    /// ベクトルリテラルプレフィックス `#v`
    #[token("#v")]
    Vec3Prefix,

    /// 点リテラルプレフィックス `#p`
    #[token("#p")]
    Point3Prefix,

    // === 型アノテーション ===
    /// 型アノテーション演算子 `::`
    #[token("::")]
    TypeAnnotation,

    // === リテラル ===
    /// 浮動小数点数リテラル
    #[regex(r"-?[0-9]+\.[0-9]*([eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    /// 整数リテラル
    #[regex(r"-?[0-9]+", |lex| lex.slice().parse::<i64>().ok(), priority = 2)]
    Int(i64),

    /// 文字列リテラル
    #[regex(r#""([^"\\]|\\.)*""#, parse_string)]
    Str(String),

    // === キーワード・シンボル ===
    /// キーワード（`:` で始まる識別子）
    #[regex(r":[a-zA-Z][a-zA-Z0-9\-_/]*", |lex| lex.slice()[1..].to_string())]
    Keyword(String),

    /// シンボル（識別子）
    /// Common Lisp 慣習: `+const+`, `*var*`, `name-with-dash`, `predicate?`
    /// 比較演算子 `=`, `<`, `>`, `<=`, `>=` もシンボルとして扱う
    #[regex(r"[a-zA-Z_+*!?<>=][a-zA-Z0-9_+*\-/!?.<>=]*", |lex| lex.slice().to_string())]
    Symbol(String),

    // === 算術演算子（単独のシンボルとして） ===
    /// 単独の `-` （マイナス演算子）
    #[token("-", priority = 1)]
    Minus,

    /// 単独の `/` （除算演算子）
    #[token("/", priority = 1)]
    Slash,
}

/// 文字列リテラルをパースする（ダブルクォートを除去）
fn parse_string(lex: &logos::Lexer<Token>) -> Option<String> {
    let s = lex.slice();
    // 前後のダブルクォートを除去
    Some(s[1..s.len() - 1].to_string())
}

/// トークンとその位置情報
#[derive(Debug, Clone)]
pub struct SpannedToken {
    /// トークン
    pub token: Token,
    /// ソースコード上のバイト範囲
    pub span: Span,
}

/// ソースコードをトークン列に変換する
///
/// # Errors
///
/// 不正なトークンが含まれる場合、`FernError::LexError` を返す。
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

/// バイトオフセットからソース位置を取得するヘルパー
pub fn span_to_location(source: &str, span: &Span) -> SourceLocation {
    offset_to_location(source, span.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// トークン種別だけを抽出するヘルパー
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
        // '+' は2行目3列目
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
            _ => panic!("想定外のエラー型"),
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
        // MVP 完了基準のコードの一部がトークン化できることを確認
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
            "MVP コード例のトークン化に失敗: {:?}",
            result.err()
        );
    }
}
