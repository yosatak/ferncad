//! ferncad 評価器（Evaluator）
//!
//! ツリーウォーク型インタープリタ。
//! S式（`Value`）を評価し、結果の `Value` を返す。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::env::Env;
use crate::error::{FernError, FernResult, SourceLocation};
use crate::types::{
    BuiltinFnDef, LambdaDef, ParamSpec, PartDef, ShapeNode, Value, DEFAULT_SEGMENTS,
};

/// 評価器
pub struct Evaluator {
    /// グローバル環境
    env: Rc<RefCell<Env>>,
    /// 環境マップ（ID → 環境参照、クロージャ用）
    env_map: HashMap<usize, Rc<RefCell<Env>>>,
}

impl Evaluator {
    /// 新しい評価器を作成し、組み込み関数を登録する
    pub fn new() -> Self {
        let env = Env::new_global();
        let mut evaluator = Self {
            env: Rc::clone(&env),
            env_map: HashMap::new(),
        };
        evaluator.register_env(Rc::clone(&env));
        evaluator.register_builtins();
        evaluator
    }

    /// 環境を環境マップに登録する
    fn register_env(&mut self, env: Rc<RefCell<Env>>) {
        let id = env.borrow().id;
        self.env_map.insert(id, env);
    }

    /// ソースコードを評価する
    pub fn eval_source(&mut self, source: &str) -> FernResult<Value> {
        let exprs = crate::parser::parse(source)?;
        let mut result = Value::Nil;
        for expr in &exprs {
            result = self.eval(expr, &Rc::clone(&self.env))?;
        }
        Ok(result)
    }

    /// 式を評価する
    pub fn eval(&mut self, expr: &Value, env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        match expr {
            Value::Int(_)
            | Value::Float(_)
            | Value::Str(_)
            | Value::Bool(_)
            | Value::Nil
            | Value::Length(_)
            | Value::Angle(_)
            | Value::Vec3(_)
            | Value::Point3(_)
            | Value::Keyword(_) => Ok(expr.clone()),

            Value::Symbol(name) => env.borrow().lookup(name, &loc),

            Value::List(items) => {
                if items.is_empty() {
                    return Ok(Value::Nil);
                }
                self.eval_list(items, env)
            }

            _ => Ok(expr.clone()),
        }
    }

    /// リスト（関数呼び出しまたはスペシャルフォーム）を評価する
    fn eval_list(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };

        // スペシャルフォームのチェック
        if let Value::Symbol(name) = &items[0] {
            match name.as_str() {
                "defvar" => return self.eval_defvar(items, env),
                "defun" => return self.eval_defun(items, env),
                "defpart" => return self.eval_defpart(items, env),
                "defmeta" => return self.eval_defmeta(items, env),
                "let*" => return self.eval_let_star(items, env),
                "if" => return self.eval_if(items, env),
                "lambda" => return self.eval_lambda(items, env),
                "quote" => return self.eval_quote(items),
                "progn" | "begin" => return self.eval_progn(items, env),
                "cond" => return self.eval_cond(items, env),
                _ => {}
            }
        }

        // 関数呼び出し
        let func = self.eval(&items[0], env)?;
        let args = &items[1..];

