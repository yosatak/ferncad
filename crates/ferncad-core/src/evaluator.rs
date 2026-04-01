//! ferncad evaluator
//!
//! A tree-walk interpreter.
//! Evaluates S-expressions (`Value`) and returns the resulting `Value`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::env::Env;
use crate::error::{FernError, FernResult, SourceLocation, SourceSpan};
use crate::types::{
    BuiltinFnDef, LambdaDef, MacroDef, ParamSpec, PartDef, PathNode, ShapeNode, TrackedShape,
    Value, DEFAULT_SEGMENTS,
};

/// Evaluator
pub struct Evaluator {
    /// Global environment
    env: Rc<RefCell<Env>>,
    /// Environment map (ID -> environment reference, for closures)
    env_map: HashMap<usize, Rc<RefCell<Env>>>,
    /// Module loader
    module_loader: crate::module::ModuleLoader,
    /// Current call span (byte range of the expression being evaluated)
    current_call_span: SourceSpan,
    /// Source code (for computing inner expression spans)
    source: String,
}

impl Evaluator {
    /// Create a new evaluator and register built-in functions
    pub fn new() -> Self {
        let env = Env::new_global();
        let mut evaluator = Self {
            env: Rc::clone(&env),
            env_map: HashMap::new(),
            module_loader: crate::module::ModuleLoader::new(),
            current_call_span: SourceSpan::dummy(),
            source: String::new(),
        };
        evaluator.register_env(Rc::clone(&env));
        evaluator.register_builtins();
        evaluator
    }

    /// Register an environment in the environment map
    fn register_env(&mut self, env: Rc<RefCell<Env>>) {
        let id = env.borrow().id;
        self.env_map.insert(id, env);
    }

    /// Get the current `*resolution*` value (global segment count)
    pub fn resolution(&self) -> u32 {
        let dummy_loc = SourceLocation { line: 0, col: 0 };
        self.env
            .borrow()
            .lookup("*resolution*", &dummy_loc)
            .ok()
            .and_then(|v| v.as_number())
            .map(|n| n as u32)
            .unwrap_or(DEFAULT_SEGMENTS)
    }

    /// Evaluate source code
    pub fn eval_source(&mut self, source: &str) -> FernResult<Value> {
        self.source = source.to_string();
        let spanned_exprs = crate::parser::parse_with_spans(source)?;
        let mut result = Value::Nil;
        for (expr, span) in &spanned_exprs {
            self.current_call_span = span.clone();
            result = self.eval(expr, &Rc::clone(&self.env))?;
        }
        Ok(result)
    }

    /// Evaluate an expression
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

    /// Evaluate a list (function call or special form)
    fn eval_list(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };

        // Check for special forms
        if let Value::Symbol(name) = &items[0] {
            match name.as_str() {
                "defvar" => return self.eval_defvar(items, env),
                "defun" => return self.eval_defun(items, env),
                "defpart" => return self.eval_defpart(items, env),
                "defmeta" => return self.eval_defmeta(items, env),
                "assembly" => return self.eval_assembly(items, env),
                "require" => return self.eval_require(items, env),
                "export" => return Ok(Value::Nil), // export is currently metadata-only
                "let*" => return self.eval_let_star(items, env),
                "if" => return self.eval_if(items, env),
                "lambda" => return self.eval_lambda(items, env),
                "quote" => return self.eval_quote(items),
                "quasiquote" => return self.eval_quasiquote(items, env),
                "defmacro" => return self.eval_defmacro(items, env),
                "progn" | "begin" => return self.eval_progn(items, env),
                "cond" => return self.eval_cond(items, env),
                "when" => return self.eval_when(items, env),
                "unless" => return self.eval_unless(items, env),
                "and" => return self.eval_and(items, env),
                "or" => return self.eval_or(items, env),
                "setf" => return self.eval_setf(items, env),
                "dotimes" => return self.eval_dotimes(items, env),
                "dolist" => return self.eval_dolist(items, env),
                "mapcar" => return self.eval_mapcar(items, env),
                "reduce" => return self.eval_reduce(items, env),
                "remove-if" => return self.eval_remove_if(items, env),
                "apply" => return self.eval_apply(items, env),
                _ => {}
            }
        }

        // Function call (evaluate head to get callable)
        let func = self.eval(&items[0], env)?;
        let args = &items[1..];

