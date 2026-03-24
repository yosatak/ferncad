//! Module system
//!
//! Loads standard library (embedded) or files via `require`,
//! and controls symbol visibility via `export`.

use std::collections::HashMap;

/// Embedded standard library module definition
struct BuiltinModule {
    /// Module name
    name: &'static str,
    /// Source code
    source: &'static str,
}

/// Embedded standard library
const BUILTIN_MODULES: &[BuiltinModule] = &[BuiltinModule {
    name: "ferncad-std/m3-bolt",
    source: include_str!("../../../std/fasteners/m3-bolt.fern"),
}];

/// Module loader
#[derive(Debug, Default)]
pub struct ModuleLoader {
    /// Cache of loaded modules
    loaded: HashMap<String, bool>,
}

impl ModuleLoader {
    /// Create a new module loader
    pub fn new() -> Self {
        Self {
            loaded: HashMap::new(),
        }
    }

    /// Search for an embedded module source
    pub fn find_builtin(name: &str) -> Option<&'static str> {
        BUILTIN_MODULES
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.source)
    }

    /// Check whether a module has been loaded
    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded.contains_key(name)
    }

    /// Mark a module as loaded
    pub fn mark_loaded(&mut self, name: &str) {
        self.loaded.insert(name.to_string(), true);
    }

    /// Return a list of available embedded module names
    pub fn available_modules() -> Vec<&'static str> {
        BUILTIN_MODULES.iter().map(|m| m.name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_builtin_module() {
        let source = ModuleLoader::find_builtin("ferncad-std/m3-bolt");
        assert!(source.is_some());
        assert!(source.unwrap().contains("m3-bolt"));
    }

    #[test]
    fn test_unknown_module() {
        let source = ModuleLoader::find_builtin("nonexistent");
        assert!(source.is_none());
    }

    #[test]
    fn test_available_modules() {
        let modules = ModuleLoader::available_modules();
        assert!(modules.contains(&"ferncad-std/m3-bolt"));
    }

    #[test]
    fn test_load_tracking() {
        let mut loader = ModuleLoader::new();
        assert!(!loader.is_loaded("test"));
        loader.mark_loaded("test");
        assert!(loader.is_loaded("test"));
    }
}
