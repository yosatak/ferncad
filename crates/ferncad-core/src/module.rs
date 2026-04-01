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
const BUILTIN_MODULES: &[BuiltinModule] = &[
    BuiltinModule {
        name: "ferncad-std/m3-bolt",
        source: include_str!("../../../std/fasteners/m3-bolt.fern"),
    },
    BuiltinModule {
        name: "ferncad-std/m3-nut",
        source: include_str!("../../../std/fasteners/m3-nut.fern"),
    },
    BuiltinModule {
        name: "ferncad-std/m4-bolt",
        source: include_str!("../../../std/fasteners/m4-bolt.fern"),
    },
    BuiltinModule {
        name: "ferncad-std/m5-bolt",
        source: include_str!("../../../std/fasteners/m5-bolt.fern"),
    },
    BuiltinModule {
        name: "ferncad-std/flat-washer",
        source: include_str!("../../../std/fasteners/flat-washer.fern"),
    },
    BuiltinModule {
        name: "ferncad-std/spring-washer",
        source: include_str!("../../../std/fasteners/spring-washer.fern"),
    },
    BuiltinModule {
        name: "ferncad-std/spur-gear",
        source: include_str!("../../../std/gears/spur-gear.fern"),
    },
    BuiltinModule {
        name: "ferncad-std/bevel-gear",
        source: include_str!("../../../std/gears/bevel-gear.fern"),
    },
];

/// Module loader
#[derive(Debug, Default)]
pub struct ModuleLoader {
    /// Cache of loaded modules
    loaded: HashMap<String, bool>,
    /// User-provided module sources (from project files)
    user_modules: HashMap<String, String>,
}

impl ModuleLoader {
    /// Create a new module loader
    pub fn new() -> Self {
        Self {
            loaded: HashMap::new(),
            user_modules: HashMap::new(),
        }
    }

    /// Register a user-provided module (e.g. from a project file)
    pub fn add_user_module(&mut self, name: String, source: String) {
        self.user_modules.insert(name, source);
    }

    /// Search for a module source: user modules first, then builtins.
    ///
    /// Tries exact name, then with `.fern` suffix appended.
    /// Returns an owned `String` to avoid borrow conflicts during evaluation.
    pub fn find_module(&self, name: &str) -> Option<String> {
        // Try exact name in user modules
        if let Some(src) = self.user_modules.get(name) {
            return Some(src.clone());
        }
        // Try with .fern suffix
        let with_ext = format!("{name}.fern");
        if let Some(src) = self.user_modules.get(&with_ext) {
            return Some(src.clone());
        }
        // Fall back to builtins
        Self::find_builtin(name).map(|s| s.to_string())
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

    /// Return all available module names (builtins + user modules)
    pub fn available_modules_all(&self) -> Vec<String> {
        let mut mods: Vec<String> = BUILTIN_MODULES.iter().map(|m| m.name.to_string()).collect();
        for name in self.user_modules.keys() {
            mods.push(name.clone());
        }
        mods
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

    #[test]
    fn test_user_module_exact_name() {
        let mut loader = ModuleLoader::new();
        loader.add_user_module("utils".to_string(), "(defvar x 1)".to_string());
        let src = loader.find_module("utils");
        assert_eq!(src, Some("(defvar x 1)".to_string()));
    }

    #[test]
    fn test_user_module_with_fern_suffix() {
        let mut loader = ModuleLoader::new();
        loader.add_user_module("utils.fern".to_string(), "(defvar x 1)".to_string());
        let src = loader.find_module("utils");
        assert_eq!(src, Some("(defvar x 1)".to_string()));
    }

    #[test]
    fn test_user_module_overrides_builtin() {
        let mut loader = ModuleLoader::new();
        loader.add_user_module(
            "ferncad-std/m3-bolt".to_string(),
            "(defvar custom 1)".to_string(),
        );
        let src = loader.find_module("ferncad-std/m3-bolt").unwrap();
        assert_eq!(src, "(defvar custom 1)");
    }

    #[test]
    fn test_find_module_falls_back_to_builtin() {
        let loader = ModuleLoader::new();
        let src = loader.find_module("ferncad-std/m3-bolt");
        assert!(src.is_some());
        assert!(src.unwrap().contains("m3-bolt"));
    }

    #[test]
    fn test_available_modules_all() {
        let mut loader = ModuleLoader::new();
        loader.add_user_module("my-lib.fern".to_string(), "".to_string());
        let all = loader.available_modules_all();
        assert!(all.contains(&"ferncad-std/m3-bolt".to_string()));
        assert!(all.contains(&"my-lib.fern".to_string()));
    }

    #[test]
    fn test_find_spur_gear_module() {
        let source = ModuleLoader::find_builtin("ferncad-std/spur-gear");
        assert!(source.is_some());
        assert!(source.unwrap().contains("spur-gear"));
    }

    #[test]
    fn test_find_bevel_gear_module() {
        let source = ModuleLoader::find_builtin("ferncad-std/bevel-gear");
        assert!(source.is_some());
        assert!(source.unwrap().contains("bevel-gear"));
    }
}