        match func {
            Value::BuiltinFn(def) => {
                // Built-in function: evaluate arguments first
                let evaluated_args = self.eval_args(args, env)?;
                let result = (def.func)(&evaluated_args, &loc)?;
                // Attach source span to shape results for editor↔viewer highlighting
                Ok(if let Value::Shape(tracked) = &result {
                    Value::Shape(Arc::new(TrackedShape {
                        node: Arc::clone(&tracked.node),
                        span: self.current_call_span.clone(),
                    }))
                } else {
                    result
                })
            }
            Value::Lambda(lambda_def) => self.apply_lambda(&lambda_def, args, env),
            Value::PartDef(part_def) => self.apply_part(&part_def, args, env),
            Value::Macro(macro_def) => self.apply_macro(&macro_def, args, env),
            _ => Err(FernError::EvalError {
                loc,
                message: format!(
                    "`{}` is not callable (type: {})",
                    items[0],
                    func.type_name()
                ),
            }),
        }
    }

    /// Evaluate an argument list
    fn eval_args(&mut self, args: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Vec<Value>> {
        args.iter().map(|arg| self.eval(arg, env)).collect()
    }

    // === Special Forms ===

    /// Evaluate `(defvar name value)`
    fn eval_defvar(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`defvar` requires (defvar name value) form".to_string(),
            });
        }
        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`defvar` first argument must be a symbol".to_string(),
                });
            }
        };
        let value = self.eval(&items[2], env)?;
        env.borrow_mut().define(name, value.clone());
        Ok(value)
    }

    /// Evaluate `(defun name (params...) body...)`
    fn eval_defun(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 4 {
            return Err(FernError::EvalError {
                loc,
                message: "`defun` requires (defun name (params...) body...) form".to_string(),
            });
        }

        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`defun` first argument must be a symbol".to_string(),
                });
            }
        };

        // Check for docstring (if third argument is a string)
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

        // Filter out :: type annotations from parameters
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

    /// Evaluate `(let* ((var1 val1) (var2 val2) ...) body...)`
    fn eval_let_star(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`let*` requires (let* ((var val)...) body...) form".to_string(),
            });
        }

        let bindings = match &items[1] {
            Value::List(b) => b,
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`let*` first argument must be a binding list".to_string(),
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
                                message: "`let*` binding variable name must be a symbol"
                                    .to_string(),
                            });
                        }
                    };
                    // Skip :: type annotation
                    let val_idx = find_value_index_after_annotation(pair);
                    let value = self.eval(&pair[val_idx], &child_env)?;
                    child_env.borrow_mut().define(name, value);
                }
                _ => {
                    return Err(FernError::EvalError {
                        loc,
                        message: "`let*` binding must be (variable value) form".to_string(),
                    });
                }
            }
        }

        // Evaluate body sequentially
        let mut result = Value::Nil;
        for expr in &items[2..] {
            result = self.eval(expr, &child_env)?;
        }
        Ok(result)
    }

    /// Evaluate `(if test then else?)`
    fn eval_if(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`if` requires (if condition then-expr else-expr?) form".to_string(),
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

    /// Evaluate `(lambda (params...) body...)`
    fn eval_lambda(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`lambda` requires (lambda (params...) body...) form".to_string(),
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

    /// Evaluate `(quote expr)`
    fn eval_quote(&mut self, items: &[Value]) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 2 {
            return Err(FernError::EvalError {
                loc,
                message: "`quote` requires (quote expr) form".to_string(),
            });
        }
        Ok(items[1].clone())
    }

    /// Evaluate `(defmacro name (params...) body...)`
    fn eval_defmacro(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 4 {
            return Err(FernError::EvalError {
                loc,
                message: "`defmacro` requires (defmacro name (params...) body...) form".to_string(),
            });
        }

        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`defmacro` first argument must be a symbol".to_string(),
                });
            }
        };

        // Skip optional docstring
        let (params_idx, body_start) = if items.len() > 4 {
            if let Value::Str(_) = &items[3] {
                (2, 4)
            } else {
                (2, 3)
            }
        } else {
            (2, 3)
        };

        let params = self.extract_param_names(&items[params_idx])?;
        let params = filter_type_annotations(&params);
        let body = items[body_start..].to_vec();
        let env_id = env.borrow().id;
        self.register_env(Rc::clone(env));

        let macro_val = Value::Macro(Arc::new(MacroDef {
            name: name.clone(),
            params,
            body,
            env_id,
        }));
        env.borrow_mut().define(name, macro_val.clone());
        Ok(macro_val)
    }

    /// Evaluate `(quasiquote template)` — recursive template expansion
    fn eval_quasiquote(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 2 {
            return Err(FernError::EvalError {
                loc,
                message: "`quasiquote` requires exactly one argument".to_string(),
            });
        }
        self.expand_quasiquote(&items[1], env)
    }

    /// Recursively expand a quasiquote template
    fn expand_quasiquote(&mut self, template: &Value, env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        match template {
            Value::List(items) if !items.is_empty() => {
                // Check for (unquote expr)
                if let Value::Symbol(s) = &items[0] {
                    if s == "unquote" {
                        if items.len() != 2 {
                            return Err(FernError::EvalError {
                                loc: SourceLocation { line: 0, col: 0 },
                                message: "`unquote` requires exactly one argument".to_string(),
                            });
                        }
                        return self.eval(&items[1], env);
                    }
                    // unquote-splicing at top level is an error
                    if s == "unquote-splicing" {
                        return Err(FernError::EvalError {
                            loc: SourceLocation { line: 0, col: 0 },
                            message: "`unquote-splicing` (,@) not valid outside a list context"
                                .to_string(),
                        });
                    }
                }

                // Process list elements, handling splicing
                let mut result = Vec::new();
                for item in items {
                    if let Value::List(inner) = item {
                        if !inner.is_empty() {
                            if let Value::Symbol(s) = &inner[0] {
                                if s == "unquote-splicing" {
                                    if inner.len() != 2 {
                                        return Err(FernError::EvalError {
                                            loc: SourceLocation { line: 0, col: 0 },
                                            message:
                                                "`unquote-splicing` requires exactly one argument"
                                                    .to_string(),
                                        });
                                    }
                                    let val = self.eval(&inner[1], env)?;
                                    if let Value::List(splice_items) = val {
                                        result.extend(splice_items);
                                    } else {
                                        return Err(FernError::EvalError {
                                            loc: SourceLocation { line: 0, col: 0 },
                                            message: format!(
                                                "`unquote-splicing` requires a list, got {}",
                                                val.type_name()
                                            ),
                                        });
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                    result.push(self.expand_quasiquote(item, env)?);
                }
                Ok(Value::List(result))
            }
            // Non-list values: return as-is (like quote)
            _ => Ok(template.clone()),
        }
    }

    /// Apply a macro: bind unevaluated args, evaluate body, then eval the expansion
    fn apply_macro(
        &mut self,
        macro_def: &MacroDef,
        args: &[Value],
        env: &Rc<RefCell<Env>>,
    ) -> FernResult<Value> {
        // Create macro expansion environment
        let parent_env = self
            .env_map
            .get(&macro_def.env_id)
            .cloned()
            .unwrap_or_else(|| Rc::clone(&self.env));
        let macro_env = Env::new_child(parent_env);
        self.register_env(Rc::clone(&macro_env));

        // Bind unevaluated arguments to macro parameters
        // Support &rest for variadic macros
        let mut rest_idx = None;
        for (i, param) in macro_def.params.iter().enumerate() {
            if param == "&rest" {
                rest_idx = Some(i);
                break;
            }
        }

        if let Some(ri) = rest_idx {
            // Bind positional params before &rest
            for (i, param) in macro_def.params[..ri].iter().enumerate() {
                let val = args.get(i).cloned().unwrap_or(Value::Nil);
                macro_env.borrow_mut().define(param.clone(), val);
            }
            // Bind &rest param to remaining args as a list
            if ri + 1 < macro_def.params.len() {
                let rest_name = &macro_def.params[ri + 1];
                let rest_vals = args[ri..].to_vec();
                macro_env
                    .borrow_mut()
                    .define(rest_name.clone(), Value::List(rest_vals));
            }
        } else {
            // Simple positional binding
            for (i, param) in macro_def.params.iter().enumerate() {
                let val = args.get(i).cloned().unwrap_or(Value::Nil);
                macro_env.borrow_mut().define(param.clone(), val);
            }
        }

        // Evaluate macro body to produce expansion
        let mut expansion = Value::Nil;
        for expr in &macro_def.body {
            expansion = self.eval(expr, &macro_env)?;
        }

        // Evaluate the expansion in the caller's environment
        self.eval(&expansion, env)
    }

    /// Evaluate `(progn body...)`
    fn eval_progn(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let mut result = Value::Nil;
        for expr in &items[1..] {
            result = self.eval(expr, env)?;
        }
        Ok(result)
    }

    /// Evaluate `(cond (test1 expr1) (test2 expr2) ...)`
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
                        message: "`cond` clause must be (condition expr...) form".to_string(),
                    });
                }
            }
        }
        Ok(Value::Nil)
    }

    /// Evaluate `(when condition body...)` — execute body when condition is truthy
    fn eval_when(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc: SourceLocation { line: 0, col: 0 },
                message: "`when` requires (when condition body...) form".to_string(),
            });
        }
        let test = self.eval(&items[1], env)?;
        if test.is_truthy() {
            let mut result = Value::Nil;
            for expr in &items[2..] {
                result = self.eval(expr, env)?;
            }
            Ok(result)
        } else {
            Ok(Value::Nil)
        }
    }

    /// Evaluate `(unless condition body...)` — execute body when condition is falsy
    fn eval_unless(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc: SourceLocation { line: 0, col: 0 },
                message: "`unless` requires (unless condition body...) form".to_string(),
            });
        }
        let test = self.eval(&items[1], env)?;
        if !test.is_truthy() {
            let mut result = Value::Nil;
            for expr in &items[2..] {
                result = self.eval(expr, env)?;
            }
            Ok(result)
        } else {
            Ok(Value::Nil)
        }
    }

    /// Evaluate `(and expr...)` — short-circuit; returns last truthy value or nil
    fn eval_and(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let mut result = Value::Bool(true);
        for expr in &items[1..] {
            result = self.eval(expr, env)?;
            if !result.is_truthy() {
                return Ok(Value::Nil);
            }
        }
        Ok(result)
    }

    /// Evaluate `(or expr...)` — short-circuit; returns first truthy value or nil
    fn eval_or(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        for expr in &items[1..] {
            let result = self.eval(expr, env)?;
            if result.is_truthy() {
                return Ok(result);
            }
        }
        Ok(Value::Nil)
    }

    /// Evaluate `(setf name value)` — mutate an existing variable
    fn eval_setf(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`setf` requires (setf name value) form".to_string(),
            });
        }
        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`setf` first argument must be a symbol".to_string(),
                });
            }
        };
        let value = self.eval(&items[2], env)?;
        if !env.borrow_mut().set(&name, value.clone()) {
            return Err(FernError::UndefinedVariable { loc, name });
        }
        Ok(value)
    }

    /// Evaluate `(dotimes (var count) body...)` — loop var from 0 to count-1
    fn eval_dotimes(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`dotimes` requires (dotimes (var count) body...) form".to_string(),
            });
        }
        let binding = match &items[1] {
            Value::List(b) if b.len() >= 2 => b,
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`dotimes` first argument must be (var count) form".to_string(),
                });
            }
        };
        let var_name = match &binding[0] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`dotimes` variable must be a symbol".to_string(),
                });
            }
        };

        let count_val = self.eval(&binding[1], env)?;
        let count = count_val.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: count_val.type_name().to_string(),
        })? as i64;

        let child_env = Env::new_child(Rc::clone(env));
        self.register_env(Rc::clone(&child_env));
        child_env
            .borrow_mut()
            .define(var_name.clone(), Value::Int(0));

        let mut result = Value::Nil;
        for i in 0..count {
            child_env.borrow_mut().set(&var_name, Value::Int(i));
            for expr in &items[2..] {
                result = self.eval(expr, &child_env)?;
            }
        }
        Ok(result)
    }

    /// Evaluate `(dolist (var list-expr) body...)` — iterate over a list
    fn eval_dolist(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`dolist` requires (dolist (var list) body...) form".to_string(),
            });
        }
        let binding = match &items[1] {
            Value::List(b) if b.len() >= 2 => b,
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`dolist` first argument must be (var list) form".to_string(),
                });
            }
        };
        let var_name = match &binding[0] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`dolist` variable must be a symbol".to_string(),
                });
            }
        };

        let list_val = self.eval(&binding[1], env)?;
        let items_list = match &list_val {
            Value::List(l) => l,
            _ => {
                return Err(FernError::TypeError {
                    loc,
                    expected: "list".to_string(),
                    actual: list_val.type_name().to_string(),
                });
            }
        };

        let child_env = Env::new_child(Rc::clone(env));
        self.register_env(Rc::clone(&child_env));
        child_env.borrow_mut().define(var_name.clone(), Value::Nil);

        let mut result = Value::Nil;
        for item in items_list {
            child_env.borrow_mut().set(&var_name, item.clone());
            for expr in &items[2..] {
                result = self.eval(expr, &child_env)?;
            }
        }
        Ok(result)
    }

    /// Evaluate `(mapcar fn list)` — apply function to each element, collect results
    fn eval_mapcar(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`mapcar` requires (mapcar function list) form".to_string(),
            });
        }
        let func = self.eval(&items[1], env)?;
        let list_val = self.eval(&items[2], env)?;
        let list = match &list_val {
            Value::List(l) => l,
            _ => {
                return Err(FernError::TypeError {
                    loc,
                    expected: "list".to_string(),
                    actual: list_val.type_name().to_string(),
                });
            }
        };

        let mut results = Vec::with_capacity(list.len());
        for item in list {
            let result = self.apply_callable(&func, &[item.clone()], env)?;
            results.push(result);
        }
        Ok(Value::List(results))
    }

    /// Evaluate `(reduce fn list initial)` — fold over list elements
    fn eval_reduce(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 3 || items.len() > 4 {
            return Err(FernError::EvalError {
                loc,
                message: "`reduce` requires (reduce function list [initial]) form".to_string(),
            });
        }
        let func = self.eval(&items[1], env)?;
        let list_val = self.eval(&items[2], env)?;
        let list = match &list_val {
            Value::List(l) => l,
            _ => {
                return Err(FernError::TypeError {
                    loc: loc.clone(),
                    expected: "list".to_string(),
                    actual: list_val.type_name().to_string(),
                });
            }
        };

        let (mut acc, start) = if items.len() == 4 {
            (self.eval(&items[3], env)?, 0)
        } else if !list.is_empty() {
            (list[0].clone(), 1)
        } else {
            return Err(FernError::EvalError {
                loc,
                message: "`reduce` requires a non-empty list or an initial value".to_string(),
            });
        };

        for item in &list[start..] {
            acc = self.apply_callable(&func, &[acc, item.clone()], env)?;
        }
        Ok(acc)
    }

    /// Evaluate `(remove-if fn list)` — remove elements where predicate returns truthy
    fn eval_remove_if(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`remove-if` requires (remove-if predicate list) form".to_string(),
            });
        }
        let func = self.eval(&items[1], env)?;
        let list_val = self.eval(&items[2], env)?;
        let list = match &list_val {
            Value::List(l) => l,
            _ => {
                return Err(FernError::TypeError {
                    loc,
                    expected: "list".to_string(),
                    actual: list_val.type_name().to_string(),
                });
            }
        };

        let mut results = Vec::new();
        for item in list {
            let test = self.apply_callable(&func, &[item.clone()], env)?;
            if !test.is_truthy() {
                results.push(item.clone());
            }
        }
        Ok(Value::List(results))
    }

    /// Evaluate `(apply fn args-list)` — apply function to a list of arguments
    fn eval_apply(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() != 3 {
            return Err(FernError::EvalError {
                loc,
                message: "`apply` requires (apply function args-list) form".to_string(),
            });
        }
        let func = self.eval(&items[1], env)?;
        let args_val = self.eval(&items[2], env)?;
        let args = match &args_val {
            Value::List(l) => l.clone(),
            _ => {
                return Err(FernError::TypeError {
                    loc,
                    expected: "list".to_string(),
                    actual: args_val.type_name().to_string(),
                });
            }
        };
        self.apply_callable(&func, &args, env)
    }

    /// Apply a callable value (Lambda, BuiltinFn, or PartDef) to pre-evaluated arguments
    fn apply_callable(
        &mut self,
        func: &Value,
        args: &[Value],
        _env: &Rc<RefCell<Env>>,
    ) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        match func {
            Value::BuiltinFn(def) => (def.func)(args, &loc),
            Value::Lambda(lambda_def) => {
                let closure_env = self
                    .env_map
                    .get(&lambda_def.env_id)
                    .ok_or_else(|| FernError::EvalError {
                        loc: loc.clone(),
                        message: "closure environment not found".to_string(),
                    })?
                    .clone();
                let func_env = Env::new_child(closure_env);
                self.register_env(Rc::clone(&func_env));

                for (i, param) in lambda_def.params.iter().enumerate() {
                    let value = args.get(i).cloned().unwrap_or(Value::Nil);
                    func_env.borrow_mut().define(param.clone(), value);
                }

                let mut result = Value::Nil;
                for expr in &lambda_def.body {
                    result = self.eval(expr, &func_env)?;
                }
                Ok(result)
            }
            _ => Err(FernError::EvalError {
                loc,
                message: format!("`{}` is not callable", func.type_name()),
            }),
        }
    }

    /// Evaluate `(defpart name "doc" :meta (...) :params (...) :body expr)`
    fn eval_defpart(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 4 {
            return Err(FernError::EvalError {
                loc,
                message:
                    "`defpart` requires (defpart name \"doc\" :meta ... :params ... :body ...) form"
                        .to_string(),
            });
        }

        let name = match &items[1] {
            Value::Symbol(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`defpart` first argument must be a symbol".to_string(),
                });
            }
        };

        let docstring = match &items[2] {
            Value::Str(s) => s.clone(),
            _ => String::new(),
        };

        // Parse keyword arguments
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

    /// Parse a metadata list
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

    /// Parse a parameter specification list
    fn parse_param_specs(&self, expr: &Value) -> FernResult<Vec<ParamSpec>> {
        let loc = SourceLocation { line: 0, col: 0 };
        let specs_list = match expr {
            Value::List(items) => items,
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`:params` requires a list".to_string(),
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

    /// Parse `:faces` specifications
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

    /// Parse `:axes` specifications
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

    /// Evaluate `(defmeta :key val ...)`
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

    /// Evaluate `(require :module-name)`
    ///
    /// Searches for a module in user modules first, then the embedded standard library,
    /// and evaluates it in the current environment.
    fn eval_require(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 2 {
            return Err(FernError::EvalError {
                loc,
                message: "`require` requires (require :module-name) or (require \"name\") form"
                    .to_string(),
            });
        }

        let module_name = match &items[1] {
            Value::Keyword(k) => k.clone(),
            Value::Str(s) => s.clone(),
            _ => {
                return Err(FernError::EvalError {
                    loc,
                    message: "`require` argument must be a keyword or string".to_string(),
                });
            }
        };

        // Skip if already loaded
        if self.module_loader.is_loaded(&module_name) {
            return Ok(Value::Nil);
        }

        // Search user modules first, then builtins
        let source = self
            .module_loader
            .find_module(&module_name)
            .ok_or_else(|| FernError::EvalError {
                loc: loc.clone(),
                message: format!(
                    "module `{module_name}` not found. Available modules: {:?}",
                    self.module_loader.available_modules_all()
                ),
            })?;

        // Evaluate the module (source is an owned String, no borrow conflict)
        let exprs = crate::parser::parse(&source)?;
        for expr in &exprs {
            self.eval(expr, env)?;
        }

        self.module_loader.mark_loaded(&module_name);
        Ok(Value::Nil)
    }

    /// Get a mutable reference to the module loader
    pub fn module_loader_mut(&mut self) -> &mut crate::module::ModuleLoader {
        &mut self.module_loader
    }

    /// Evaluate `(assembly "name" "doc" (place ...) (place ...) (mate ...) ...)`
    /// Find spans of top-level S-expressions within a byte range of the source.
    ///
    /// Scans for balanced parenthesis groups, skipping strings and comments.
    /// Returns spans relative to the full source (absolute byte offsets).
    fn find_inner_expr_spans(source: &str, start: usize, end: usize) -> Vec<SourceSpan> {
        let mut spans = Vec::new();
        let bytes = source.as_bytes();
        let mut i = start;
        // Skip the outer opening paren
        if i < end && bytes[i] == b'(' {
            i += 1;
        }
        // Skip until we reach the closing paren of the outer list
        let outer_end = end.min(source.len());

        while i < outer_end {
            let ch = bytes[i];
            match ch {
                b' ' | b'\t' | b'\n' | b'\r' => {
                    i += 1;
                }
                b';' => {
                    // Skip line comment
                    while i < outer_end && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'"' => {
                    // String literal — record as atom span
                    let expr_start = i;
                    i += 1; // skip opening quote
                    while i < outer_end {
                        if bytes[i] == b'\\' {
                            i += 2; // skip escape sequence
                        } else if bytes[i] == b'"' {
                            i += 1;
                            break;
                        } else {
                            i += 1;
                        }
                    }
                    spans.push(SourceSpan {
                        start: expr_start,
                        end: i,
                    });
                }
                b'(' => {
                    // Balanced parenthesis group
                    let expr_start = i;
                    let mut depth = 1;
                    i += 1;
                    while i < outer_end && depth > 0 {
                        match bytes[i] {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            b'"' => {
                                i += 1;
                                while i < outer_end {
                                    if bytes[i] == b'\\' {
                                        i += 1;
                                    } else if bytes[i] == b'"' {
                                        break;
                                    }
                                    i += 1;
                                }
                            }
                            b';' => {
                                while i < outer_end && bytes[i] != b'\n' {
                                    i += 1;
                                }
                                continue;
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                    spans.push(SourceSpan {
                        start: expr_start,
                        end: i,
                    });
                }
                b')' => {
                    // Outer closing paren — stop
                    break;
                }
                _ => {
                    // Atom (symbol, number, keyword, etc.)
                    let expr_start = i;
                    while i < outer_end
                        && !matches!(
                            bytes[i],
                            b' ' | b'\t' | b'\n' | b'\r' | b'(' | b')' | b';' | b'"'
                        )
                    {
                        i += 1;
                    }
                    spans.push(SourceSpan {
                        start: expr_start,
                        end: i,
                    });
                }
            }
        }

        spans
    }

    fn eval_assembly(&mut self, items: &[Value], env: &Rc<RefCell<Env>>) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };
        if items.len() < 2 {
            return Err(FernError::EvalError {
                loc,
                message: "`assembly` requires (assembly \"name\" ...) form".to_string(),
            });
        }

        // Name (string)
        let name = match &items[1] {
            Value::Str(s) => s.clone(),
            Value::Symbol(s) => s.clone(),
            _ => "unnamed".to_string(),
        };

        // Docstring (optional)
        let (docstring, body_start) = if items.len() > 2 && matches!(&items[2], Value::Str(_)) {
            match &items[2] {
                Value::Str(s) => (s.clone(), 3),
                _ => (String::new(), 2),
            }
        } else {
            (String::new(), 2)
        };

        // Assembly scope environment
        let asm_env = Env::new_child(Rc::clone(env));
        self.register_env(Rc::clone(&asm_env));

        let mut parts = Vec::new();
        let mut constraints = Vec::new();

        // Compute per-expression spans within the assembly body
        let assembly_span = self.current_call_span.clone();
        let inner_spans =
            Self::find_inner_expr_spans(&self.source, assembly_span.start, assembly_span.end);
        // inner_spans: [assembly_keyword, name, docstring?, body_expr_0, body_expr_1, ...]
        // items[0] = assembly keyword, items[1] = name, items[body_start..] = body

        // Evaluate each expression in the body
        for (i, expr) in items[body_start..].iter().enumerate() {
            // Set span for this specific body expression
            // inner_spans indices match items indices (both start after the outer paren)
            let span_idx = body_start + i;
            if let Some(span) = inner_spans.get(span_idx) {
                self.current_call_span = span.clone();
            }
            let result = self.eval(expr, &asm_env)?;

            // Collect place results as parts
            if let Value::List(ref list) = result {
                if list.len() >= 2 && matches!(&list[0], Value::Keyword(_)) {
                    // place result: [name_keyword, shape, nil, position]
                    if let Some(pi) = self.extract_part_instance(list, env)? {
                        parts.push(pi);
                        continue;
                    }
                }
            }

            // Collect constraint values
            if let Some(constraint) = self.try_extract_constraint(&result) {
                constraints.push(constraint);
            }
        }

        // Restore assembly-level span
        self.current_call_span = assembly_span;

        // Auto-assign colors
        for (i, part) in parts.iter_mut().enumerate() {
            part.color = crate::assembly::part_color(i);
        }

        let assembly = crate::assembly::AssemblyDef {
            name,
            docstring,
            parts,
            constraints,
        };

        Ok(Value::Assembly(Arc::new(assembly)))
    }

    /// Extract a Constraint from a constraint value
    fn try_extract_constraint(&self, value: &Value) -> Option<crate::assembly::Constraint> {
        match value {
            Value::List(items) if items.len() >= 2 => {
                if let Value::Symbol(s) = &items[0] {
                    match s.as_str() {
                        "constraint-mate" => {
                            if let (Value::FaceRef(f1), Value::FaceRef(f2)) = (&items[1], &items[2])
                            {
                                let offset =
                                    items.get(3).and_then(|v| v.as_number()).unwrap_or(0.0);
                                return Some(crate::assembly::Constraint::Mate {
                                    face1: f1.clone(),
                                    face2: f2.clone(),
                                    offset,
                                });
                            }
                        }
                        "constraint-align-axis" => {
                            if let (Value::AxisRef(a1), Value::AxisRef(a2)) = (&items[1], &items[2])
                            {
                                return Some(crate::assembly::Constraint::AlignAxis {
                                    axis1: a1.clone(),
                                    axis2: a2.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Create a PartInstance from part data list
    fn extract_part_instance(
        &mut self,
        data: &[Value],
        _env: &Rc<RefCell<Env>>,
    ) -> FernResult<Option<crate::assembly::PartInstance>> {
        // data: [name_keyword, shape_value, part_def_or_nil, position]
        if data.len() < 2 {
            return Ok(None);
        }
        let name = match &data[0] {
            Value::Keyword(k) => k.clone(),
            Value::Str(s) => s.clone(),
            _ => return Ok(None),
        };

        let shape = match &data[1] {
            Value::Shape(s) => Some(Arc::clone(s)),
            _ => None,
        };

        let part_def = data.get(2).and_then(|v| {
            if let Value::PartDef(pd) = v {
                Some(Arc::clone(pd))
            } else {
                None
            }
        });

        let dummy_part_def = part_def.unwrap_or_else(|| {
            Arc::new(PartDef {
                name: name.clone(),
                docstring: String::new(),
                meta: HashMap::new(),
                params: Vec::new(),
                faces: Vec::new(),
                axes: Vec::new(),
                body: Vec::new(),
                env_id: 0,
            })
        });

        let mut instance = crate::assembly::PartInstance::new(name, dummy_part_def, HashMap::new());
        instance.shape = shape;

        // Initial position
        if let Some(Value::Point3(p)) = data.get(3) {
            instance.translate(*p);
        }

        Ok(Some(instance))
    }

    /// Apply a Lambda to arguments
    fn apply_lambda(
        &mut self,
        lambda: &LambdaDef,
        args: &[Value],
        call_env: &Rc<RefCell<Env>>,
    ) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };

        // Get the closure's definition-time environment
        let closure_env = self
            .env_map
            .get(&lambda.env_id)
            .ok_or_else(|| FernError::EvalError {
                loc: loc.clone(),
                message: "closure environment for function not found".to_string(),
            })?
            .clone();

        let func_env = Env::new_child(closure_env);
        self.register_env(Rc::clone(&func_env));

        // Evaluate and bind arguments
        let evaluated_args = self.eval_keyword_args(args, call_env)?;

        // Separate positional and keyword arguments
        let (positional, kwargs) = split_kwargs(&evaluated_args);

        // Bind positional arguments
        for (i, param) in lambda.params.iter().enumerate() {
            if let Some(value) = kwargs.get(param) {
                func_env.borrow_mut().define(param.clone(), value.clone());
            } else if let Some(value) = positional.get(i) {
                func_env.borrow_mut().define(param.clone(), value.clone());
            } else {
                return Err(FernError::EvalError {
                    loc,
                    message: format!("no value provided for argument `{param}`"),
                });
            }
        }

        // Evaluate body sequentially
        let mut result = Value::Nil;
        for expr in &lambda.body {
            result = self.eval(expr, &func_env)?;
        }
        Ok(result)
    }

    /// Instantiate a PartDef
    fn apply_part(
        &mut self,
        part: &PartDef,
        args: &[Value],
        call_env: &Rc<RefCell<Env>>,
    ) -> FernResult<Value> {
        let loc = SourceLocation { line: 0, col: 0 };

        // Get the definition-time environment
        let closure_env = self
            .env_map
            .get(&part.env_id)
            .ok_or_else(|| FernError::EvalError {
                loc: loc.clone(),
                message: "closure environment for part not found".to_string(),
            })?
            .clone();

        let part_env = Env::new_child(closure_env);
        self.register_env(Rc::clone(&part_env));

        // Evaluate keyword arguments
        let evaluated_args = self.eval_keyword_args(args, call_env)?;
        let (_, kwargs) = split_kwargs(&evaluated_args);

        // Bind parameters (keyword arguments or default values)
        for param in &part.params {
            let value = if let Some(v) = kwargs.get(&param.name) {
                v.clone()
            } else if let Some(default) = &param.default {
                default.clone()
            } else {
                return Err(FernError::EvalError {
                    loc: loc.clone(),
                    message: format!(
                        "part `{}` parameter `{}` has no value and no default",
                        part.name, param.name
                    ),
                });
            };
            // Check type annotation if present
            if let Some(ref type_name) = param.type_annotation {
                if !value.matches_type(type_name) {
                    return Err(FernError::TypeError {
                        loc: loc.clone(),
                        expected: type_name.clone(),
                        actual: value.type_name().to_string(),
                    });
                }
            }
            part_env.borrow_mut().define(param.name.clone(), value);
        }

        // Evaluate body
        let mut result = Value::Nil;
        for expr in &part.body {
            result = self.eval(expr, &part_env)?;
        }
        Ok(result)
    }

    /// Evaluate an argument list that may contain keyword arguments
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

    /// Extract parameter names from a parameter list
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
                message: "parameter list required".to_string(),
            }),
        }
    }

    // === Built-in function registration ===

    /// Register all built-in functions
    fn register_builtins(&mut self) {
        // Arithmetic
        self.register_builtin("+", builtin_add);
        self.register_builtin("-", builtin_sub);
        self.register_builtin("*", builtin_mul);
        self.register_builtin("/", builtin_div);

        // Comparison
        self.register_builtin("=", builtin_eq);
        self.register_builtin("<", builtin_lt);
        self.register_builtin(">", builtin_gt);
        self.register_builtin("<=", builtin_le);
        self.register_builtin(">=", builtin_ge);

        // Logic
        self.register_builtin("not", builtin_not);

        // List
        self.register_builtin("list", builtin_list);
        self.register_builtin("cons", builtin_cons);
        self.register_builtin("car", builtin_car);
        self.register_builtin("cdr", builtin_cdr);
        self.register_builtin("append", builtin_append);
        self.register_builtin("nth", builtin_nth);
        self.register_builtin("length", builtin_length);
        self.register_builtin("reverse", builtin_reverse);
        self.register_builtin("last", builtin_last);

        // Math
        self.register_builtin("cos", builtin_cos);
        self.register_builtin("sin", builtin_sin);
        self.register_builtin("tan", builtin_tan);
        self.register_builtin("acos", builtin_acos);
        self.register_builtin("atan", builtin_atan);
        self.register_builtin("atan2", builtin_atan2);
        self.register_builtin("sqrt", builtin_sqrt);
        self.register_builtin("abs", builtin_abs);
        self.register_builtin("mod", builtin_mod);
        self.register_builtin("expt", builtin_expt);
        self.register_builtin("floor", builtin_floor);
        self.register_builtin("ceil", builtin_ceil);
        self.register_builtin("min", builtin_min);
        self.register_builtin("max", builtin_max);

        // Type predicates
        self.register_builtin("numberp", builtin_numberp);
        self.register_builtin("listp", builtin_listp);
        self.register_builtin("nilp", builtin_nilp);
        self.register_builtin("stringp", builtin_stringp);

        // String
        self.register_builtin("format", builtin_format);

        // Constants
        self.env
            .borrow_mut()
            .define("pi".to_string(), Value::Float(std::f64::consts::PI));
        self.env.borrow_mut().define(
            "*resolution*".to_string(),
            Value::Int(DEFAULT_SEGMENTS as i64),
        );

        // Debug
        self.register_builtin("print", builtin_print);
        self.register_builtin("macroexpand-1", builtin_macroexpand_1);

        // CAD primitives
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

        // Transforms
        self.register_builtin("translate", builtin_translate);
        self.register_builtin("rotate", builtin_rotate);
        self.register_builtin("scale", builtin_scale);

        // Unit conversion
        self.register_builtin("to-mm", builtin_to_mm);
        self.register_builtin("to-rad", builtin_to_rad);

        // Face/axis references
        self.register_builtin("face", builtin_face);
        self.register_builtin("axis", builtin_axis);

        // Assembly
        self.register_builtin("place", builtin_place);

        // Constraints (used within assemblies)
        self.register_builtin("mate", builtin_mate);
        self.register_builtin("align-axis", builtin_align_axis);
        self.register_builtin("fit", builtin_fit);
        self.register_builtin("joint", builtin_joint);

        // Profile operations
        self.register_builtin("polygon", builtin_polygon);
        self.register_builtin("circle", builtin_circle);
        self.register_builtin("extrude", builtin_extrude);
        self.register_builtin("revolve", builtin_revolve);

        // Path constructors
        self.register_builtin("helix", builtin_helix);
        self.register_builtin("arc", builtin_arc);
        self.register_builtin("bezier", builtin_bezier);

        // Sweep & Loft
        self.register_builtin("sweep", builtin_sweep);
        self.register_builtin("loft", builtin_loft);

        // Edge operations
        self.register_builtin("chamfer", builtin_chamfer);
        self.register_builtin("fillet", builtin_fillet);
        self.register_builtin("shell", builtin_shell);
    }

    /// Register a built-in function
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

    /// Get a reference to the global environment
    pub fn global_env(&self) -> &Rc<RefCell<Env>> {
        &self.env
    }

    /// Set the current call span (used by completion for partial evaluation)
    pub fn set_call_span(&mut self, span: SourceSpan) {
        self.current_call_span = span;
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

// === Helper functions ===

/// Filter out :: type annotations
fn filter_type_annotations(params: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < params.len() {
        if params[i] == "::" {
            // Skip :: and the following type name
            i += 2;
        } else {
            result.push(params[i].clone());
            i += 1;
        }
    }
    result
}

/// Find the value index after :: type annotation in a let* binding
fn find_value_index_after_annotation(pair: &[Value]) -> usize {
    // (name :: type value) -> value is at index 3
    // (name value) -> value is at index 1
    if pair.len() >= 4 {
        if let Value::Symbol(s) = &pair[1] {
            if s == "::" {
                return 3;
            }
        }
    }
    1
}

/// Separate keyword arguments from evaluated arguments
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

/// Helper to get an f64 value from keyword arguments
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
                expected: "number".to_string(),
                actual: v.type_name().to_string(),
            }),
        },
        None => Ok(None),
    }
}

/// Get a required f64 keyword argument
fn require_kwarg_f64(
    kwargs: &HashMap<String, Value>,
    key: &str,
    func_name: &str,
    loc: &SourceLocation,
) -> FernResult<f64> {
    get_kwarg_f64(kwargs, key, loc)?.ok_or_else(|| FernError::EvalError {
        loc: loc.clone(),
        message: format!("`{func_name}` requires keyword argument `:{key}`"),
    })
}

/// Get an argument as a ShapeNode (extracts from TrackedShape)
fn get_shape_arg(value: &Value, loc: &SourceLocation) -> FernResult<Arc<ShapeNode>> {
    match value {
        Value::Shape(s) => Ok(Arc::clone(&s.node)),
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "shape".to_string(),
            actual: value.type_name().to_string(),
        }),
    }
}

// === Built-in function implementations ===

fn builtin_add(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let mut sum = 0.0_f64;
    for arg in args {
        sum += arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: arg.type_name().to_string(),
        })?;
    }
    Ok(Value::Float(sum))
}