        match func {
            Value::BuiltinFn(def) => {
                // 組み込み関数: まず引数を評価
                let evaluated_args = self.eval_args(args, env)?;
                (def.func)(&evaluated_args, &loc)
            }
            Value::Lambda(lambda_def) => self.apply_lambda(&lambda_def, args, env),
            Value::PartDef(part_def) => self.apply_part(&part_def, args, env),
            _ => Err(FernError::EvalError {
                loc,
                message: format!(
                    "`{}` は関数として呼び出せません（型: {}）",
                    items[0],
                    func.type_name_ja()
                ),
            }),
        }
    }

    /// 引数リストを評価する
    fn eval_args(&mut self, args: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Vec<Value>> {
        args.iter().map(|arg| self.eval(arg, env)).collect()
    }

    // === スペシャルフォーム ===

    /// `(defvar name value)` を評価する
    fn eval_defvar(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 3 {
            return Err(FernError::EvalError {
                loc,
                message: "defvar は (defvar 名前 値) の形式が必要です".to_string(),
            });
        }
        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "defvar の第1引数はシンボルが必要です".to_string(),
                });
            }
        };
        let value = self.eval(&items[2], env)?;
        env.borrow_mut().define(name, value.clone());
        Ok(value)
    }

    /// `(defun name (params...) body...)` を評価する
    fn eval_defun(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 4 {
            return Err(FernError::EvalError {
                loc,
                message: "defun は (defun 名前 (引数...) 本体...) の形式が必要です".to_string(),
            });
        }

        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "defun の第1引数はシンボルが必要です".to_string(),
                });
            }
        };

        // docstring があるかチェック（第3引数が文字列の場合）
        let (params_idx, body_start) = if items.len() > 4 {
            if let Value::Str(_) = &items[3] {
                // (defun name (params) "docstring" body...)
                (2, 4)
            } else {
                (2, 3)
            }
        } else {
            (2, 3)
        };

        let params = self.extract_param_names(&items[params_idx])?;

        // :: 型アノテーション付きのパラメータをフィルタ
        let params = filter_type_annotations(&params);

        let body = items[body_start..].to_vec();
        let env_id = env.borrow().id;
        self.register_env(Rc::clone(env));

        let lambda = Value::Lambda(Arc::new(LambdaDef {
            params,
            body,
            env_id,
        }));
        env.borrow_mut().define(name, lambda.clone());
        Ok(lambda)
    }

    /// `(let* ((var1 val1) (var2 val2) ...) body...)` を評価する
    fn eval_let_star(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "let* は (let* ((変数 値)...) 本体...) の形式が必要です".to_string(),
            });
        }

        let bindings = match &items[1] {
            Value::List(b) => b,
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "let* の第1引数はバインディングリストが必要です".to_string(),
                });
            }
        };

        let child_env = Env::new_child(Rc::clone(env));
        self.register_env(Rc::clone(&child_env));

        for binding in bindings {
            match binding {
                Value::List(pair) if pair.len() >= 2 => {
                    let name = match &pair[0] {
                        Value::Symbol(s) => s.clone(),
                        _ => {
                            return Err(FernError::EvalError {
                                loc,
                                message: "let* のバインディングの変数名はシンボルが必要です"
                                    .to_string(),
                            });
                        }
                    };
                    // :: 型アノテーションをスキップ
                    let val_idx = find_value_index_after_annotation(pair);
                    let value = self.eval(&pair[val_idx], &child_env)?;
                    child_env.borrow_mut().define(name, value);
                }
                _ => {
                    return Err(FernError::EvalError {
                        loc,
                        message: "let* のバインディングは (変数 値) の形式が必要です".to_string(),
                    });
                }
            }
        }

        // body を逐次評価
        let mut result = Value::Nil;
        for expr in &items[2..] {
            result = self.eval(expr, &child_env)?;
        }
        Ok(result)
    }

    /// `(if test then else?)` を評価する
    fn eval_if(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "if は (if 条件 真の値 偽の値?) の形式が必要です".to_string(),
            });
        }

        let test = self.eval(&items[1], env)?;
        if test.is_truthy() {
            self.eval(&items[2], env)
        } else if items.len() > 3 {
            self.eval(&items[3], env)
        } else {
            Ok(Value::Nil)
        }
    }

    /// `(lambda (params...) body...)` を評価する
    fn eval_lambda(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "lambda は (lambda (引数...) 本体...) の形式が必要です".to_string(),
            });
        }

        let params = self.extract_param_names(&items[1])?;
        let params = filter_type_annotations(&params);
        let body = items[2..].to_vec();
        let env_id = env.borrow().id;
        self.register_env(Rc::clone(env));

        Ok(Value::Lambda(Arc::new(LambdaDef {
            params,
            body,
            env_id,
        })))
    }

    /// `(quote expr)` を評価する
    fn eval_quote(&mut self, items: &[Value]) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 2 {
            return Err(FernError::EvalError {
                loc,
                message: "quote は (quote 式) の形式が必要です".to_string(),
            });
        }
        Ok(items[1].clone())
    }

    /// `(progn body...)` を評価する
    fn eval_progn(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let mut result = Value::Nil;
        for expr in &items[1..] {
            result = self.eval(expr, env)?;
        }
        Ok(result)
    }

    /// `(cond (test1 expr1) (test2 expr2) ...)` を評価する
    fn eval_cond(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        for clause in &items[1..] {
            match clause {
                Value::List(pair) if pair.len() >= 2 => {
                    let test = self.eval(&pair[0], env)?;
                    if test.is_truthy() {
                        let mut result = Value::Nil;
                        for expr in &pair[1..] {
                            result = self.eval(expr, env)?;
                        }
                        return Ok(result);
                    }
                }
                _ => {
                    return Err(FernError::EvalError {
                        loc: SourceLocation { line: 0, col: 0 },
                        message: "cond の各節は (条件 式...) の形式が必要です".to_string(),
                    });
                }
            }
        }
        Ok(Value::Nil)
    }

    /// `(defpart name "doc" :meta (...) :params (...) :body expr)` を評価する
    fn eval_defpart(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 4 {
            return Err(FernError::EvalError {
                loc,
                message: "defpart は (defpart 名前 \"doc\" :meta ... :params ... :body ...) の形式が必要です".to_string(),
            });
        }

        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "defpart の第1引数はシンボルが必要です".to_string(),
                });
            }
        };

        let docstring = match &items[2] {
            Value::Str(s) => s.clone(),
            _ => String::new(),
        };

        // キーワード引数をパース
        let kw_start = if items.get(2).is_some_and(|v| matches!(v, Value::Str(_))) {
            3
        } else {
            2
        };

        let mut meta = HashMap::new();
        let mut params = Vec::new();
        let mut faces = Vec::new();
        let mut axes = Vec::new();
        let mut body = Vec::new();

        let mut i = kw_start;
        while i < items.len() {
            if let Value::Keyword(k) = &items[i] {
                match k.as_str() {
                    "meta" => {
                        i += 1;
                        if i < items.len() {
                            meta = self.parse_meta(&items[i])?;
                        }
                    }
                    "params" => {
                        i += 1;
                        if i < items.len() {
                            params = self.parse_param_specs(&items[i])?;
                        }
                    }
                    "faces" => {
                        i += 1;
                        if i < items.len() {
                            faces = self.parse_face_specs(&items[i])?;
                        }
                    }
                    "axes" => {
                        i += 1;
                        if i < items.len() {
                            axes = self.parse_axis_specs(&items[i])?;
                        }
                    }
                    "body" => {
                        body = items[i + 1..].to_vec();
                        break;
                    }
                    _ => {
                        i += 1;
                        continue;
                    }
                }
            }
            i += 1;
        }

        let env_id = env.borrow().id;
        self.register_env(Rc::clone(env));

        let part_def = Value::PartDef(Arc::new(PartDef {
            name: name.clone(),
            docstring,
            meta,
            faces,
            axes,
            params,
            body,
            env_id,
        }));

        env.borrow_mut().define(name, part_def.clone());
        Ok(part_def)
    }

    /// メタデータリストをパースする
    fn parse_meta(&self, expr: &Value) -> FernResult<HashMap<String, Value>> {
        let mut meta = HashMap::new();
        if let Value::List(items) = expr {
            let mut i = 0;
            while i < items.len() {
                if let Value::Keyword(k) = &items[i] {
                    if i + 1 < items.len() {
                        meta.insert(k.clone(), items[i + 1].clone());
                        i += 2;
                        continue;
                    }
                }
                i += 1;
            }
        }
        Ok(meta)
    }

    /// パラメータ仕様リストをパースする
    fn parse_param_specs(&self, expr: &Value) -> FernResult<Vec<ParamSpec>> {
        let loc = SourceLocation { line: 0, col: 0 };
        let specs_list = match expr {
            Value::List(items) => items,
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: ":params はリストが必要です".to_string(),
                });
            }
        };

        let mut specs = Vec::new();
        for spec_expr in specs_list {
            match spec_expr {
                Value::List(items) if !items.is_empty() => {
                    let name = match &items[0] {
                        Value::Symbol(s) => s.clone(),
                        _ => continue,
                    };

                    let mut type_annotation = None;
                    let mut default = None;
                    let mut doc = None;

                    let mut j = 1;
                    while j < items.len() {
                        match &items[j] {
                            Value::Symbol(s) if s == "::" => {
                                if j + 1 < items.len() {
                                    if let Value::Symbol(t) = &items[j + 1] {
                                        type_annotation = Some(t.clone());
                                    }
                                    j += 2;
                                    continue;
                                }
                            }
                            Value::Keyword(k) => match k.as_str() {
                                "default" => {
                                    if j + 1 < items.len() {
                                        default = Some(items[j + 1].clone());
                                        j += 2;
                                        continue;
                                    }
                                }
                                "doc" => {
                                    if j + 1 < items.len() {
                                        if let Value::Str(s) = &items[j + 1] {
                                            doc = Some(s.clone());
                                        }
                                        j += 2;
                                        continue;
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                        j += 1;
                    }

                    specs.push(ParamSpec {
                        name,
                        type_annotation,
                        default,
                        doc,
                    });
                }
                _ => {}
            }
        }
        Ok(specs)
    }

    /// `:faces` 仕様をパースする
    fn parse_face_specs(&self, expr: &Value) -> FernResult<Vec<crate::face::FaceSpec>> {
        let items = match expr {
            Value::List(items) => items,
            _ => return Ok(Vec::new()),
        };
        let mut specs = Vec::new();
        for item in items {
            if let Value::List(pair) = item {
                if let Some(Value::Keyword(name)) = pair.first() {
                    let doc = pair.get(1).and_then(|v| {
                        if let Value::Str(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                    specs.push(crate::face::FaceSpec {
                        name: name.clone(),
                        doc,
                        normal: None,
                    });
                }
            }
        }
        Ok(specs)
    }

    /// `:axes` 仕様をパースする
    fn parse_axis_specs(&self, expr: &Value) -> FernResult<Vec<crate::face::AxisSpec>> {
        let items = match expr {
            Value::List(items) => items,
            _ => return Ok(Vec::new()),
        };
        let mut specs = Vec::new();
        for item in items {
            if let Value::List(pair) = item {
                if let Some(Value::Keyword(name)) = pair.first() {
                    let doc = pair.get(1).and_then(|v| {
                        if let Value::Str(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    });
                    specs.push(crate::face::AxisSpec {
                        name: name.clone(),
                        doc,
                        direction: None,
                    });
                }
            }
        }
        Ok(specs)
    }

    /// `(defmeta :key val ...)` を評価する
    fn eval_defmeta(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let meta = self.parse_meta(&Value::List(items[1..].to_vec()))?;
        let meta_value = Value::List(
            meta.iter()
                .flat_map(|(k, v)| vec![Value::Keyword(k.clone()), v.clone()])
                .collect(),
        );
        env.borrow_mut()
            .define("*file-meta*".to_string(), meta_value.clone());
        Ok(meta_value)
    }

    /// Lambda をパラメータに適用する
    fn apply_lambda(
        &mut self,
        lambda: &LambdaDef,
        args: &[Value],
        call_env: &Rc<RefCell<Env>>,
    ) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };

        // クロージャの定義時環境を取得
        let closure_env = self
            .env_map
            .get(&lambda.env_id)
            .ok_or_else(|| FernError::EvalError {
                loc: loc.clone(),
                message: "関数のクロージャ環境が見つかりません".to_string(),
            })?
            .clone();

        let func_env = Env::new_child(closure_env);
        self.register_env(Rc::clone(&func_env));

        // 引数を評価してバインド
        let evaluated_args = self.eval_keyword_args(args, call_env)?;

        // 位置引数とキーワード引数を分離
        let (positional, kwargs) = split_kwargs(&evaluated_args);

        // 位置引数のバインド
        for (i, param) in lambda.params.iter().enumerate() {
            if let Some(value) = kwargs.get(param) {
                func_env.borrow_mut().define(param.clone(), value.clone());
            } else if let Some(value) = positional.get(i) {
                func_env.borrow_mut().define(param.clone(), value.clone());
            } else {
                return Err(FernError::EvalError {
                    loc,
                    message: format!("引数 `{param}` に値が渡されていません"),
                });
            }
        }

        // body を逐次評価
        let mut result = Value::Nil;
        for expr in &lambda.body {
            result = self.eval(expr, &func_env)?;
        }
        Ok(result)
    }

    /// PartDef をインスタンス化する
    fn apply_part(
        &mut self,
        part: &PartDef,
        args: &[Value],
        call_env: &Rc<RefCell<Env>>,
    ) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };

        // 定義時環境を取得
        let closure_env = self
            .env_map
            .get(&part.env_id)
            .ok_or_else(|| FernError::EvalError {
                loc: loc.clone(),
                message: "パーツのクロージャ環境が見つかりません".to_string(),
            })?
            .clone();

        let part_env = Env::new_child(closure_env);
        self.register_env(Rc::clone(&part_env));

        // キーワード引数を評価
        let evaluated_args = self.eval_keyword_args(args, call_env)?;
        let (_, kwargs) = split_kwargs(&evaluated_args);

        // パラメータをバインド（キーワード引数 or デフォルト値）
        for param in &part.params {
            let value = if let Some(v) = kwargs.get(&param.name) {
                v.clone()
            } else if let Some(default) = &param.default {
                default.clone()
            } else {
                return Err(FernError::EvalError {
                    loc: loc.clone(),
                    message: format!(
                        "パーツ `{}` のパラメータ `{}` に値が渡されていません（デフォルト値もありません）",
                        part.name, param.name
                    ),
                });
            };
            // 型アノテーションがある場合はチェック
            if let Some(ref type_name) = param.type_annotation {
                if !value.matches_type(type_name) {
                    return Err(FernError::TypeError {
                        loc: loc.clone(),
                        expected: type_name.clone(),
                        actual: value.type_name_ja().to_string(),
                    });
                }
            }
            part_env.borrow_mut().define(param.name.clone(), value);
        }

        // body を評価
        let mut result = Value::Nil;
        for expr in &part.body {
            result = self.eval(expr, &part_env)?;
        }
        Ok(result)
    }

    /// キーワード引数を含む引数リストを評価する
    fn eval_keyword_args(
        &mut self,
        args: &[Value],
        env: &Rc<RefCell<Env>>,
    ) -> FernResult<Vec<Value>> {
        let mut result = Vec::new();
        for arg in args {
            result.push(self.eval(arg, env)?);
        }
        Ok(result)
    }

    /// パラメータリストから名前を抽出する
    fn extract_param_names(&self, expr: &Value) -> FernResult<Vec<String>> {
        let loc = SourceLocation { line: 0, col: 0 };
        match expr {
            Value::List(items) => {
                let mut names = Vec::new();
                for item in items {
                    if let Value::Symbol(s) = item {
                        names.push(s.clone());
                    }
                }
                Ok(names)
            }
            _ => Err(FernError::EvalError {
                loc,
                message: "パラメータリストが必要です".to_string(),
            }),
        }
    }

    // === 組み込み関数登録 ===

    /// すべての組み込み関数を登録する
    fn register_builtins(&mut self) {
        // 算術
        self.register_builtin("+", builtin_add);
        self.register_builtin("-", builtin_sub);
        self.register_builtin("*", builtin_mul);
        self.register_builtin("/", builtin_div);

        // 比較
        self.register_builtin("=", builtin_eq);
        self.register_builtin("<", builtin_lt);
        self.register_builtin(">", builtin_gt);
        self.register_builtin("<=", builtin_le);
        self.register_builtin(">=", builtin_ge);

        // 論理
        self.register_builtin("not", builtin_not);

        // リスト
        self.register_builtin("list", builtin_list);

        // 数学
        self.register_builtin("cos", builtin_cos);
        self.register_builtin("sin", builtin_sin);
        self.register_builtin("sqrt", builtin_sqrt);

        // 定数
        self.env
            .borrow_mut()
            .define("pi".to_string(), Value::Float(std::f64::consts::PI));

        // デバッグ
        self.register_builtin("print", builtin_print);

        // CAD プリミティブ
        self.register_builtin("box", builtin_box);
        self.register_builtin("cube", builtin_cube);
        self.register_builtin("sphere", builtin_sphere);
        self.register_builtin("cylinder", builtin_cylinder);
        self.register_builtin("cone", builtin_cone);
        self.register_builtin("prism", builtin_prism);
        self.register_builtin("torus", builtin_torus);

        // CSG
        self.register_builtin("union", builtin_union);
        self.register_builtin("difference", builtin_difference);
        self.register_builtin("intersection", builtin_intersection);

        // 変換
        self.register_builtin("translate", builtin_translate);
        self.register_builtin("rotate", builtin_rotate);
        self.register_builtin("scale", builtin_scale);

        // 単位変換
        self.register_builtin("to-mm", builtin_to_mm);
        self.register_builtin("to-rad", builtin_to_rad);

        // 面・軸参照
        self.register_builtin("face", builtin_face);
        self.register_builtin("axis", builtin_axis);
    }

    /// 組み込み関数を登録する
    fn register_builtin(
        &self,
        name: &str,
        func: fn(&[Value], &SourceLocation) -> FernResult<Value>,
    ) {
        self.env.borrow_mut().define(
            name.to_string(),
            Value::BuiltinFn(BuiltinFnDef {
                name: name.to_string(),
                func,
            }),
        );
    }

    /// グローバル環境への参照を取得する
    pub fn global_env(&self) -> &Rc<RefCell<Env>> {
        &self.env
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

// === ヘルパー関数 ===

/// :: 型アノテーションをフィルタする
fn filter_type_annotations(params: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < params.len() {
        if params[i] == "::" {
            // :: の次の型名もスキップ
            i += 2;
        } else {
            result.push(params[i].clone());
            i += 1;
        }
    }
    result
}

/// let* バインディングで :: 型アノテーション後の値インデックスを見つける
fn find_value_index_after_annotation(pair: &[Value]) -> usize {
    // (name :: type value) → value は index 3
    // (name value) → value は index 1
    if pair.len() >= 4 {
        if let Value::Symbol(s) = &pair[1] {
            if s == "::" {
                return 3;
            }
        }
    }
    1
}

/// 評価済み引数からキーワード引数を分離する
fn split_kwargs(args: &[Value]) -> (Vec<Value>, HashMap<String, Value>) {
    let mut positional = Vec::new();
    let mut kwargs = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        if let Value::Keyword(k) = &args[i] {
            if i + 1 < args.len() {
                kwargs.insert(k.clone(), args[i + 1].clone());
                i += 2;
                continue;
            }
        }
        positional.push(args[i].clone());
        i += 1;
    }
    (positional, kwargs)
}

/// キーワード引数から値を取得するヘルパー
fn get_kwarg_f64(
    kwargs: &HashMap<String, Value>,
    key: &str,
    loc: &SourceLocation,
) -> FernResult<Option<f64>> {
    match kwargs.get(key) {
        Some(v) => match v.as_number() {
            Some(n) => Ok(Some(n)),
            None => Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "数値".to_string(),
                actual: v.type_name_ja().to_string(),
            }),
        },
        None => Ok(None),
    }
}

/// 必須キーワード引数の f64 値を取得する
fn require_kwarg_f64(
    kwargs: &HashMap<String, Value>,
    key: &str,
    func_name: &str,
    loc: &SourceLocation,
) -> FernResult<f64> {
    get_kwarg_f64(kwargs, key, loc)?.ok_or_else(|| FernError::EvalError {
        loc: loc.clone(),
        message: format!("`{func_name}` にはキーワード引数 `:{key}` が必要です"),
    })
}

/// 引数を Shape として取得する
fn get_shape_arg(value: &Value, loc: &SourceLocation) -> FernResult<Arc<ShapeNode>> {
    match value {
        Value::Shape(s) => Ok(Arc::clone(s)),
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "形状".to_string(),
            actual: value.type_name_ja().to_string(),
        }),
    }
}

