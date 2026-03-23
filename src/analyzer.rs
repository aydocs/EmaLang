use crate::ast::{Program, Stmt, Expr, BinaryOp, Type, Span, EmbeddedKind, EmaType};
use crate::css_parser::parse_css;
use crate::html_parser::parse_html;
use crate::js_parser::parse_js;
use crate::php_parser::parse_php;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Diagnostic {
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub severity: String, // "error" | "warning"
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolInfo {
    pub name: String,
    pub ema_type: EmaType,
    pub span: Span,
}

pub struct Analyzer {
    scopes: Vec<HashMap<String, EmaType>>,
    models: HashMap<String, Vec<(String, EmaType)>>,
    pub diagnostics: Vec<Diagnostic>,
    pub symbols: Vec<SymbolInfo>,
    exec_ctx: Vec<ExecContext>,
    strict_embedded: bool,
    is_strict: bool,
    expected_return: Vec<EmaType>,
    is_in_async: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecContext {
    Any,
    Server,
    Client,
}

impl Analyzer {
    pub fn new(strict_embedded: bool) -> Self {
        Analyzer {
            scopes: vec![HashMap::new()],
            models: HashMap::new(),
            diagnostics: Vec::new(),
            symbols: Vec::new(),
            exec_ctx: vec![ExecContext::Any],
            strict_embedded,
            is_strict: false,
            expected_return: Vec::new(),
            is_in_async: false,
        }
    }

    fn record_symbol(&mut self, name: String, ema_type: EmaType, span: Span) {
        eprintln!("[Analyzer] Recording symbol '{}' at {}:{}", name, span.line, span.col);
        self.symbols.push(SymbolInfo { name, ema_type, span });
    }

    fn ema_type_from_ast(&self, t: &Type) -> EmaType {
        match t {
            Type::Int => EmaType::Int,
            Type::Float => EmaType::Float,
            Type::Str => EmaType::Str,
            Type::Bool => EmaType::Bool,
            Type::Custom(name) => EmaType::Model { name: name.clone() },
        }
    }

    fn error(&mut self, span: Span, message: String) {
        self.diagnostics.push(Diagnostic {
            line: span.line,
            col: span.col,
            message,
            severity: "error".to_string(),
        });
    }

    fn warn(&mut self, span: Span, message: String) {
        self.diagnostics.push(Diagnostic {
            line: span.line,
            col: span.col,
            message,
            severity: "warning".to_string(),
        });
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn current_exec_ctx(&self) -> ExecContext {
        *self.exec_ctx.last().unwrap_or(&ExecContext::Any)
    }

    fn push_exec_ctx(&mut self, ctx: ExecContext) {
        self.exec_ctx.push(ctx);
    }

    fn pop_exec_ctx(&mut self) {
        self.exec_ctx.pop();
        if self.exec_ctx.is_empty() {
            self.exec_ctx.push(ExecContext::Any);
        }
    }

    fn define(&mut self, name: String, ema_type: EmaType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ema_type);
        }
    }

    fn resolve(&self, name: &str) -> Option<EmaType> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }

    pub fn analyze(&mut self, program: &Program) -> Vec<Diagnostic> {
        self.is_strict = program.is_strict;
        for stmt in &program.statements {
            self.analyze_stmt(stmt);
        }
        eprintln!("[Analyzer] Total symbols recorded: {}", self.symbols.len());
        self.diagnostics.clone()
    }

    fn analyze_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { name, type_ann, value, span } => {
                let val_type = self.analyze_expr(value);
                let declared_type = if let Some(t) = type_ann {
                    let dt = self.ema_type_from_ast(t);
                    if dt != val_type && val_type != EmaType::Void {
                        self.error(span.clone(), format!("Error: Type mismatch for variable '{}'. Declared: {:?}, Found: {:?}", name, dt, val_type));
                    }
                    dt
                } else if self.is_strict {
                    self.error(span.clone(), format!("Error: In strict mode, variable '{}' must have an explicit type annotation.", name));
                    val_type
                } else {
                    val_type
                };
                self.record_symbol(name.clone(), declared_type.clone(), span.clone());
                self.define(name.clone(), declared_type);
            }
            Stmt::StateDecl { name, type_ann, value, span } => {
                let val_type = self.analyze_expr(value);
                let declared_type = if let Some(t) = type_ann {
                    let dt = self.ema_type_from_ast(t);
                    if dt != val_type && val_type != EmaType::Void {
                        self.error(span.clone(), format!("Error: Type mismatch for state '{}'. Declared: {:?}, Found: {:?}", name, dt, val_type));
                    }
                    dt
                } else if self.is_strict {
                    self.error(span.clone(), format!("Error: In strict mode, state '{}' must have an explicit type annotation.", name));
                    val_type
                } else {
                    val_type
                };
                self.record_symbol(name.clone(), declared_type.clone(), span.clone());
                self.define(name.clone(), declared_type);
            }
            Stmt::AssignStmt { name, value, span } => {
                let declared_type = match self.resolve(name) {
                    Some(t) => t.clone(),
                    None => {
                        self.error(span.clone(), format!("Error: Assignment to undeclared variable '{}'", name));
                        EmaType::Int
                    }
                };
                let val_type = self.analyze_expr(value);
                if self.is_strict && val_type != declared_type && declared_type != EmaType::Void {
                    self.error(span.clone(), format!("Error: Type mismatch. Cannot assign '{:?}' to variable '{}' of type '{:?}'", val_type, name, declared_type));
                }
            }
            Stmt::PrintStmt(expr, _span) => {
                self.analyze_expr(expr);
            }
            Stmt::IfStmt { condition, then_branch, else_branch, span } => {
                let cond_type = self.analyze_expr(condition);
                if cond_type != EmaType::Bool && cond_type != EmaType::Int && cond_type != EmaType::Float {
                    self.error(span.clone(), format!("Error: 'if' condition must be bool, int, or float; found: {:?}", cond_type));
                }
                self.enter_scope();
                for s in then_branch { self.analyze_stmt(s); }
                self.exit_scope();
                if let Some(else_stmts) = else_branch {
                    self.enter_scope();
                    for s in else_stmts { self.analyze_stmt(s); }
                    self.exit_scope();
                }
            }
            Stmt::WhileStmt { condition, body, span } => {
                let cond_type = self.analyze_expr(condition);
                if cond_type != EmaType::Bool && cond_type != EmaType::Int && cond_type != EmaType::Float {
                    self.error(span.clone(), "Error: 'while' condition must be bool, int, or float.".to_string());
                }
                self.enter_scope();
                for s in body { self.analyze_stmt(s); }
                self.exit_scope();
            }
            Stmt::FnDecl { name, params, return_type, body, is_async, span: _span } => {
                let mut param_types = Vec::new();
                for (p_name, p_type_ann) in params {
                    let pt = if let Some(t) = p_type_ann {
                        self.ema_type_from_ast(t)
                    } else if self.is_strict {
                        self.error(_span.clone(), format!("Error: In strict mode, parameter '{}' of function '{}' must have a type annotation.", p_name, name));
                        EmaType::Int
                    } else {
                        EmaType::Int
                    };
                    param_types.push(pt);
                }

                let ret_t = if let Some(t) = return_type {
                    self.ema_type_from_ast(t)
                } else {
                    EmaType::Void
                };

                let fn_type = EmaType::Function { 
                    params: param_types.clone(), 
                    return_type: Box::new(ret_t.clone())
                };
                self.record_symbol(name.clone(), fn_type.clone(), _span.clone());
                self.define(name.clone(), fn_type);
                
                self.enter_scope();
                let prev_async = self.is_in_async;
                self.is_in_async = *is_async;
                self.expected_return.push(ret_t);
                for (i, (p_name, _)) in params.iter().enumerate() {
                    self.define(p_name.clone(), param_types[i].clone());
                }
                for s in body { self.analyze_stmt(s); }
                self.expected_return.pop();
                self.is_in_async = prev_async;
                self.exit_scope();
            }
            Stmt::ReturnStmt(expr_opt, span) => {
                let actual_t = if let Some(expr) = expr_opt {
                    self.analyze_expr(expr)
                } else {
                    EmaType::Void
                };
                
                if let Some(expected) = self.expected_return.last() {
                    if actual_t != *expected {
                        self.error(span.clone(), format!("Error: Return type mismatch. Expected: {:?}, Found: {:?}", expected, actual_t));
                    }
                }
            }
            Stmt::ComponentDecl { name, props, body, span: _span } => {
                let mut param_types = Vec::new();
                for (p_name, p_type_ann) in props {
                    let pt = self.ema_type_from_ast(p_type_ann);
                    param_types.push((p_name.clone(), pt));
                }

                let comp_type = EmaType::Component { 
                    name: name.clone(),
                    props: param_types.clone(), 
                };
                self.record_symbol(name.clone(), comp_type.clone(), _span.clone());
                self.define(name.clone(), comp_type);
                
                self.enter_scope();
                for (p_name, pt) in param_types {
                    self.define(p_name.clone(), pt.clone());
                }
                for s in body { self.analyze_stmt(s); }
                self.exit_scope();
            }
            Stmt::ModelDecl { name, fields, span: _span } => {
                let mut field_types = Vec::new();
                for field in fields {
                    let f_type = match &field.field_type {
                        Type::Int => EmaType::Int,
                        Type::Str => EmaType::Str,
                        Type::Float => EmaType::Float,
                        Type::Bool => EmaType::Bool,
                        Type::Custom(name) => EmaType::Model { name: name.clone() },
                    };
                    field_types.push((field.name.clone(), f_type));
                }
                self.models.insert(name.clone(), field_types);
                self.define(name.clone(), EmaType::Model { name: name.clone() });
            }
            Stmt::ServerBlock(stmts, _span) => {
                self.push_exec_ctx(ExecContext::Server);
                self.enter_scope();
                for s in stmts { self.analyze_stmt(s); }
                self.exit_scope();
                self.pop_exec_ctx();
            }
            Stmt::ClientBlock(stmts, _span) => {
                self.push_exec_ctx(ExecContext::Client);
                self.enter_scope();
                for s in stmts { self.analyze_stmt(s); }
                self.exit_scope();
                self.pop_exec_ctx();
            }
            Stmt::ExprStmt(expr, _span) => {
                self.analyze_expr(expr);
            }
            Stmt::ImportStmt { .. } => {}
            Stmt::Test { name: _, body, .. } => {
                self.enter_scope();
                for s in body { self.analyze_stmt(s); }
                self.exit_scope();
            }
        }
    }

    fn analyze_expr(&mut self, expr: &Expr) -> EmaType {
        match expr {
            Expr::Ternary { condition, then_expr, else_expr, .. } => {
                self.analyze_expr(condition);
                let t = self.analyze_expr(then_expr);
                self.analyze_expr(else_expr);
                t
            }
            Expr::IntLit(_, _) => EmaType::Int,
            Expr::FloatLit(_, _) => EmaType::Float,
            Expr::StringLit(_, _) => EmaType::Str,
            Expr::BoolLit(_, _) => EmaType::Bool,
            Expr::Identifier(name, span) => {
                let t = self.resolve(name).unwrap_or_else(|| {
                    self.error(span.clone(), format!("Error: Undefined variable '{}'", name));
                    EmaType::Void
                });
                self.record_symbol(name.clone(), t.clone(), span.clone());
                t
            }
            Expr::Binary { left, op, right, span } => {
                let left_t = self.analyze_expr(left);
                let right_t = self.analyze_expr(right);
                
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                        if left_t == EmaType::Int && right_t == EmaType::Int {
                            EmaType::Int
                        } else if op == &BinaryOp::Add && left_t == EmaType::Str && right_t == EmaType::Str {
                            EmaType::Str
                        } else {
                            self.error(span.clone(), format!("Error: Invalid arithmetic operation: {:?} and {:?}", left_t, right_t));
                            EmaType::Void
                        }
                    }
                    BinaryOp::EqEq | BinaryOp::BangEq | BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                        if left_t == right_t {
                            EmaType::Bool
                        } else {
                            self.error(span.clone(), format!("Error: Cannot compare: {:?} and {:?}", left_t, right_t));
                            EmaType::Bool
                        }
                    }
                }
            }
            Expr::Call { callee, args, span } => {
                // Built-in client helper: set(stateVar, value)
                if let Expr::Identifier(name, _) = callee.as_ref() {
                    if name == "set" {
                        if args.len() != 2 {
                            self.error(span.clone(), "Error: set(x, value) expects 2 arguments.".to_string());
                            return EmaType::Void;
                        }
                        // Validate args but treat as intrinsic
                        self.analyze_expr(&args[0]);
                        self.analyze_expr(&args[1]);
                        return EmaType::Void;
                    }
                    if name == "inc" || name == "dec" || name == "toggle" {
                        if args.len() != 1 {
                            self.error(span.clone(), format!("Error: {}(x) expects 1 argument.", name));
                            return EmaType::Void;
                        }
                        self.analyze_expr(&args[0]);
                        return EmaType::Void;
                    }
                }
                let callee_t = self.analyze_expr(callee);
                if let EmaType::Function { params, return_type } = callee_t {
                    if args.len() != params.len() {
                        self.error(span.clone(), format!("Error: Function expects {} arguments, {} provided", params.len(), args.len()));
                    }
                    for (i, arg) in args.iter().enumerate() {
                        let arg_t = self.analyze_expr(arg);
                        if i < params.len() && arg_t != params[i] {
                            self.error(span.clone(), format!("Error: Argument {} type mismatch. Expected: {:?}, Found: {:?}", i, params[i], arg_t));
                        }
                    }
                    *return_type
                } else {
                    self.error(span.clone(), "Error: Callee is not a function.".to_string());
                    EmaType::Void
                }
            }
            Expr::NamespaceCall { namespace, method, args: _args, span: _span } => {
                // Built-in OS, DB, and Network calls analysis
                match namespace.as_str() {
                    "std::fs" | "std::file" => {
                        match method.as_str() {
                            "read" => EmaType::Str,
                            "write" | "append" | "exists" | "delete" => EmaType::Bool,
                            _ => EmaType::Void,
                        }
                    }
                    "std::net" => {
                        match method.as_str() {
                            "fetch" => EmaType::Str,
                            _ => EmaType::Void,
                        }
                    }
                    "std::crypto" => {
                        match method.as_str() {
                            "sha256" | "uuid" => EmaType::Str,
                            _ => EmaType::Void,
                        }
                    }
                    "std::db" => {
                        match method.as_str() {
                            "migrate" => EmaType::Void,
                            _ => EmaType::Void,
                        }
                    }
                    "std::http" => {
                        // All methods return Void in this mock phase
                        // Validate specific methods if needed, but return Void for now
                        match method.as_str() {
                            "route" | "serve" => EmaType::Void, // Specific methods validated
                            _ => EmaType::Void, // Other http methods also return Void
                        }
                    }
                    _ => {
                        if let Some(_) = self.models.get(namespace) {
                            if method == "insert" {
                                return EmaType::Bool;
                            }
                        }
                        EmaType::Void
                    }
                }
            }
            Expr::UiElement { tag, props, children, span } => {
                if let Some(first_char) = tag.chars().next() {
                    if first_char.is_uppercase() {
                        let mut found_comp = false;
                        if let Some(EmaType::Component { props: comp_props, .. }) = self.resolve(tag) {
                            found_comp = true;
                            for (p_name, p_type) in comp_props {
                                if let Some(expr) = props.get(&p_name) {
                                    let arg_type = self.analyze_expr(expr);
                                    if self.is_strict && arg_type != p_type && p_type != EmaType::Void {
                                        self.error(span.clone(), format!("Error: Component '{}' expected prop '{}' of type {:?}, found {:?}", tag, p_name, p_type, arg_type));
                                    }
                                } else {
                                    self.error(span.clone(), format!("Error: Component '{}' missing required prop '{}'", tag, p_name));
                                }
                            }
                        }
                        if self.is_strict && !found_comp {
                            self.error(span.clone(), format!("Error: Undefined component '{}'", tag));
                        }
                    }
                }
                for prop_val in props.values() {
                    self.analyze_expr(prop_val);
                }
                for child in children {
                    self.analyze_expr(child);
                }
                EmaType::Ui
            }
            Expr::EmbeddedBlock { kind, raw, span: _span } => {
                let ctx = self.current_exec_ctx();
                match (ctx, kind) {
                    (ExecContext::Client, EmbeddedKind::Php) => {
                        self.error(_span.clone(), "Error: `php { ... }` can only be used inside @server.".to_string());
                        EmaType::Void
                    }
                    (ExecContext::Server, EmbeddedKind::Html | EmbeddedKind::Css | EmbeddedKind::Js) => {
                        self.error(_span.clone(), "Error: `html/css/js { ... }` can only be used inside @client.".to_string());
                        EmaType::Void
                    }
                    (_, EmbeddedKind::Html) => {
                        // Produce stable diagnostics for raw blocks too: attempt parsing.
                        if let Err(e) = parse_html(raw, _span.clone()) {
                            let msg = format!("Error parsing html block: {}", e);
                            if self.strict_embedded { self.error(_span.clone(), msg); } else { self.warn(_span.clone(), msg); }
                        }
                        EmaType::Ui
                    }
                    (_, EmbeddedKind::Css) => {
                        if let Err(e) = parse_css(raw, _span.clone()) {
                            let msg = format!("Error parsing css block: {}", e);
                            if self.strict_embedded { self.error(_span.clone(), msg); } else { self.warn(_span.clone(), msg); }
                        }
                        EmaType::Ui
                    }
                    (_, EmbeddedKind::Js) => {
                        if let Err(e) = parse_js(raw, _span.clone()) {
                            let msg = format!("Error parsing js block: {}", e);
                            if self.strict_embedded { self.error(_span.clone(), msg); } else { self.warn(_span.clone(), msg); }
                        }
                        EmaType::Ui
                    }
                    (_, EmbeddedKind::Php) => {
                        if let Err(e) = parse_php(raw, _span.clone()) {
                            let msg = format!("Error parsing php block: {}", e);
                            if self.strict_embedded { self.error(_span.clone(), msg); } else { self.warn(_span.clone(), msg); }
                        }
                        EmaType::Void
                    }
                }
            }
            Expr::HtmlAst { span, .. } | Expr::CssAst { span, .. } | Expr::JsAst { span, .. } => {
                let ctx = self.current_exec_ctx();
                if ctx == ExecContext::Server {
                    self.error(span.clone(), "Error: html/css/js AST nodes can only be used inside @client.".to_string());
                    return EmaType::Void;
                }
                EmaType::Ui
            }
            Expr::PhpAst { span, .. } => {
                let ctx = self.current_exec_ctx();
                if ctx == ExecContext::Client {
                    self.error(span.clone(), "Error: php AST nodes can only be used inside @server.".to_string());
                }
                EmaType::Void
            }
            Expr::Interpolation(inner, _span) => {
                self.analyze_expr(inner)
            }
            Expr::Member { object, .. } => {
                self.analyze_expr(object);
                EmaType::Void
            }
            Expr::StructLiteral { fields, .. } => {
                for (_, val) in fields {
                    self.analyze_expr(val);
                }
                EmaType::Json
            }
            Expr::Await(inner, span) => {
                self.analyze_expr(inner)
            }
            Expr::ClientScript(stmts, _span) => {
                self.push_exec_ctx(ExecContext::Client);
                self.enter_scope();
                for s in stmts { self.analyze_stmt(s); }
                self.exit_scope();
                self.pop_exec_ctx();
                EmaType::Void
            }
            Expr::ServerScript(stmts, _span) => {
                self.push_exec_ctx(ExecContext::Server);
                self.enter_scope();
                for s in stmts { self.analyze_stmt(s); }
                self.exit_scope();
                self.pop_exec_ctx();
                EmaType::Void
            }
        }
    }
}