fn builtin_sub(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`-` requires at least one argument".to_string(),
        });
    }
    let first = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    if args.len() == 1 {
        return Ok(Value::Float(-first));
    }
    let mut result = first;
    for arg in &args[1..] {
        result -= arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: arg.type_name().to_string(),
        })?;
    }
    Ok(Value::Float(result))
}

fn builtin_mul(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let mut product = 1.0_f64;
    for arg in args {
        product *= arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: arg.type_name().to_string(),
        })?;
    }
    Ok(Value::Float(product))
}

fn builtin_div(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() < 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`/` requires at least two arguments".to_string(),
        });
    }
    let mut result = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    for arg in &args[1..] {
        let divisor = arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: arg.type_name().to_string(),
        })?;
        if divisor == 0.0 {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "division by zero".to_string(),
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
            expected: "number".to_string(),
            actual: window[0].type_name().to_string(),
        })?;
        let b = window[1].as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: window[1].type_name().to_string(),
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
            message: "`not` requires exactly one argument".to_string(),
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
            message: "`cos` requires exactly one argument".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    Ok(Value::Float(v.cos()))
}

fn builtin_sin(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`sin` requires exactly one argument".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    Ok(Value::Float(v.sin()))
}

fn builtin_sqrt(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`sqrt` requires exactly one argument".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    Ok(Value::Float(v.sqrt()))
}