// === 組み込み関数実装 ===

fn builtin_add(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let mut sum = 0.0_f64;
    for arg in args {
        sum += arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "数値".to_string(),
            actual: arg.type_name_ja().to_string(),
        })?;
    }
    Ok(Value::Float(sum))
}

fn builtin_sub(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`-` は少なくとも1つの引数が必要です".to_string(),
        });
    }
    let first = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    if args.len() == 1 {
        return Ok(Value::Float(-first));
    }
    let mut result = first;
    for arg in &args[1..] {
        result -= arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "数値".to_string(),
            actual: arg.type_name_ja().to_string(),
        })?;
    }
    Ok(Value::Float(result))
}

fn builtin_mul(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let mut product = 1.0_f64;
    for arg in args {
        product *= arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "数値".to_string(),
            actual: arg.type_name_ja().to_string(),
        })?;
    }
    Ok(Value::Float(product))
}

fn builtin_div(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() < 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`/` は少なくとも2つの引数が必要です".to_string(),
        });
    }
    let mut result = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    for arg in &args[1..] {
        let divisor = arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "数値".to_string(),
            actual: arg.type_name_ja().to_string(),
        })?;
        if divisor == 0.0 {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "ゼロによる除算です".to_string(),
            });
        }
        result /= divisor;
    }
    Ok(Value::Float(result))
}

