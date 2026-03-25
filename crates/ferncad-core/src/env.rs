//! ferncad 環境（スコープ）管理
//!
//! レキシカルスコープを連鎖で管理する。
//! クロージャをサポートするため `Rc<RefCell<Env>>` を使用。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::{FernError, FernResult, SourceLocation};
use crate::types::Value;

/// 環境（スコープ）
#[derive(Debug)]
pub struct Env {
    /// この環境の ID（デバッグ・参照用）
    pub id: usize,
    /// 変数バインディング
    bindings: HashMap<String, Value>,
    /// 親環境（レキシカルスコープ）
    parent: Option<Rc<RefCell<Env>>>,
}

/// 環境 ID のグローバルカウンター
static ENV_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// 新しい環境 ID を生成する
fn next_env_id() -> usize {
    ENV_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Env {
    /// 新しいグローバル環境を作成する
    pub fn new_global() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            id: next_env_id(),
            bindings: HashMap::new(),
            parent: None,
        }))
    }

    /// 親環境を持つ子環境を作成する
    pub fn new_child(parent: Rc<RefCell<Env>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            id: next_env_id(),
            bindings: HashMap::new(),
            parent: Some(parent),
        }))
    }

    /// 変数を定義する（現在のスコープに追加）
    pub fn define(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }

    /// 変数を検索する（スコープチェーンを遡る）
    pub fn lookup(&self, name: &str, loc: &SourceLocation) -> FernResult<Value> {
        if let Some(value) = self.bindings.get(name) {
            return Ok(value.clone());
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().lookup(name, loc);
        }
        Err(FernError::UndefinedVariable {
            loc: loc.clone(),
            name: name.to_string(),
        })
    }

    /// 変数が定義されているか確認する
    pub fn is_defined(&self, name: &str) -> bool {
        if self.bindings.contains_key(name) {
            return true;
        }
        if let Some(parent) = &self.parent {
            return parent.borrow().is_defined(name);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_env() {
        let env = Env::new_global();
        env.borrow_mut().define("x".to_string(), Value::Int(42));
        let loc = SourceLocation { line: 1, col: 1 };
        assert_eq!(env.borrow().lookup("x", &loc).unwrap(), Value::Int(42));
    }

    #[test]
    fn test_child_env_inherits() {
        let parent = Env::new_global();
        parent.borrow_mut().define("x".to_string(), Value::Int(10));
        let child = Env::new_child(parent);
        child.borrow_mut().define("y".to_string(), Value::Int(20));
        let loc = SourceLocation { line: 1, col: 1 };
        assert_eq!(child.borrow().lookup("x", &loc).unwrap(), Value::Int(10));
        assert_eq!(child.borrow().lookup("y", &loc).unwrap(), Value::Int(20));
    }

    #[test]
    fn test_child_env_shadows() {
        let parent = Env::new_global();
        parent.borrow_mut().define("x".to_string(), Value::Int(10));
        let child = Env::new_child(parent);
        child.borrow_mut().define("x".to_string(), Value::Int(99));
        let loc = SourceLocation { line: 1, col: 1 };
        assert_eq!(child.borrow().lookup("x", &loc).unwrap(), Value::Int(99));
    }

    #[test]
    fn test_undefined_variable() {
        let env = Env::new_global();
        let loc = SourceLocation { line: 1, col: 1 };
        let err = env.borrow().lookup("unknown", &loc).unwrap_err();
        match err {
            FernError::UndefinedVariable { name, .. } => {
                assert_eq!(name, "unknown");
            }
            _ => panic!("想定外のエラー型"),
        }
    }
}