/// Helper: extract a single numeric argument from args
fn require_single_number(args: &[Value], name: &str, loc: &SourceLocation) -> FernResult<f64> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: format!("`{name}` requires exactly one argument"),
        });
    }
    args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })
}

/// Helper: extract two numeric arguments from args
fn require_two_numbers(args: &[Value], name: &str, loc: &SourceLocation) -> FernResult<(f64, f64)> {
    if args.len() != 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: format!("`{name}` requires exactly two arguments"),
        });
    }
    let a = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    let b = args[1].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[1].type_name().to_string(),
    })?;
    Ok((a, b))
}

/// `(tan x)` — tangent
fn builtin_tan(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let v = require_single_number(args, "tan", loc)?;
    Ok(Value::Float(v.tan()))
}

/// `(acos x)` — arc cosine
fn builtin_acos(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let v = require_single_number(args, "acos", loc)?;
    Ok(Value::Float(v.acos()))
}

/// `(atan x)` — arc tangent (single argument)
fn builtin_atan(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let v = require_single_number(args, "atan", loc)?;
    Ok(Value::Float(v.atan()))
}

/// `(atan2 y x)` — two-argument arc tangent
fn builtin_atan2(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (y, x) = require_two_numbers(args, "atan2", loc)?;
    Ok(Value::Float(y.atan2(x)))
}

/// `(abs x)` — absolute value
fn builtin_abs(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let v = require_single_number(args, "abs", loc)?;
    Ok(Value::Float(v.abs()))
}

/// `(mod a b)` — modulo (remainder)
fn builtin_mod(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (a, b) = require_two_numbers(args, "mod", loc)?;
    if b == 0.0 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "division by zero in `mod`".to_string(),
        });
    }
    Ok(Value::Float(a % b))
}

/// `(expt base power)` — exponentiation
fn builtin_expt(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (base, power) = require_two_numbers(args, "expt", loc)?;
    Ok(Value::Float(base.powf(power)))
}

/// `(floor x)` — round down
fn builtin_floor(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let v = require_single_number(args, "floor", loc)?;
    Ok(Value::Float(v.floor()))
}

/// `(ceil x)` — round up
fn builtin_ceil(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let v = require_single_number(args, "ceil", loc)?;
    Ok(Value::Float(v.ceil()))
}

/// `(min a b ...)` — minimum value
fn builtin_min(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`min` requires at least one argument".to_string(),
        });
    }
    let mut result = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    for arg in &args[1..] {
        let v = arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: arg.type_name().to_string(),
        })?;
        if v < result {
            result = v;
        }
    }
    Ok(Value::Float(result))
}

/// `(max a b ...)` — maximum value
fn builtin_max(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`max` requires at least one argument".to_string(),
        });
    }
    let mut result = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    for arg in &args[1..] {
        let v = arg.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: arg.type_name().to_string(),
        })?;
        if v > result {
            result = v;
        }
    }
    Ok(Value::Float(result))
}

// === List built-in functions ===

/// `(cons item list)` — prepend item to list
fn builtin_cons(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`cons` requires exactly two arguments".to_string(),
        });
    }
    let mut result = match &args[1] {
        Value::List(l) => l.clone(),
        Value::Nil => Vec::new(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "list".to_string(),
                actual: args[1].type_name().to_string(),
            });
        }
    };
    result.insert(0, args[0].clone());
    Ok(Value::List(result))
}

/// `(car list)` — first element
fn builtin_car(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`car` requires exactly one argument".to_string(),
        });
    }
    match &args[0] {
        Value::List(l) if !l.is_empty() => Ok(l[0].clone()),
        Value::List(_) | Value::Nil => Ok(Value::Nil),
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "list".to_string(),
            actual: args[0].type_name().to_string(),
        }),
    }
}

/// `(cdr list)` — tail (all but first)
fn builtin_cdr(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`cdr` requires exactly one argument".to_string(),
        });
    }
    match &args[0] {
        Value::List(l) if l.len() > 1 => Ok(Value::List(l[1..].to_vec())),
        Value::List(_) | Value::Nil => Ok(Value::Nil),
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "list".to_string(),
            actual: args[0].type_name().to_string(),
        }),
    }
}

/// `(append list1 list2 ...)` — concatenate lists
fn builtin_append(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let mut result = Vec::new();
    for arg in args {
        match arg {
            Value::List(l) => result.extend(l.iter().cloned()),
            Value::Nil => {}
            _ => {
                return Err(FernError::TypeError {
                    loc: loc.clone(),
                    expected: "list".to_string(),
                    actual: arg.type_name().to_string(),
                });
            }
        }
    }
    Ok(Value::List(result))
}