fn builtin_eq(args: &[Value], _loc: &SourceLocation) -> FernResult<Value> {
    if args.len() < 2 {
        return Ok(Value::Bool(true));
    }
    for window in args.windows(2) {
        let a = window[0].as_number();
        let b = window[1].as_number();
        match (a, b) {
            (Some(a), Some(b)) if (a - b).abs() < f64::EPSILON => {}
            _ if window[0] == window[1] => {}
            _ => return Ok(Value::Bool(false)),
        }
    }
    Ok(Value::Bool(true))
}

fn builtin_lt(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    compare_chain(args, loc, |a, b| a < b)
}

fn builtin_gt(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    compare_chain(args, loc, |a, b| a > b)
}

fn builtin_le(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    compare_chain(args, loc, |a, b| a <= b)
}

fn builtin_ge(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    compare_chain(args, loc, |a, b| a >= b)
}

fn compare_chain(
    args: &[Value],
    loc: &SourceLocation,
    cmp: fn(f64, f64) -> bool,
) -> FernResult<Value> {
    if args.len() < 2 {
        return Ok(Value::Bool(true));
    }
    for window in args.windows(2) {
        let a = window[0].as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "数値".to_string(),
            actual: window[0].type_name_ja().to_string(),
        })?;
        let b = window[1].as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "数値".to_string(),
            actual: window[1].type_name_ja().to_string(),
        })?;
        if !cmp(a, b) {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

fn builtin_not(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`not` は1つの引数が必要です".to_string(),
        });
    }
    Ok(Value::Bool(!args[0].is_truthy()))
}

