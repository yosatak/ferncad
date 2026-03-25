//! モジュールシステム
//!
//! `require` で標準ライブラリ（埋め込み）やファイルを読み込み、
//! `export` でシンボルの公開制御を行う。

use std::collections::HashMap;

/// 埋め込み標準ライブラリのモジュール定義
struct BuiltinModule {
    /// モジュール名
    name: &'static str,
    /// ソースコード
    source: &'static str,
}

/// 埋め込み標準ライブラリ
const BUILTIN_MODULES: &[BuiltinModule] = &[BuiltinModule {
    name: "ferncad-std/m3-bolt",
    source: include_str!("../../../std/fasteners/m3-bolt.fern"),
}];

/// モジュールローダー
#[derive(Debug, Default)]
pub struct ModuleLoader {
    /// 読み込み済みモジュールのキャッシュ
    loaded: HashMap<String, bool>,
}

impl ModuleLoader {
    /// 新しいモジュールローダーを作成する
    pub fn new() -> Self {
        Self {
            loaded: HashMap::new(),
        }
    }

    /// 埋め込みモジュールのソースを検索する
    pub fn find_builtin(name: &str) -> Option<&'static str> {
        BUILTIN_MODULES
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.source)
    }

    /// モジュールが読み込み済みか確認する
    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded.contains_key(name)
    }

    /// モジュールを読み込み済みとしてマークする
    pub fn mark_loaded(&mut self, name: &str) {
        self.loaded.insert(name.to_string(), true);
    }

    /// 利用可能な埋め込みモジュール名の一覧を返す
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