/// `(nth n list)` — element at index n (0-based)
fn builtin_nth(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`nth` requires exactly two arguments (index list)".to_string(),
        });
    }
    let index = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })? as usize;
    match &args[1] {
        Value::List(l) => Ok(l.get(index).cloned().unwrap_or(Value::Nil)),
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "list".to_string(),
            actual: args[1].type_name().to_string(),
        }),
    }
}

/// `(length list)` — number of elements
fn builtin_length(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`length` requires exactly one argument".to_string(),
        });
    }
    match &args[0] {
        Value::List(l) => Ok(Value::Int(l.len() as i64)),
        Value::Str(s) => Ok(Value::Int(s.len() as i64)),
        Value::Nil => Ok(Value::Int(0)),
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "list or string".to_string(),
            actual: args[0].type_name().to_string(),
        }),
    }
}

/// `(reverse list)` — reversed list
fn builtin_reverse(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`reverse` requires exactly one argument".to_string(),
        });
    }
    match &args[0] {
        Value::List(l) => {
            let mut reversed = l.clone();
            reversed.reverse();
            Ok(Value::List(reversed))
        }
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "list".to_string(),
            actual: args[0].type_name().to_string(),
        }),
    }
}

/// `(last list)` — last element
fn builtin_last(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`last` requires exactly one argument".to_string(),
        });
    }
    match &args[0] {
        Value::List(l) => Ok(l.last().cloned().unwrap_or(Value::Nil)),
        Value::Nil => Ok(Value::Nil),
        _ => Err(FernError::TypeError {
            loc: loc.clone(),
            expected: "list".to_string(),
            actual: args[0].type_name().to_string(),
        }),
    }
}

// === Type predicates ===

/// `(numberp x)` — is x a number?
fn builtin_numberp(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`numberp` requires exactly one argument".to_string(),
        });
    }
    Ok(Value::Bool(args[0].as_number().is_some()))
}

/// `(listp x)` — is x a list?
fn builtin_listp(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`listp` requires exactly one argument".to_string(),
        });
    }
    Ok(Value::Bool(matches!(args[0], Value::List(_))))
}

/// `(nilp x)` — is x nil?
fn builtin_nilp(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`nilp` requires exactly one argument".to_string(),
        });
    }
    Ok(Value::Bool(matches!(
        args[0],
        Value::Nil | Value::Bool(false)
    )))
}

/// `(stringp x)` — is x a string?
fn builtin_stringp(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`stringp` requires exactly one argument".to_string(),
        });
    }
    Ok(Value::Bool(matches!(args[0], Value::Str(_))))
}

// === String functions ===

/// `(format template args...)` — simple string formatting with ~a placeholders
fn builtin_format(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`format` requires at least a template string".to_string(),
        });
    }
    let template = match &args[0] {
        Value::Str(s) => s.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "string".to_string(),
                actual: args[0].type_name().to_string(),
            });
        }
    };

    let mut result = String::new();
    let mut arg_idx = 1;
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '~' {
            if let Some(&directive) = chars.peek() {
                match directive {
                    'a' | 'A' => {
                        chars.next();
                        if arg_idx < args.len() {
                            match &args[arg_idx] {
                                Value::Str(s) => result.push_str(s),
                                other => result.push_str(&format!("{other}")),
                            }
                            arg_idx += 1;
                        }
                    }
                    'd' | 'D' => {
                        chars.next();
                        if arg_idx < args.len() {
                            if let Some(n) = args[arg_idx].as_number() {
                                result.push_str(&format!("{}", n as i64));
                            } else {
                                result.push_str(&format!("{}", args[arg_idx]));
                            }
                            arg_idx += 1;
                        }
                    }
                    'f' | 'F' => {
                        chars.next();
                        if arg_idx < args.len() {
                            if let Some(n) = args[arg_idx].as_number() {
                                result.push_str(&format!("{n}"));
                            } else {
                                result.push_str(&format!("{}", args[arg_idx]));
                            }
                            arg_idx += 1;
                        }
                    }
                    '~' => {
                        chars.next();
                        result.push('~');
                    }
                    '%' => {
                        chars.next();
                        result.push('\n');
                    }
                    _ => {
                        result.push(c);
                    }
                }
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }
    Ok(Value::Str(result))
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

/// `macroexpand-1` — expand a macro call once without evaluating the result
fn builtin_macroexpand_1(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    // This is a placeholder; actual expansion requires evaluator context.
    // The real work is done by the evaluator when it encounters a macro call.
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`macroexpand-1` requires a quoted form argument".to_string(),
        });
    }
    // Return the form as-is since we can't expand without evaluator context
    // (actual expansion happens when the form is evaluated)
    Ok(args[0].clone())
}

// === CAD built-in functions ===

fn builtin_box(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let width = require_kwarg_f64(&kwargs, "width", "box", loc)?;
    let depth = require_kwarg_f64(&kwargs, "depth", "box", loc)?;
    let height = require_kwarg_f64(&kwargs, "height", "box", loc)?;
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Box {
            width,
            depth,
            height,
        },
    ))))
}

fn builtin_cube(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`cube` requires one numeric argument (e.g. (cube 10.0))".to_string(),
        });
    }
    let size = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Box {
            width: size,
            depth: size,
            height: size,
        },
    ))))
}

fn builtin_sphere(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius = require_kwarg_f64(&kwargs, "radius", "sphere", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Sphere { radius, segments },
    ))))
}

fn builtin_cylinder(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius = require_kwarg_f64(&kwargs, "radius", "cylinder", loc)?;
    let height = require_kwarg_f64(&kwargs, "height", "cylinder", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Cylinder {
            radius,
            height,
            segments,
        },
    ))))
}

fn builtin_cone(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius_bottom = require_kwarg_f64(&kwargs, "radius-bottom", "cone", loc)?;
    let radius_top = get_kwarg_f64(&kwargs, "radius-top", loc)?.unwrap_or(0.0);
    let height = require_kwarg_f64(&kwargs, "height", "cone", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Cone {
            radius_bottom,
            radius_top,
            height,
            segments,
        },
    ))))
}

fn builtin_prism(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let sides = require_kwarg_f64(&kwargs, "sides", "prism", loc)? as u32;
    let radius = require_kwarg_f64(&kwargs, "radius", "prism", loc)?;
    let height = require_kwarg_f64(&kwargs, "height", "prism", loc)?;
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Prism {
            sides,
            radius,
            height,
        },
    ))))
}

fn builtin_torus(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius_major = require_kwarg_f64(&kwargs, "radius-major", "torus", loc)?;
    let radius_minor = require_kwarg_f64(&kwargs, "radius-minor", "torus", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Torus {
            radius_major,
            radius_minor,
            segments,
        },
    ))))
}

fn builtin_union(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let children: Vec<_> = args
        .iter()
        .map(|a| get_shape_arg(a, loc))
        .collect::<FernResult<Vec<_>>>()?;
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Union { children },
    ))))
}

fn builtin_difference(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`difference` requires at least one argument".to_string(),
        });
    }
    let base = get_shape_arg(&args[0], loc)?;
    let cutters: Vec<_> = args[1..]
        .iter()
        .map(|a| get_shape_arg(a, loc))
        .collect::<FernResult<Vec<_>>>()?;
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Difference { base, cutters },
    ))))
}

fn builtin_intersection(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let children: Vec<_> = args
        .iter()
        .map(|a| get_shape_arg(a, loc))
        .collect::<FernResult<Vec<_>>>()?;
    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Intersection { children },
    ))))
}

fn builtin_translate(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);

    let shape = kwargs
        .get("shape")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`translate` requires keyword argument `:shape`".to_string(),
        })
        .and_then(|v| get_shape_arg(v, loc))?;

    let offset = kwargs
        .get("by")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`translate` requires keyword argument `:by`".to_string(),
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
                expected: "vector".to_string(),
                actual: v.type_name().to_string(),
            }),
        })?;

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Translate { shape, offset },
    ))))
}

fn builtin_rotate(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);

    let shape = kwargs
        .get("shape")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`rotate` requires keyword argument `:shape`".to_string(),
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
                    message: format!("unsupported axis `:{k}`. Use :x, :y, or :z"),
                });
            }
        },
        Some(Value::Vec3(v)) => *v,
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "`rotate` requires keyword argument `:axis`".to_string(),
            });
        }
    };

    let angle_rad = match kwargs.get("angle") {
        Some(Value::Angle(a)) => *a,
        Some(v) => v.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "angle".to_string(),
            actual: v.type_name().to_string(),
        })?,
        None => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "`rotate` requires keyword argument `:angle`".to_string(),
            });
        }
    };

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Rotate {
            shape,
            axis,
            angle_rad,
        },
    ))))
}

fn builtin_scale(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);

    let shape = kwargs
        .get("shape")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`scale` requires keyword argument `:shape`".to_string(),
        })
        .and_then(|v| get_shape_arg(v, loc))?;

    let factors = if let Some(f) = kwargs.get("factor") {
        let v = f.as_number().ok_or_else(|| FernError::TypeError {
            loc: loc.clone(),
            expected: "number".to_string(),
            actual: f.type_name().to_string(),
        })?;
        [v, v, v]
    } else {
        let x = get_kwarg_f64(&kwargs, "x", loc)?.unwrap_or(1.0);
        let y = get_kwarg_f64(&kwargs, "y", loc)?.unwrap_or(1.0);
        let z = get_kwarg_f64(&kwargs, "z", loc)?.unwrap_or(1.0);
        [x, y, z]
    };

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Scale { shape, factors },
    ))))
}

fn builtin_to_mm(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`to-mm` requires one length argument".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    Ok(Value::Float(v))
}

fn builtin_to_rad(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 1 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`to-rad` requires one angle argument".to_string(),
        });
    }
    let v = args[0].as_number().ok_or_else(|| FernError::TypeError {
        loc: loc.clone(),
        expected: "number".to_string(),
        actual: args[0].type_name().to_string(),
    })?;
    Ok(Value::Float(v))
}

/// `(face instance-keyword :face-name)` -- returns a face reference
fn builtin_face(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`face` requires (face :instance-name :face-name) form".to_string(),
        });
    }
    let instance_name = match &args[0] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "keyword".to_string(),
                actual: args[0].type_name().to_string(),
            });
        }
    };
    let face_name = match &args[1] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "keyword".to_string(),
                actual: args[1].type_name().to_string(),
            });
        }
    };
    Ok(Value::FaceRef(crate::face::FaceRef {
        instance_name,
        face_name,
    }))
}

/// `(axis instance-keyword :axis-name)` -- returns an axis reference
fn builtin_axis(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() != 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`axis` requires (axis :instance-name :axis-name) form".to_string(),
        });
    }
    let instance_name = match &args[0] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "keyword".to_string(),
                actual: args[0].type_name().to_string(),
            });
        }
    };
    let axis_name = match &args[1] {
        Value::Keyword(k) => k.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "keyword".to_string(),
                actual: args[1].type_name().to_string(),
            });
        }
    };
    Ok(Value::AxisRef(crate::face::AxisRef {
        instance_name,
        axis_name,
    }))
}

// === Assembly constraint builtins ===

/// `(place :part shape :as :name :at #p(...))` -- place a part
/// Results are accumulated in the assembly environment's *assembly-parts*
fn builtin_place(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);

    let shape = kwargs.get("part").cloned().unwrap_or(Value::Nil);
    let name = match kwargs.get("as") {
        Some(Value::Keyword(k)) => Value::Keyword(k.clone()),
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "`place` requires keyword argument `:as`".to_string(),
            });
        }
    };
    let position = kwargs
        .get("at")
        .cloned()
        .unwrap_or(Value::Point3([0.0, 0.0, 0.0]));

    // Return part data as a list (eval_assembly collects it)
    Ok(Value::List(vec![name, shape, Value::Nil, position]))
}