fn builtin_list(args: &[Value], _loc: &SourceLocation) -> FernResult<Value> {
    Ok(Value::List(args.to_vec()))
}

fn builtin_cos(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`cos` は1つの引数が必要です".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    Ok(Value::Float(v.cos()))
}

fn builtin_sin(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`sin` は1つの引数が必要です".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    Ok(Value::Float(v.sin()))
}

fn builtin_sqrt(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`sqrt` は1つの引数が必要です".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    Ok(Value::Float(v.sqrt()))
}

fn builtin_print(args: &[Value], _loc: &SourceLocation) -> FernResult<Value> {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{arg}");
    }
    println!();
    Ok(args.last().cloned().unwrap_or(Value::Nil))
}

// === CAD 組み込み関数 ===

fn builtin_box(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let width = require_kwarg_f64(&kwargs, "width", "box", loc)?;
    let depth = require_kwarg_f64(&kwargs, "depth", "box", loc)?;
    let height = require_kwarg_f64(&kwargs, "height", "box", loc)?;
    Ok(Value::Shape(Arc::new(ShapeNode::Box {
        width,
        depth,
        height,
    })))
}

fn builtin_cube(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`cube` は1つの数値引数が必要です（例: (cube 10.0)）".to_string(),
        });
    }
    let size = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    Ok(Value::Shape(Arc::new(ShapeNode::Box {
        width: size,
        depth: size,
        height: size,
    })))
}

fn builtin_sphere(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius = require_kwarg_f64(&kwargs, "radius", "sphere", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(ShapeNode::Sphere {
        radius,
        segments,
    })))
}

fn builtin_cylinder(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius = require_kwarg_f64(&kwargs, "radius", "cylinder", loc)?;
    let height = require_kwarg_f64(&kwargs, "height", "cylinder", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(ShapeNode::Cylinder {
        radius,
        height,
        segments,
    })))
}

fn builtin_cone(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius_bottom = require_kwarg_f64(&kwargs, "radius-bottom", "cone", loc)?;
    let radius_top = get_kwarg_f64(&kwargs, "radius-top", loc)?.unwrap_or(0.0);
    let height = require_kwarg_f64(&kwargs, "height", "cone", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(ShapeNode::Cone {
        radius_bottom,
        radius_top,
        height,
        segments,
    })))
}

