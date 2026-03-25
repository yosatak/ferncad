//! ferncad environment (scope) management
//!
//! Manages lexical scopes using a chain structure.
//! Uses `Rc<RefCell<Env>>` to support closures.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::{FernError, FernResult, SourceLocation};
use crate::types::Value;

/// Environment (scope)
#[derive(Debug)]
pub struct Env {
    /// ID of this environment (for debugging and referencing)
    pub id: usize,
    /// Variable bindings
    bindings: HashMap<String, Value>,
    /// Parent environment (lexical scope)
    parent: Option<Rc<RefCell<Env>>>,
}

/// Global counter for environment IDs
static ENV_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Generate a new environment ID
fn next_env_id() -> usize {
    ENV_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Env {
    /// Create a new global environment
    pub fn new_global() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            id: next_env_id(),
            bindings: HashMap::new(),
            parent: None,
        }))
    }

    /// Create a child environment with a parent
    pub fn new_child(parent: Rc<RefCell<Env>>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            id: next_env_id(),
            bindings: HashMap::new(),
            parent: Some(parent),
        }))
    }

    /// Define a variable (add to the current scope)
    pub fn define(&mut self, name: String, value: Value) {
        self.bindings.insert(name, value);
    }

    /// Look up a variable (traversing the scope chain)
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

    /// Check whether a variable is defined
    /// Iterate over bindings in this environment (not including parents)
    pub fn bindings(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.bindings.iter()
    }

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
            _ => panic!("unexpected error type"),
        }
    }
}