/// `(mate face-ref1 face-ref2)` -- mate faces constraint
fn builtin_mate(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() < 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`mate` requires (mate face-ref1 face-ref2) form".to_string(),
        });
    }
    let f1 = match &args[0] {
        Value::FaceRef(r) => r.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "face-ref".to_string(),
                actual: args[0].type_name().to_string(),
            });
        }
    };
    let f2 = match &args[1] {
        Value::FaceRef(r) => r.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "face-ref".to_string(),
                actual: args[1].type_name().to_string(),
            });
        }
    };
    Ok(Value::List(vec![
        Value::Symbol("constraint-mate".to_string()),
        Value::FaceRef(f1),
        Value::FaceRef(f2),
    ]))
}

/// `(align-axis axis-ref1 axis-ref2)` -- align axes constraint
fn builtin_align_axis(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.len() < 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`align-axis` requires (align-axis axis-ref1 axis-ref2) form".to_string(),
        });
    }
    let a1 = match &args[0] {
        Value::AxisRef(r) => r.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "axis-ref".to_string(),
                actual: args[0].type_name().to_string(),
            });
        }
    };
    let a2 = match &args[1] {
        Value::AxisRef(r) => r.clone(),
        _ => {
            return Err(FernError::TypeError {
                loc: loc.clone(),
                expected: "axis-ref".to_string(),
                actual: args[1].type_name().to_string(),
            });
        }
    };
    Ok(Value::List(vec![
        Value::Symbol("constraint-align-axis".to_string()),
        Value::AxisRef(a1),
        Value::AxisRef(a2),
    ]))
}

/// `(fit :shaft face-ref :hole face-ref :clearance 0.1 :type :clearance)` -- fit constraint
fn builtin_fit(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let shaft = kwargs.get("shaft").ok_or_else(|| FernError::EvalError {
        loc: loc.clone(),
        message: "`fit` requires `:shaft`".to_string(),
    })?;
    let hole = kwargs.get("hole").ok_or_else(|| FernError::EvalError {
        loc: loc.clone(),
        message: "`fit` requires `:hole`".to_string(),
    })?;
    Ok(Value::List(vec![
        Value::Symbol("constraint-fit".to_string()),
        shaft.clone(),
        hole.clone(),
    ]))
}

/// `(joint :type :fixed :parts (list :a :b))` -- joint definition
fn builtin_joint(args: &[Value], _loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let joint_type = kwargs
        .get("type")
        .cloned()
        .unwrap_or(Value::Keyword("fixed".to_string()));
    let parts = kwargs
        .get("parts")
        .cloned()
        .unwrap_or(Value::List(Vec::new()));
    Ok(Value::List(vec![
        Value::Symbol("constraint-joint".to_string()),
        joint_type,
        parts,
    ]))
}

// === Profile operations ===

/// `(polygon (list x1 y1) (list x2 y2) ...)` — create a 2D polygon from point pairs
fn builtin_polygon(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    if args.is_empty() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`polygon` requires at least 3 points: (polygon (list x y) ...)".to_string(),
        });
    }

    let mut points = Vec::new();
    for arg in args {
        match arg {
            Value::List(coords) if coords.len() == 2 => {
                let x = coords[0].as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "polygon point x must be a number".to_string(),
                })?;
                let y = coords[1].as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "polygon point y must be a number".to_string(),
                })?;
                points.push(Value::List(vec![Value::Float(x), Value::Float(y)]));
            }
            Value::Point3(p) => {
                // Allow 3D points, use X and Y
                points.push(Value::List(vec![Value::Float(p[0]), Value::Float(p[1])]));
            }
            _ => {
                return Err(FernError::EvalError {
                    loc: loc.clone(),
                    message: format!(
                        "polygon expects 2D point pairs (list x y), got: {}",
                        arg.type_name()
                    ),
                });
            }
        }
    }

    if points.len() < 3 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "polygon requires at least 3 points".to_string(),
        });
    }

    Ok(Value::List(points))
}

/// `(extrude :profile polygon :height h)` — extrude a 2D polygon along Z
fn builtin_extrude(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let profile = require_kwarg(&kwargs, "profile", "extrude", loc)?;
    let height = require_kwarg_f64(&kwargs, "height", "extrude", loc)?;

    let points = extract_polygon_points(profile, loc)?;

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Extrude {
            profile: points,
            height,
        },
    ))))
}

/// `(revolve :profile polygon :angle angle)` — revolve a 2D polygon around Z
fn builtin_revolve(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let profile = require_kwarg(&kwargs, "profile", "revolve", loc)?;
    let angle = get_kwarg_f64(&kwargs, "angle", loc)?.unwrap_or(2.0 * std::f64::consts::PI);
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);

    let points = extract_polygon_points(profile, loc)?;

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Revolve {
            profile: points,
            angle_rad: angle,
            segments,
        },
    ))))
}

/// Extract 2D points from a polygon Value::List
fn extract_polygon_points(val: &Value, loc: &SourceLocation) -> FernResult<Vec<[f64; 2]>> {
    let items = match val {
        Value::List(items) => items,
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "profile must be a polygon (list of 2D points)".to_string(),
            });
        }
    };

    let mut points = Vec::new();
    for item in items {
        match item {
            Value::List(coords) if coords.len() == 2 => {
                let x = coords[0].as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "polygon point x must be a number".to_string(),
                })?;
                let y = coords[1].as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "polygon point y must be a number".to_string(),
                })?;
                points.push([x, y]);
            }
            _ => {
                return Err(FernError::EvalError {
                    loc: loc.clone(),
                    message: "polygon point must be a 2-element list (list x y)".to_string(),
                });
            }
        }
    }

    if points.len() < 3 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "profile requires at least 3 points".to_string(),
        });
    }

    Ok(points)
}

/// `(circle :radius r :segments s)` — generate a polygon approximation of a circle
fn builtin_circle(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius = require_kwarg_f64(&kwargs, "radius", "circle", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);

    if radius <= 0.0 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`circle` requires positive radius".to_string(),
        });
    }

    let mut points = Vec::with_capacity(segments as usize);
    for i in 0..segments {
        let theta = 2.0 * std::f64::consts::PI * i as f64 / segments as f64;
        points.push(Value::List(vec![
            Value::Float(radius * theta.cos()),
            Value::Float(radius * theta.sin()),
        ]));
    }
    Ok(Value::List(points))
}

// === Path constructors ===

/// `(helix :radius r :pitch p :turns n)` — helical path in 3D
fn builtin_helix(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius = require_kwarg_f64(&kwargs, "radius", "helix", loc)?;
    let pitch = require_kwarg_f64(&kwargs, "pitch", "helix", loc)?;
    let turns = require_kwarg_f64(&kwargs, "turns", "helix", loc)?;

    if radius < 0.0 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`helix` requires non-negative radius".to_string(),
        });
    }
    if turns <= 0.0 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`helix` requires positive turns".to_string(),
        });
    }

    Ok(Value::Path(Arc::new(PathNode::Helix {
        radius,
        pitch,
        turns,
    })))
}

/// `(arc :radius r :angle a)` — circular arc path in XY plane
fn builtin_arc(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let radius = require_kwarg_f64(&kwargs, "radius", "arc", loc)?;
    let angle = get_kwarg_f64(&kwargs, "angle", loc)?.unwrap_or(2.0 * std::f64::consts::PI);

    if radius <= 0.0 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`arc` requires positive radius".to_string(),
        });
    }

    Ok(Value::Path(Arc::new(PathNode::Arc {
        radius,
        angle_rad: angle,
    })))
}

/// `(bezier :points ((list x y z) ...))` — Bezier curve path
fn builtin_bezier(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let points_val = require_kwarg(&kwargs, "points", "bezier", loc)?;
    let points = extract_3d_points(points_val, loc)?;

    if points.len() < 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`bezier` requires at least 2 control points".to_string(),
        });
    }

    Ok(Value::Path(Arc::new(PathNode::Bezier { points })))
}

/// Extract 3D points from a Value::List of (list x y z)
fn extract_3d_points(val: &Value, loc: &SourceLocation) -> FernResult<Vec<[f64; 3]>> {
    let items = match val {
        Value::List(items) => items,
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "expected a list of 3D points".to_string(),
            });
        }
    };

    let mut points = Vec::new();
    for item in items {
        match item {
            Value::List(coords) if coords.len() == 3 => {
                let x = coords[0].as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "point x must be a number".to_string(),
                })?;
                let y = coords[1].as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "point y must be a number".to_string(),
                })?;
                let z = coords[2].as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "point z must be a number".to_string(),
                })?;
                points.push([x, y, z]);
            }
            Value::Vec3(v) => points.push(*v),
            Value::Point3(p) => points.push(*p),
            _ => {
                return Err(FernError::EvalError {
                    loc: loc.clone(),
                    message: format!("expected 3D point (list x y z), got: {}", item.type_name()),
                });
            }
        }
    }
    Ok(points)
}

// === Sweep & Loft ===

/// `(sweep :profile poly :path path :segments s)` — sweep profile along path
fn builtin_sweep(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let profile = require_kwarg(&kwargs, "profile", "sweep", loc)?;
    let path_val = require_kwarg(&kwargs, "path", "sweep", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);

    let points = extract_polygon_points(profile, loc)?;
    let path = match path_val {
        Value::Path(p) => Arc::clone(p),
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: format!(
                    "`sweep` requires a path for `:path`, got: {}. Use (helix ...), (arc ...), or (bezier ...) to create a path.",
                    path_val.type_name()
                ),
            });
        }
    };

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Sweep {
            profile: points,
            path,
            segments,
        },
    ))))
}

/// `(loft :profiles (p1 p2 ...) :at (z1 z2 ...) :segments s)` — loft between profiles
fn builtin_loft(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let profiles_val = require_kwarg(&kwargs, "profiles", "loft", loc)?;
    let at_val = require_kwarg(&kwargs, "at", "loft", loc)?;
    let segments = get_kwarg_f64(&kwargs, "segments", loc)?
        .map(|s| s as u32)
        .unwrap_or(DEFAULT_SEGMENTS);

    // Extract list of profiles
    let profile_list = match profiles_val {
        Value::List(items) => items,
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "`loft` requires a list of profiles for `:profiles`".to_string(),
            });
        }
    };

    let mut profiles = Vec::new();
    for p in profile_list {
        profiles.push(extract_polygon_points(p, loc)?);
    }

    // Extract Z positions
    let positions = match at_val {
        Value::List(items) => items
            .iter()
            .map(|v| {
                v.as_number().ok_or_else(|| FernError::EvalError {
                    loc: loc.clone(),
                    message: "`loft` `:at` values must be numbers".to_string(),
                })
            })
            .collect::<FernResult<Vec<f64>>>()?,
        _ => {
            return Err(FernError::EvalError {
                loc: loc.clone(),
                message: "`loft` requires a list of positions for `:at`".to_string(),
            });
        }
    };

    if profiles.len() != positions.len() {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: format!(
                "`loft` requires same number of profiles ({}) and positions ({})",
                profiles.len(),
                positions.len()
            ),
        });
    }
    if profiles.len() < 2 {
        return Err(FernError::EvalError {
            loc: loc.clone(),
            message: "`loft` requires at least 2 profiles".to_string(),
        });
    }

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Loft {
            profiles,
            positions,
            segments,
        },
    ))))
}

/// Get a required keyword argument (any type)
fn require_kwarg<'a>(
    kwargs: &'a HashMap<String, Value>,
    key: &str,
    func_name: &str,
    loc: &SourceLocation,
) -> FernResult<&'a Value> {
    kwargs.get(key).ok_or_else(|| FernError::EvalError {
        loc: loc.clone(),
        message: format!("`{func_name}` requires `:{key}` argument"),
    })
}