fn builtin_prism(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let sides = require_kwarg_f64(&kwargs, "sides", "prism", loc)? as u32;
    let radius = require_kwarg_f64(&kwargs, "radius", "prism", loc)?;
    let height = require_kwarg_f64(&kwargs, "height", "prism", loc)?;
    Ok(Value::Shape(Arc::new(ShapeNode::Prism {
        sides,
        radius,
        height,
    })))
}

fn builtin_torus(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius_major = require_kwarg_f64(&kwargs, "radius-major", "torus", loc)?;
    let radius_minor = require_kwarg_f64(&kwargs, "radius-minor", "torus", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(ShapeNode::Torus {
        radius_major,
        radius_minor,
        segments,
    })))
}

fn builtin_union(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let children: Vec<_> = args
        .iter()
        .map(|a| get_shape_arg(a, loc))
        .collect::<FernResult<Vec<_>>>()?;
    Ok(Value::Shape(Arc::new(ShapeNode::Union { children })))
}

fn builtin_difference(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`difference` は少なくとも1つの引数が必要です".to_string(),
        });
    }
    let base = get_shape_arg(&args[0], loc)?;
    let cutters: Vec<_> = args[1..]
        .iter()
        .map(|a| get_shape_arg(a, loc))
        .collect::<FernResult<Vec<_>>>()?;
    Ok(Value::Shape(Arc::new(ShapeNode::Difference {
        base,
        cutters,
    })))
}

fn builtin_intersection(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let children: Vec<_> = args
        .iter()
        .map(|a| get_shape_arg(a, loc))
        .collect::<FernResult<Vec<_>>>()?;
    Ok(Value::Shape(Arc::new(ShapeNode::Intersection { children })))
}

fn builtin_translate(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);

    let shape = kwargs
        .get("shape")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`translate` にはキーワード引数 `:shape` が必要です".to_string(),
        })
        .and_then(|v| get_shape_arg(v, loc))?;

    let offset = kwargs
        .get("by")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`translate` にはキーワード引数 `:by` が必要です".to_string(),
        })
        .and_then(|v| match v {
            Value::Vec3(v) => Ok(*v),
            Value::List(items) if items.len() == 3 => {
                let x = items[0].as_number().unwrap_or(0.0);
                let y = items[1].as_number().unwrap_or(0.0);
                let z = items[2].as_number().unwrap_or(0.0);
                Ok([x, y, z])
            }
            _ => Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "ベクトル".to_string(),
                actual: v.type_name_ja().to_string(),
            }),
        })?;

    Ok(Value::Shape(Arc::new(ShapeNode::Translate {
        shape,
        offset,
    })))
}

fn builtin_rotate(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);

    let shape = kwargs
        .get("shape")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`rotate` にはキーワード引数 `:shape` が必要です".to_string(),
        })
        .and_then(|v| get_shape_arg(v, loc))?;

    let axis = match kwargs.get("axis") {
        Some(Value::Keyword(k)) => match k.as_str() {
            "x" => [1.0, 0.0, 0.0],
            "y" => [0.0, 1.0, 0.0],
            "z" => [0.0, 0.0, 1.0],
            _ => {
                return Err(FernError::EvalError {
                    loc: loc.clone(),
                    message: format!("軸 `:{k}` は未対応です。:x, :y, :z を使用してください"),
                });
            }
        },
        Some(Value::Vec3(v)) => *v,
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "`rotate` にはキーワード引数 `:axis` が必要です".to_string(),
            });
        }
    };

    let angle_rad = match kwargs.get("angle") {
        Some(Value::Angle(a)) => *a,
        Some(v) => v.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "角度".to_string(),
            actual: v.type_name_ja().to_string(),
        })?,
        None => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "`rotate` にはキーワード引数 `:angle` が必要です".to_string(),
            });
        }
    };

    Ok(Value::Shape(Arc::new(ShapeNode::Rotate {
        shape,
        axis,
        angle_rad,
    })))
}

fn builtin_scale(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);

    let shape = kwargs
        .get("shape")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`scale` にはキーワード引数 `:shape` が必要です".to_string(),
        })
        .and_then(|v| get_shape_arg(v, loc))?;

    let factors = if let Some(f) = kwargs.get("factor") {
        let v = f.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "数値".to_string(),
            actual: f.type_name_ja().to_string(),
        })?;
        [v, v, v]
    } else {
        let x = get_kwarg_f64(&kwargs, "x", loc)?.unwrap_or(1.0);
        let y = get_kwarg_f64(&kwargs, "y", loc)?.unwrap_or(1.0);
        let z = get_kwarg_f64(&kwargs, "z", loc)?.unwrap_or(1.0);
        [x, y, z]
    };

    Ok(Value::Shape(Arc::new(ShapeNode::Scale { shape, factors })))
}

fn builtin_to_mm(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`to-mm` は1つの長さ引数が必要です".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    Ok(Value::Float(v))
}

fn builtin_to_rad(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`to-rad` は1つの角度引数が必要です".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "数値".to_string(),
        actual: args[0].type_name_ja().to_string(),
    })?;
    Ok(Value::Float(v))
}

/// `(face instance-keyword :face-name)` — 面への参照を返す
fn builtin_face(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`face` は (face :インスタンス名 :面名) の形式が必要です".to_string(),
        });
    }
    let instance_name = match &args[0] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "キーワード".to_string(),
                actual: args[0].type_name_ja().to_string(),
            });
        }
    };
    let face_name = match &args[1] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "キーワード".to_string(),
                actual: args[1].type_name_ja().to_string(),
            });
        }
    };
    Ok(Value::FaceRef(crate::face::FaceRef {
        instance_name,
        face_name,
    }))
}

