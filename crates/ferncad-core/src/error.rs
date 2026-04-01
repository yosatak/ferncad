//! ferncad error type definitions
//!
//! All errors include line and column numbers.

use thiserror::Error;

/// Source location information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub col: usize,
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Byte-range span in source code (for source location tracking)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// Start byte offset (0-based, inclusive)
    pub start: usize,
    /// End byte offset (0-based, exclusive)
    pub end: usize,
}

impl SourceSpan {
    /// Create a dummy span (used when no real location is available)
    pub fn dummy() -> Self {
        Self { start: 0, end: 0 }
    }

    /// Convert to line/column location
    pub fn to_location(&self, source: &str) -> SourceLocation {
        offset_to_location(source, self.start)
    }
}

/// Compute line and column numbers from a byte offset in source code
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

/// Unified error type for ferncad
#[derive(Error, Debug, Clone, PartialEq)]
pub enum FernError {
    /// Lexer error
    #[error("{loc} lex error: invalid token `{token}`")]
    LexError { loc: SourceLocation, token: String },

    /// Parse error: unmatched open parenthesis
    #[error("{loc} parse error: missing closing `)`")]
    UnmatchedOpenParen { loc: SourceLocation },

    /// Parse error: unmatched close parenthesis
    #[error("{loc} parse error: unexpected `)` without matching `(`")]
    UnmatchedCloseParen { loc: SourceLocation },

    /// Parse error: general
    #[error("{loc} parse error: {message}")]
    ParseError {
        loc: SourceLocation,
        message: String,
    },

    /// Evaluation error: general
    #[error("{loc} eval error: {message}")]
    EvalError {
        loc: SourceLocation,
        message: String,
    },

    /// Evaluation error: undefined variable
    #[error("{loc} eval error: variable `{name}` is not defined")]
    UndefinedVariable { loc: SourceLocation, name: String },

    /// Type error
    #[error("{loc} type error: expected {expected}, but got {actual}")]
    TypeError {
        loc: SourceLocation,
        expected: String,
        actual: String,
    },

    /// CAD error
    #[error("CAD error: {message}")]
    CadError { message: String },
}

/// Result type alias for ferncad
pub type FernResult<T> = Result<T, FernError>;