// === Edge operations ===

/// `(chamfer :shape s :distance d)` — apply chamfer to all edges
fn builtin_chamfer(args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    let (_, kwargs) = split_kwargs(args);
    let shape = kwargs
        .get("shape")
        .ok_or_else(|| FernError::EvalError {
            loc: loc.clone(),
            message: "`chamfer` requires keyword argument `:shape`".to_string(),
        })
        .and_then(|v| get_shape_arg(v, loc))?;
    let distance = require_kwarg_f64(&kwargs, "distance", "chamfer", loc)?;

    Ok(Value::Shape(Arc::new(TrackedShape::untracked(
        ShapeNode::Chamfer { shape, distance },
    ))))
}

/// `(fillet ...)` — not yet supported
fn builtin_fillet(_args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    Err(FernError::EvalError {
        loc: loc.clone(),
        message: "fillet is not yet supported (truck 0.6 limitation). \
                  Consider using chamfer as an alternative."
            .to_string(),
    })
}

/// `(shell ...)` — not yet supported
fn builtin_shell(_args: &[Value], loc: &SourceLocation) -> FernResult<Value> {
    Err(FernError::EvalError {
        loc: loc.clone(),
        message: "shell (hollowing) is not yet supported (truck 0.6 limitation)".to_string(),
    })
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
                assert!(message.contains("zero"));
            }
            other => panic!("unexpected error type: {other:?}"),
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
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn test_pi() {
        let result = eval("pi");
        match result {
            Value::Float(v) => assert!((v - std::f64::consts::PI).abs() < 1e-10),
            other => panic!("expected Float, got {other:?}"),
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
            Value::Shape(tracked) => {
                assert_eq!(
                    *tracked.node,
                    ShapeNode::Box {
                        width: 10.0,
                        depth: 20.0,
                        height: 30.0,
                    }
                );
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn test_sphere_shape() {
        let result = eval("(sphere :radius 5.0)");
        match result {
            Value::Shape(tracked) => match &*tracked.node {
                ShapeNode::Sphere { radius, .. } => {
                    assert!((radius - 5.0).abs() < 1e-10);
                }
                other => panic!("expected Sphere, got {other:?}"),
            },
            other => panic!("expected Shape, got {other:?}"),
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
            Value::Shape(tracked) => match &*tracked.node {
                ShapeNode::Translate { offset, .. } => {
                    assert_eq!(*offset, [5.0, 0.0, 0.0]);
                }
                other => panic!("expected Translate, got {other:?}"),
            },
            other => panic!("expected Shape, got {other:?}"),
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
            Value::Shape(tracked) => {
                assert_eq!(
                    *tracked.node,
                    ShapeNode::Box {
                        width: 10.0,
                        depth: 10.0,
                        height: 10.0,
                    }
                );
            }
            other => panic!("expected Shape, got {other:?}"),
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
            Value::Shape(tracked) => {
                assert_eq!(
                    *tracked.node,
                    ShapeNode::Box {
                        width: 20.0,
                        depth: 20.0,
                        height: 20.0,
                    }
                );
            }
            other => panic!("expected Shape, got {other:?}"),
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
            Value::Shape(tracked) => {
                assert_eq!(
                    *tracked.node,
                    ShapeNode::Box {
                        width: 10.0,
                        depth: 10.0,
                        height: 10.0,
                    }
                );
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn test_mvp_completion_criteria() {
        // Phase 1 MVP completion criteria code
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
            Value::Shape(tracked) => match &*tracked.node {
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
                other => panic!("expected Difference, got {other:?}"),
            },
            other => panic!("expected Shape, got {other:?}"),
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
        // Passing a number to length type -> OK
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
        // Passing a string to length type -> type error
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
        // Passing a keyword to keyword type -> OK
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
            other => panic!("expected FaceRef, got {other:?}"),
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
            other => panic!("expected AxisRef, got {other:?}"),
        }
    }

    #[test]
    fn test_assembly_basic() {
        let result = eval(
            r#"
            (assembly "test"
              (place :part (box :width 10 :depth 10 :height 5) :as :plate)
              (place :part (cylinder :radius 2 :height 15) :as :pin))
            "#,
        );
        match result {
            Value::Assembly(asm) => {
                assert_eq!(asm.name, "test");
                assert_eq!(asm.parts.len(), 2);
                assert_eq!(asm.parts[0].name, "plate");
                assert_eq!(asm.parts[1].name, "pin");
            }
            other => panic!("expected Assembly, got {other:?}"),
        }
    }

    #[test]
    fn test_assembly_with_constraints() {
        let result = eval(
            r#"
            (assembly "constrained"
              (place :part (box :width 10 :depth 10 :height 5) :as :base)
              (place :part (cylinder :radius 2 :height 10) :as :pin)
              (mate (face :pin :bottom) (face :base :top))
              (align-axis (axis :pin :center) (axis :base :hole)))
            "#,
        );
        match result {
            Value::Assembly(asm) => {
                assert_eq!(asm.parts.len(), 2);
                assert_eq!(asm.constraints.len(), 2);
            }
            other => panic!("expected Assembly, got {other:?}"),
        }
    }

    #[test]
    fn test_assembly_with_position() {
        let result = eval(
            r#"
            (assembly "positioned"
              (place :part (box :width 10 :depth 10 :height 5) :as :plate)
              (place :part (sphere :radius 3) :as :ball :at #p(0 0 10)))
            "#,
        );
        match result {
            Value::Assembly(asm) => {
                assert_eq!(asm.parts.len(), 2);
                // ball's transform should have Z=10 offset
                let ball = &asm.parts[1];
                assert!((ball.transform[14] - 10.0).abs() < 1e-10);
            }
            other => panic!("expected Assembly, got {other:?}"),
        }
    }

    #[test]
    fn test_require_m3_bolt() {
        let result = eval(
            r#"
            (require :ferncad-std/m3-bolt)
            (m3-bolt :length 20)
            "#,
        );
        assert!(
            matches!(result, Value::Shape(_)),
            "require + m3-bolt should return Shape: {result:?}"
        );
    }

    #[test]
    fn test_require_spur_gear() {
        let result = eval(
            r#"
            (require :ferncad-std/spur-gear)
            (spur-gear :module 2 :teeth 20 :face-width 10)
            "#,
        );
        assert!(
            matches!(result, Value::Shape(_)),
            "spur-gear should return Shape: {result:?}"
        );
    }

    #[test]
    fn test_require_bevel_gear() {
        let result = eval(
            r#"
            (require :ferncad-std/bevel-gear)
            (bevel-gear :module 2 :teeth 20 :face-width 10)
            "#,
        );
        assert!(
            matches!(result, Value::Shape(_)),
            "bevel-gear should return Shape: {result:?}"
        );
    }

    #[test]
    fn test_require_unknown_module() {
        let err = eval_err("(require :nonexistent-module)");
        match err {
            FernError::EvalError { message, .. } => {
                assert!(message.contains("not found"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // === Macro tests ===

    #[test]
    fn test_quasiquote_basic() {
        // `(a b c) should return (a b c) like quote
        assert_eq!(
            eval("`(a b c)"),
            Value::List(vec![
                Value::Symbol("a".to_string()),
                Value::Symbol("b".to_string()),
                Value::Symbol("c".to_string()),
            ])
        );
    }

    #[test]
    fn test_quasiquote_unquote() {
        // `(a ,(+ 1 2) c) should return (a 3.0 c)
        let result = eval("`(a ,(+ 1 2) c)");
        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Symbol("a".to_string()));
                assert_eq!(items[1], Value::Float(3.0));
                assert_eq!(items[2], Value::Symbol("c".to_string()));
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn test_quasiquote_splice() {
        // `(a ,@(list 1 2 3) b) should return (a 1 2 3 b)
        let result = eval("`(a ,@(list 1 2 3) b)");
        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 5);
                assert_eq!(items[0], Value::Symbol("a".to_string()));
                assert_eq!(items[1], Value::Int(1));
                assert_eq!(items[2], Value::Int(2));
                assert_eq!(items[3], Value::Int(3));
                assert_eq!(items[4], Value::Symbol("b".to_string()));
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn test_defmacro_simple() {
        // Define a macro that wraps its argument in a list call
        let result = eval(
            r#"
            (defmacro wrap (x)
              `(list ,x))
            (wrap 42)
        "#,
        );
        assert_eq!(result, Value::List(vec![Value::Int(42)]));
    }

    #[test]
    fn test_defmacro_with_splice() {
        // Define a macro that creates a sum
        let result = eval(
            r#"
            (defmacro add-all (&rest args)
              `(+ ,@args))
            (add-all 1 2 3 4)
        "#,
        );
        assert_eq!(result, Value::Float(10.0));
    }

    #[test]
    fn test_defmacro_when() {
        // Classic "when" macro: (when condition body...) → (if condition (progn body...))
        let result = eval(
            r#"
            (defmacro when (condition &rest body)
              `(if ,condition (progn ,@body)))
            (when t (+ 1 2))
        "#,
        );
        assert_eq!(result, Value::Float(3.0));
    }

    #[test]
    fn test_quasiquote_nested_unquote() {
        let result = eval(
            r#"
            (defvar x 10)
            (defvar y 20)
            `(+ ,x ,y)
        "#,
        );
        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Symbol("+".to_string()));
                assert_eq!(items[1], Value::Int(10));
                assert_eq!(items[2], Value::Int(20));
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn test_backquote_with_variable() {
        let result = eval("(defvar b 42) `(a ,b)");
        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], Value::Symbol("a".to_string()));
                assert_eq!(items[1], Value::Int(42));
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    // === Circle, Path, Sweep, Loft tests ===

    #[test]
    fn test_circle_builtin() {
        let result = eval("(circle :radius 5 :segments 8)");
        match result {
            Value::List(items) => {
                assert_eq!(items.len(), 8);
                // First point should be (5, 0)
                if let Value::List(coords) = &items[0] {
                    assert!((coords[0].as_number().unwrap() - 5.0).abs() < 1e-10);
                    assert!((coords[1].as_number().unwrap()).abs() < 1e-10);
                }
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn test_helix_builtin() {
        let result = eval("(helix :radius 1 :pitch 1 :turns 1)");
        assert!(matches!(result, Value::Path(_)));
    }

    #[test]
    fn test_arc_builtin() {
        let result = eval("(arc :radius 5 :angle pi)");
        assert!(matches!(result, Value::Path(_)));
    }

    #[test]
    fn test_bezier_builtin() {
        let result = eval("(bezier :points (list (list 0 0 0) (list 5 3 0) (list 10 0 0)))");
        assert!(matches!(result, Value::Path(_)));
    }

    #[test]
    fn test_sweep_basic() {
        let result = eval(
            "(sweep :profile (circle :radius 1 :segments 8) \
                    :path (helix :radius 5 :pitch 2 :turns 1) \
                    :segments 16)",
        );
        assert!(matches!(result, Value::Shape(_)));
    }

    #[test]
    fn test_loft_basic() {
        let result = eval(
            "(loft :profiles (list (circle :radius 5 :segments 8) \
                                   (circle :radius 3 :segments 8)) \
                   :at (list 0 10))",
        );
        assert!(matches!(result, Value::Shape(_)));
    }

    #[test]
    fn test_sweep_type_error() {
        let mut evaluator = Evaluator::new();
        let result = evaluator
            .eval_source("(sweep :profile (circle :radius 1 :segments 4) :path (box :width 1 :depth 1 :height 1))");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("path"), "error should mention 'path': {err}");
    }

    #[test]
    fn test_shape_has_source_span() {
        let source = "(box :width 10 :depth 10 :height 10)";
        let result = eval(source);
        match result {
            Value::Shape(tracked) => {
                assert_eq!(tracked.span.start, 0);
                assert_eq!(tracked.span.end, source.len());
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn test_shape_span_with_preceding_defvar() {
        let source = "(defvar x 10) (box :width x :depth x :height x)";
        let result = eval(source);
        match result {
            Value::Shape(tracked) => {
                // Span should cover the box expression, not the defvar
                assert_eq!(
                    &source[tracked.span.start..tracked.span.end],
                    "(box :width x :depth x :height x)"
                );
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn test_nested_shape_gets_outer_span() {
        let source = "(difference (box :width 20 :depth 20 :height 20) (sphere :radius 12))";
        let result = eval(source);
        match result {
            Value::Shape(tracked) => {
                // Span covers the entire difference expression
                assert_eq!(tracked.span.start, 0);
                assert_eq!(tracked.span.end, source.len());
            }
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn test_assembly_parts_have_spans() {
        let source = r#"(assembly "test"
  (place :part (box :width 10 :depth 10 :height 5) :as :base)
  (place :part (cylinder :radius 2 :height 10) :as :pin))"#;
        let result = eval(source);
        match result {
            Value::Assembly(assembly) => {
                assert_eq!(assembly.parts.len(), 2);
                let span0 = &assembly.parts[0]
                    .shape
                    .as_ref()
                    .expect("base should have shape")
                    .span;
                let span1 = &assembly.parts[1]
                    .shape
                    .as_ref()
                    .expect("pin should have shape")
                    .span;
                // Each part should have a distinct, non-dummy span
                assert!(span0.end > span0.start, "base span should be non-empty");
                assert!(span1.end > span1.start, "pin span should be non-empty");
                // Spans should be different (not the whole assembly)
                assert_ne!(
                    span0.start, span1.start,
                    "parts should have different span starts"
                );
                // Spans should point to the (place ...) expressions
                let base_text = &source[span0.start..span0.end];
                let pin_text = &source[span1.start..span1.end];
                assert!(
                    base_text.starts_with("(place"),
                    "base span should cover (place ...), got: {base_text}"
                );
                assert!(
                    pin_text.starts_with("(place"),
                    "pin span should cover (place ...), got: {pin_text}"
                );
            }
            other => panic!("expected Assembly, got {other:?}"),
        }
    }

    // === New built-in tests ===

    // --- Math functions ---

    #[test]
    fn test_tan() {
        let result = eval("(tan 0)");
        assert_eq!(result, Value::Float(0.0));
    }

    #[test]
    fn test_acos() {
        let result = eval("(acos 1)");
        match result {
            Value::Float(v) => assert!(v.abs() < 1e-10),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn test_atan() {
        let result = eval("(atan 0)");
        assert_eq!(result, Value::Float(0.0));
    }

    #[test]
    fn test_atan2() {
        let result = eval("(atan2 1 1)");
        match result {
            Value::Float(v) => assert!((v - std::f64::consts::FRAC_PI_4).abs() < 1e-10),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn test_abs() {
        assert_eq!(eval("(abs -5)"), Value::Float(5.0));
        assert_eq!(eval("(abs 3)"), Value::Float(3.0));
    }

    #[test]
    fn test_mod() {
        assert_eq!(eval("(mod 10 3)"), Value::Float(1.0));
    }

    #[test]
    fn test_expt() {
        assert_eq!(eval("(expt 2 10)"), Value::Float(1024.0));
    }

    #[test]
    fn test_floor_ceil() {
        assert_eq!(eval("(floor 3.7)"), Value::Float(3.0));
        assert_eq!(eval("(ceil 3.2)"), Value::Float(4.0));
    }

    #[test]
    fn test_min_max() {
        assert_eq!(eval("(min 3 1 2)"), Value::Float(1.0));
        assert_eq!(eval("(max 3 1 2)"), Value::Float(3.0));
    }

    // --- List functions ---

    #[test]
    fn test_cons() {
        assert_eq!(
            eval("(cons 1 (list 2 3))"),
            Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        );
    }

    #[test]
    fn test_car_cdr() {
        assert_eq!(eval("(car (list 1 2 3))"), Value::Int(1));
        assert_eq!(
            eval("(cdr (list 1 2 3))"),
            Value::List(vec![Value::Int(2), Value::Int(3)])
        );
        assert_eq!(eval("(car nil)"), Value::Nil);
        assert_eq!(eval("(cdr (list 1))"), Value::Nil);
    }

    #[test]
    fn test_append() {
        assert_eq!(
            eval("(append (list 1 2) (list 3 4))"),
            Value::List(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4)
            ])
        );
    }

    #[test]
    fn test_nth() {
        assert_eq!(eval("(nth 0 (list 10 20 30))"), Value::Int(10));
        assert_eq!(eval("(nth 2 (list 10 20 30))"), Value::Int(30));
        assert_eq!(eval("(nth 5 (list 10 20 30))"), Value::Nil);
    }

    #[test]
    fn test_length() {
        assert_eq!(eval("(length (list 1 2 3))"), Value::Int(3));
        assert_eq!(eval("(length nil)"), Value::Int(0));
    }

    #[test]
    fn test_reverse() {
        assert_eq!(
            eval("(reverse (list 1 2 3))"),
            Value::List(vec![Value::Int(3), Value::Int(2), Value::Int(1)])
        );
    }

    #[test]
    fn test_last() {
        assert_eq!(eval("(last (list 1 2 3))"), Value::Int(3));
        assert_eq!(eval("(last nil)"), Value::Nil);
    }

    // --- Special forms ---

    #[test]
    fn test_when() {
        assert_eq!(eval("(when t 42)"), Value::Float(42.0));
        assert_eq!(eval("(when nil 42)"), Value::Nil);
    }

    #[test]
    fn test_unless() {
        assert_eq!(eval("(unless nil 42)"), Value::Float(42.0));
        assert_eq!(eval("(unless t 42)"), Value::Nil);
    }

    #[test]
    fn test_and() {
        assert_eq!(eval("(and 1 2 3)"), Value::Float(3.0));
        assert_eq!(eval("(and 1 nil 3)"), Value::Nil);
        assert_eq!(eval("(and)"), Value::Bool(true));
    }

    #[test]
    fn test_or() {
        assert_eq!(eval("(or nil nil 3)"), Value::Float(3.0));
        assert_eq!(eval("(or nil nil)"), Value::Nil);
        assert_eq!(eval("(or 1 2)"), Value::Float(1.0));
    }

    #[test]
    fn test_setf() {
        assert_eq!(eval("(defvar x 1) (setf x 42) x"), Value::Float(42.0));
    }

    #[test]
    fn test_setf_undefined() {
        let err = eval_err("(setf nonexistent 42)");
        assert!(matches!(err, FernError::UndefinedVariable { .. }));
    }

    #[test]
    fn test_dotimes() {
        // Sum 0..4 using dotimes + setf
        assert_eq!(
            eval("(defvar sum 0) (dotimes (i 5) (setf sum (+ sum i))) sum"),
            Value::Float(10.0) // 0+1+2+3+4
        );
    }

    #[test]
    fn test_dolist() {
        assert_eq!(
            eval("(defvar sum 0) (dolist (x (list 10 20 30)) (setf sum (+ sum x))) sum"),
            Value::Float(60.0)
        );
    }

    #[test]
    fn test_mapcar() {
        assert_eq!(
            eval("(mapcar (lambda (x) (* x 2)) (list 1 2 3))"),
            Value::List(vec![
                Value::Float(2.0),
                Value::Float(4.0),
                Value::Float(6.0)
            ])
        );
    }

    #[test]
    fn test_reduce() {
        assert_eq!(
            eval("(reduce (lambda (a b) (+ a b)) (list 1 2 3 4))"),
            Value::Float(10.0)
        );
        // With initial value
        assert_eq!(
            eval("(reduce (lambda (a b) (+ a b)) (list 1 2 3) 10)"),
            Value::Float(16.0)
        );
    }

    #[test]
    fn test_remove_if() {
        assert_eq!(
            eval("(remove-if (lambda (x) (> x 2)) (list 1 2 3 4))"),
            Value::List(vec![Value::Int(1), Value::Int(2)])
        );
    }

    #[test]
    fn test_apply() {
        assert_eq!(eval("(apply + (list 1 2 3))"), Value::Float(6.0));
    }

    // --- Type predicates ---

    #[test]
    fn test_numberp() {
        assert_eq!(eval("(numberp 42)"), Value::Bool(true));
        assert_eq!(eval("(numberp \"hello\")"), Value::Bool(false));
    }

    #[test]
    fn test_listp() {
        assert_eq!(eval("(listp (list 1 2))"), Value::Bool(true));
        assert_eq!(eval("(listp 42)"), Value::Bool(false));
    }

    #[test]
    fn test_nilp() {
        assert_eq!(eval("(nilp nil)"), Value::Bool(true));
        assert_eq!(eval("(nilp 42)"), Value::Bool(false));
    }

    #[test]
    fn test_stringp() {
        assert_eq!(eval("(stringp \"hello\")"), Value::Bool(true));
        assert_eq!(eval("(stringp 42)"), Value::Bool(false));
    }

    // --- Format ---

    #[test]
    fn test_format() {
        assert_eq!(
            eval(r#"(format "Hello ~a, you are ~d" "world" 42)"#),
            Value::Str("Hello world, you are 42".to_string())
        );
    }

    // --- Integration: build a list with dotimes ---

    #[test]
    fn test_dotimes_build_list() {
        // Build polygon-like point list using dotimes
        assert_eq!(
            eval(
                r#"
                (defvar pts (list))
                (dotimes (i 4)
                  (setf pts (append pts (list (list (cos (* i 1.5707963)) (sin (* i 1.5707963)))))))
                (length pts)
            "#
            ),
            Value::Int(4)
        );
    }

    // --- Integration: mapcar + reduce for functional style ---

    #[test]
    fn test_functional_pipeline() {
        // Generate list of squares, filter, sum
        assert_eq!(
            eval(
                r#"
                (reduce
                  (lambda (a b) (+ a b))
                  (remove-if
                    (lambda (x) (> x 10))
                    (mapcar (lambda (x) (* x x)) (list 1 2 3 4 5)))
                  0)
            "#
            ),
            Value::Float(14.0) // 1+4+9 = 14 (16 and 25 filtered out)
        );
    }

    #[test]
    fn test_shape_arc_sharing() {
        // Verify that shapes stored in variables share Arc pointers
        let result = eval(
            r#"
            (defvar my-box (box :width 10 :depth 10 :height 10))
            (union my-box (translate :shape my-box :by #v(20 0 0)))
            "#,
        );
        match result {
            Value::Shape(tracked) => match &*tracked.node {
                ShapeNode::Union { children } => {
                    assert_eq!(children.len(), 2);
                    // Both children should reference the same box via Arc
                    if let ShapeNode::Translate { shape, .. } = &*children[1] {
                        assert!(
                            Arc::ptr_eq(&children[0], shape),
                            "shapes from same variable should share Arc pointer"
                        );
                    } else {
                        panic!("expected Translate as second child");
                    }
                }
                other => panic!("expected Union, got {other:?}"),
            },
            other => panic!("expected Shape, got {other:?}"),
        }
    }

    #[test]
    fn test_mapcar_no_extra_clone() {
        // mapcar should work correctly without cloning the entire list
        assert_eq!(
            eval("(mapcar (lambda (x) (* x 2)) (list 1 2 3))"),
            Value::List(vec![
                Value::Float(2.0),
                Value::Float(4.0),
                Value::Float(6.0)
            ])
        );
    }

    #[test]
    fn test_dolist_no_extra_clone() {
        // dolist should work with borrowed list and return last body expr
        assert_eq!(
            eval(
                r#"
                (dolist (x (list 10 20 30))
                  (* x 2))
                "#
            ),
            Value::Float(60.0)
        );
    }
}