/// `(axis instance-keyword :axis-name)` — 軸への参照を返す
fn builtin_axis(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`axis` は (axis :インスタンス名 :軸名) の形式が必要です".to_string(),
        });
    }
    let instance_name = match &args[0] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "キーワード".to_string(),
                actual: args[0].type_name_ja().to_string(),
            });
        }
    };
    let axis_name = match &args[1] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "キーワード".to_string(),
                actual: args[1].type_name_ja().to_string(),
            });
        }
    };
    Ok(Value::AxisRef(crate::face::AxisRef {
        instance_name,
        axis_name,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(source: &str) -> Value {
        let mut evaluator = Evaluator::new();
        evaluator.eval_source(source).unwrap()
    }

    fn eval_err(source: &str) -> FernError {
        let mut evaluator = Evaluator::new();
        evaluator.eval_source(source).unwrap_err()
    }

    #[test]
    fn test_arithmetic_add() {
        assert_eq!(eval("(+ 1 2 3)"), Value::Float(6.0));
    }

    #[test]
    fn test_arithmetic_sub() {
        assert_eq!(eval("(- 10 3)"), Value::Float(7.0));
    }

    #[test]
    fn test_arithmetic_negate() {
        assert_eq!(eval("(- 5)"), Value::Float(-5.0));
    }

    #[test]
    fn test_arithmetic_mul() {
        assert_eq!(eval("(* 2 3 4)"), Value::Float(24.0));
    }

    #[test]
    fn test_arithmetic_div() {
        assert_eq!(eval("(/ 10 2)"), Value::Float(5.0));
    }

    #[test]
    fn test_division_by_zero() {
        let err = eval_err("(/ 10 0)");
        match err {
            FernError::EvalError { message, .. } => {
                assert!(message.contains("ゼロ"));
            }
            other => panic!("想定外のエラー型: {other:?}"),
        }
    }

    #[test]
    fn test_comparison() {
        assert_eq!(eval("(> 3 2)"), Value::Bool(true));
        assert_eq!(eval("(< 3 2)"), Value::Bool(false));
        assert_eq!(eval("(= 5 5)"), Value::Bool(true));
        assert_eq!(eval("(<= 3 3)"), Value::Bool(true));
        assert_eq!(eval("(>= 4 3)"), Value::Bool(true));
    }

    #[test]
    fn test_defvar() {
        assert_eq!(eval("(defvar x 42) x"), Value::Float(42.0));
    }

    #[test]
    fn test_let_star() {
        assert_eq!(eval("(let* ((a 1) (b (+ a 1))) b)"), Value::Float(2.0));
    }

    #[test]
    fn test_defun() {
        assert_eq!(eval("(defun sq (x) (* x x)) (sq 5)"), Value::Float(25.0));
    }

    #[test]
    fn test_if_true() {
        assert_eq!(eval("(if (> 3 2) 1 0)"), Value::Float(1.0));
    }

    #[test]
    fn test_if_false() {
        assert_eq!(eval("(if (< 3 2) 1 0)"), Value::Float(0.0));
    }

    #[test]
    fn test_if_nil_is_falsy() {
        assert_eq!(eval("(if nil 1 0)"), Value::Float(0.0));
    }

    #[test]
    fn test_lambda() {
        assert_eq!(
            eval("(let* ((f (lambda (x) (* x 2)))) (f 5))"),
            Value::Float(10.0)
        );
    }

    #[test]
    fn test_not() {
        assert_eq!(eval("(not t)"), Value::Bool(false));
        assert_eq!(eval("(not nil)"), Value::Bool(true));
    }

    #[test]
    fn test_nested_arithmetic() {
        assert_eq!(eval("(+ (* 2 3) (- 10 5))"), Value::Float(11.0));
    }

    #[test]
    fn test_math_functions() {
        let result = eval("(cos 0)");
        match result {
            Value::Float(v) => assert!((v - 1.0).abs() < 1e-10),
            other => panic!("期待: Float、実際: {other:?}"),
        }
    }

    #[test]
    fn test_pi() {
        let result = eval("pi");
        match result {
            Value::Float(v) => assert!((v - std::f64::consts::PI).abs() < 1e-10),
            other => panic!("期待: Float、実際: {other:?}"),
        }
    }

    #[test]
    fn test_undefined_variable() {
        let err = eval_err("unknown");
        assert!(matches!(err, FernError::UndefinedVariable { .. }));
    }

    #[test]
    fn test_box_shape() {
        let result = eval("(box :width 10 :depth 20 :height 30)");
        match result {
            Value::Shape(node) => {
                assert_eq!(
                    *node,
                    ShapeNode::Box {
                        width: 10.0,
                        depth: 20.0,
                        height: 30.0,
                    }
                );
            }
            other => panic!("期待: Shape、実際: {other:?}"),
        }
    }

    #[test]
    fn test_sphere_shape() {
        let result = eval("(sphere :radius 5.0)");
        match result {
            Value::Shape(node) => match &*node {
                ShapeNode::Sphere { radius, .. } => {
                    assert!((radius - 5.0).abs() < 1e-10);
                }
                other => panic!("期待: Sphere、実際: {other:?}"),
            },
            other => panic!("期待: Shape、実際: {other:?}"),
        }
    }

    #[test]
    fn test_union() {
        let result = eval("(union (box :width 10 :depth 10 :height 10) (sphere :radius 5))");
        assert!(matches!(result, Value::Shape(_)));
    }

    #[test]
    fn test_difference() {
        let result = eval("(difference (box :width 10 :depth 10 :height 10) (sphere :radius 5))");
        assert!(matches!(result, Value::Shape(_)));
    }

    #[test]
    fn test_translate() {
        let result = eval("(translate :shape (box :width 10 :depth 10 :height 10) :by #v(5 0 0))");
        match result {
            Value::Shape(node) => match &*node {
                ShapeNode::Translate { offset, .. } => {
                    assert_eq!(*offset, [5.0, 0.0, 0.0]);
                }
                other => panic!("期待: Translate、実際: {other:?}"),
            },
            other => panic!("期待: Shape、実際: {other:?}"),
        }
    }

    #[test]
    fn test_defvar_and_shapes() {
        let result = eval(
            r#"
            (defvar +size+ 10.0)
            (box :width +size+ :depth +size+ :height +size+)
            "#,
        );
        match result {
            Value::Shape(node) => {
                assert_eq!(
                    *node,
                    ShapeNode::Box {
                        width: 10.0,
                        depth: 10.0,
                        height: 10.0,
                    }
                );
            }
            other => panic!("期待: Shape、実際: {other:?}"),
        }
    }

    #[test]
    fn test_defpart_basic() {
        let result = eval(
            r#"
            (defpart my-part
              "テスト用パーツ"
              :meta (:category :test :material :steel :description "test")
              :params ((size :: length :default 10.0 :doc "サイズ"))
              :body
              (box :width size :depth size :height size))

            (my-part :size 20.0)
            "#,
        );
        match result {
            Value::Shape(node) => {
                assert_eq!(
                    *node,
                    ShapeNode::Box {
                        width: 20.0,
                        depth: 20.0,
                        height: 20.0,
                    }
                );
            }
            other => panic!("期待: Shape、実際: {other:?}"),
        }
    }

    #[test]
    fn test_defpart_default_value() {
        let result = eval(
            r#"
            (defpart my-part
              "テスト"
              :params ((size :: length :default 10.0 :doc "s"))
              :body
              (box :width size :depth size :height size))

            (my-part)
            "#,
        );
        match result {
            Value::Shape(node) => {
                assert_eq!(
                    *node,
                    ShapeNode::Box {
                        width: 10.0,
                        depth: 10.0,
                        height: 10.0,
                    }
                );
            }
            other => panic!("期待: Shape、実際: {other:?}"),
        }
    }

    #[test]
    fn test_mvp_completion_criteria() {
        // Phase 1 MVP 完了基準のコード
        let result = eval(
            r#"
            (defpart my-part
              "テスト用パーツ"
              :meta (:category :test :material :steel :description "test")
              :params ((size :: length :default 10.0 :doc "サイズ"))
              :body
              (difference
                (box :width size :depth size :height size)
                (sphere :radius (/ size 3))))

            (my-part :size 20.0)
            "#,
        );
        match result {
            Value::Shape(node) => match &*node {
                ShapeNode::Difference { base, cutters } => {
                    assert_eq!(
                        **base,
                        ShapeNode::Box {
                            width: 20.0,
                            depth: 20.0,
                            height: 20.0,
                        }
                    );
                    assert_eq!(cutters.len(), 1);
                }
                other => panic!("期待: Difference、実際: {other:?}"),
            },
            other => panic!("期待: Shape、実際: {other:?}"),
        }
    }

    #[test]
    fn test_cond() {
        assert_eq!(
            eval("(cond ((> 1 2) 10) ((> 3 2) 20) (t 30))"),
            Value::Float(20.0)
        );
    }

    #[test]
    fn test_quote() {
        let result = eval("(quote (+ 1 2))");
        assert!(matches!(result, Value::List(_)));
    }

    #[test]
    fn test_defun_with_closure() {
        assert_eq!(
            eval(
                r#"
                (defvar x 10)
                (defun add-x (y) (+ x y))
                (add-x 5)
                "#
            ),
            Value::Float(15.0)
        );
    }

    #[test]
    fn test_type_check_defpart_accepts_valid() {
        // length 型に数値を渡す → OK
        let result = eval(
            r#"
            (defpart p "t"
              :params ((r :: length :default 5 :doc "r"))
              :body (sphere :radius r))
            (p :r 10)
            "#,
        );
        assert!(matches!(result, Value::Shape(_)));
    }

    #[test]
    fn test_type_check_defpart_rejects_invalid() {
        // length 型に文字列を渡す → 型エラー
        let err = eval_err(
            r#"
            (defpart p "t"
              :params ((r :: length :default 5 :doc "r"))
              :body (sphere :radius r))
            (p :r "not-a-number")
            "#,
        );
        assert!(matches!(err, FernError::TypeError { .. }));
    }

    #[test]
    fn test_type_check_keyword_param() {
        // keyword 型にキーワードを渡す → OK
        let result = eval(
            r#"
            (defpart p "t"
              :params ((finish :: keyword :default :none :doc "f"))
              :body (box :width 10 :depth 10 :height 10))
            (p :finish :zinc)
            "#,
        );
        assert!(matches!(result, Value::Shape(_)));
    }

    #[test]
    fn test_defmeta() {
        let result = eval(
            r#"
            (defmeta :title "Test" :version "1.0.0" :author "test")
            *file-meta*
            "#,
        );
        assert!(matches!(result, Value::List(_)));
    }

    #[test]
    fn test_defpart_with_faces_axes() {
        let result = eval(
            r#"
            (defpart bolt "ボルト"
              :faces ((:head-top "頭部上面") (:head-bottom "着座面"))
              :axes ((:center "中心軸"))
              :params ((r :: length :default 1.5 :doc "r"))
              :body (cylinder :radius r :height 10))
            (bolt)
            "#,
        );
        assert!(matches!(result, Value::Shape(_)));
    }

    #[test]
    fn test_face_ref() {
        let result = eval("(face :bolt :head-top)");
        match result {
            Value::FaceRef(r) => {
                assert_eq!(r.instance_name, "bolt");
                assert_eq!(r.face_name, "head-top");
            }
            other => panic!("期待: FaceRef、実際: {other:?}"),
        }
    }

    #[test]
    fn test_axis_ref() {
        let result = eval("(axis :bolt :center)");
        match result {
            Value::AxisRef(r) => {
                assert_eq!(r.instance_name, "bolt");
                assert_eq!(r.axis_name, "center");
            }
            other => panic!("期待: AxisRef、実際: {other:?}"),
        }
    }
}
