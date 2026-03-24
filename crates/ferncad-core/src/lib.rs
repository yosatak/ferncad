//! ferncad core library
//!
//! Provides the Lexer, Parser, Evaluator, and type definitions.
//! A pure Rust library with no WASM dependencies.

pub mod assembly;
pub mod env;
pub mod error;
pub mod evaluator;
pub mod face;
pub mod lexer;
pub mod module;
pub mod parser;
pub mod types;
