//! ferncad エラー型定義
//!
//! すべてのエラーは日本語メッセージで、行番号・列番号を含む。

use thiserror::Error;

/// ソースコード上の位置情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    /// 行番号（1始まり）
    pub line: usize,
    /// 列番号（1始まり）
    pub col: usize,
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// ソースコード中のバイトオフセットから行番号・列番号を計算する
pub fn offset_to_location(source: &str, offset: usize) -> SourceLocation {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    SourceLocation { line, col }
}

/// ferncad の統合エラー型
#[derive(Error, Debug, Clone, PartialEq)]
pub enum FernError {
    /// 字句解析エラー
    #[error("{loc} 字句解析エラー: 不正なトークン `{token}`")]
    LexError { loc: SourceLocation, token: String },

    /// 構文エラー: 対応する閉じ括弧がない
    #[error("{loc} 構文エラー: 対応する `)` がありません")]
    UnmatchedOpenParen { loc: SourceLocation },

    /// 構文エラー: 余分な閉じ括弧
    #[error("{loc} 構文エラー: 対応する `(` がない `)` があります")]
    UnmatchedCloseParen { loc: SourceLocation },

    /// 構文エラー: 一般
    #[error("{loc} 構文エラー: {message}")]
    ParseError {
        loc: SourceLocation,
        message: String,
    },

    /// 評価エラー: 一般
    #[error("{loc} 評価エラー: {message}")]
    EvalError {
        loc: SourceLocation,
        message: String,
    },

    /// 評価エラー: 未定義の変数
    #[error("{loc} 評価エラー: 変数 `{name}` は定義されていません")]
    UndefinedVariable { loc: SourceLocation, name: String },

    /// 型エラー
    #[error("{loc} 型エラー: {expected}が必要ですが、{actual}が渡されました")]
    TypeError {
        loc: SourceLocation,
        expected: String,
        actual: String,
    },

    /// CAD エラー
    #[error("CADエラー: {message}")]
    CadError { message: String },
}

/// ferncad の Result 型エイリアス
pub type FernResult<T> = Result<T, FernError>;
