use crate::ast::{Program, Stmt, Expr, BinaryOp, Type, Span, EmbeddedKind};
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

#[derive(Debug, Clone, PartialEq)]
pub enum EmaType {
    Int,
    Float,
    Str,
    Bool,
    Void,
    Function { params: Vec<EmaType>, return_type: Box<EmaType> },
    Model { name: String },
    Ui,
}

pub struct Analyzer {
    scopes: Vec<HashMap<String, EmaType>>,
    models: HashMap<String, Vec<(String, EmaType)>>,
    pub diagnostics: Vec<Diagnostic>,
    exec_ctx: Vec<ExecContext>,
    strict_embedded: bool,
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
            exec_ctx: vec![ExecContext::Any],
            strict_embedded,
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
        for stmt in &program.statements {
            self.analyze_stmt(stmt);
        }
        self.diagnostics.clone()
    }

    fn analyze_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { name, value, span: _ } => {
                let val_type = self.analyze_expr(value);
                self.define(name.clone(), val_type);
            }
            Stmt::StateDecl { name, value, span: _ } => {
                let val_type = self.analyze_expr(value);
                self.define(name.clone(), val_type);
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
            Stmt::FnDecl { name, params, body, span: _span } => {
                // Simplified: all params are Int for now
                let param_types = vec![EmaType::Int; params.len()];
                let fn_type = EmaType::Function { 
                    params: param_types.clone(), 
                    return_type: Box::new(EmaType::Void) // Simplified return
                };
                self.define(name.clone(), fn_type);
                
                self.enter_scope();
                for (i, param) in params.iter().enumerate() {
                    self.define(param.clone(), param_types[i].clone());
                }
                for s in body { self.analyze_stmt(s); }
                self.exit_scope();
            }
            Stmt::ReturnStmt(expr_opt, _span) => {
                if let Some(expr) = expr_opt {
                    self.analyze_expr(expr);
                }
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
        }
    }

    fn analyze_expr(&mut self, expr: &Expr) -> EmaType {
        match expr {
            Expr::IntLit(_, _) => EmaType::Int,
            Expr::FloatLit(_, _) => EmaType::Float,
            Expr::StringLit(_, _) => EmaType::Str,
            Expr::BoolLit(_, _) => EmaType::Bool,
            Expr::Identifier(name, span) => {
                self.resolve(name).unwrap_or_else(|| {
                    self.error(span.clone(), format!("Error: Undefined variable '{}'", name));
                    EmaType::Void
                })
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
                    "std::fs" | "std::net" => EmaType::Void,
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
            Expr::UiElement { tag: _, props, children, span: _span } => {
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
        }
    }
}
