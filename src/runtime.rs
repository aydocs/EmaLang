use std::collections::HashMap;
use crate::ast::{
    EmbeddedKind, Expr, Program, Stmt, BinaryOp,
    PhpExpr as AstPhpExpr, PhpProgram as AstPhpProgram, PhpStmt as AstPhpStmt,
};
use serde_json::json;
use serde_json::Value as JsonValue;
use sqlx::{AnyPool, Row};
use axum::{
    routing::{get, any},
    http::{header, StatusCode},
    response::IntoResponse,
    Router,
    extract::State,
};
use tower_http::cors::CorsLayer;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum PhpTok {
    DollarIdent(String),
    Ident(String),
    String(String),
    Int(i64),
    Float(f64),
    True,
    False,
    Echo,
    Print,
    If,
    Else,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Assign,
    Dot,
    Plus,
    Minus,
    EqEq,
    BangEq,
    EOF,
}

#[derive(Debug, Clone)]
enum PhpStmt {
    Echo(PhpExpr),
    Print(PhpExpr),
    Expr(PhpExpr),
    Assign { name: String, value: PhpExpr },
    If {
        condition: PhpExpr,
        then_branch: Vec<PhpStmt>,
        else_branch: Option<Vec<PhpStmt>>,
    },
    While {
        condition: PhpExpr,
        body: Vec<PhpStmt>,
    },
    For {
        init: Option<Box<PhpStmt>>,
        condition: Option<PhpExpr>,
        update: Option<PhpExpr>,
        body: Vec<PhpStmt>,
    },
}

#[derive(Debug, Clone)]
enum PhpExpr {
    Var(String),
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
    Call {
        name: String,
        args: Vec<PhpExpr>,
    },
    Binary {
        left: Box<PhpExpr>,
        op: PhpBinOp,
        right: Box<PhpExpr>,
    },
    ArrayLit(Vec<PhpExpr>),
    ObjectLit(Vec<(String, PhpExpr)>),
    Index { target: Box<PhpExpr>, index: Box<PhpExpr> },
}

#[derive(Debug, Clone, Copy)]
enum PhpBinOp {
    Concat,
    Add,
    Sub,
    EqEq,
    BangEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    AndAnd,
    OrOr,
}

struct PhpLexer {
    input: Vec<char>,
    pos: usize,
}

impl PhpLexer {
    fn new(s: &str) -> Self {
        Self { input: s.chars().collect(), pos: 0 }
    }

    fn cur(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.cur() {
            if c.is_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn read_while<F: Fn(char) -> bool>(&mut self, f: F) -> String {
        let mut s = String::new();
        while let Some(c) = self.cur() {
            if f(c) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    fn read_string(&mut self, quote: char) -> String {
        // assumes current is quote
        self.bump();
        let mut out = String::new();
        while let Some(c) = self.cur() {
            if c == '\\' {
                self.bump();
                if let Some(n) = self.cur() {
                    out.push(n);
                    self.bump();
                }
                continue;
            }
            if c == quote {
                self.bump();
                break;
            }
            out.push(c);
            self.bump();
        }
        out
    }

    fn next_tok(&mut self) -> PhpTok {
        self.skip_ws();
        let Some(c) = self.cur() else { return PhpTok::EOF; };

        match c {
            ';' => { self.bump(); PhpTok::Semi }
            '(' => { self.bump(); PhpTok::LParen }
            ')' => { self.bump(); PhpTok::RParen }
            '{' => { self.bump(); PhpTok::LBrace }
            '}' => { self.bump(); PhpTok::RBrace }
            ',' => { self.bump(); PhpTok::Comma }
            '.' => { self.bump(); PhpTok::Dot }
            '+' => { self.bump(); PhpTok::Plus }
            '-' => { self.bump(); PhpTok::Minus }
            '=' => {
                self.bump();
                if self.cur() == Some('=') {
                    self.bump();
                    PhpTok::EqEq
                } else {
                    PhpTok::Assign
                }
            }
            '!' => {
                self.bump();
                if self.cur() == Some('=') {
                    self.bump();
                    PhpTok::BangEq
                } else {
                    PhpTok::BangEq
                }
            }
            '"' | '\'' => PhpTok::String(self.read_string(c)),
            '$' => {
                self.bump();
                let name = self.read_while(|ch| ch.is_alphanumeric() || ch == '_');
                PhpTok::DollarIdent(name)
            }
            ch if ch.is_ascii_digit() => {
                let num = self.read_while(|x| x.is_ascii_digit() || x == '.');
                if num.contains('.') {
                    PhpTok::Float(num.parse::<f64>().unwrap_or(0.0))
                } else {
                    PhpTok::Int(num.parse::<i64>().unwrap_or(0))
                }
            }
            ch if ch.is_alphabetic() || ch == '_' => {
                let ident = self.read_while(|x| x.is_alphanumeric() || x == '_');
                match ident.as_str() {
                    "true" => PhpTok::True,
                    "false" => PhpTok::False,
                    "echo" => PhpTok::Echo,
                    "print" => PhpTok::Print,
                    "if" => PhpTok::If,
                    "else" => PhpTok::Else,
                    _ => PhpTok::Ident(ident),
                }
            }
            _ => {
                self.bump();
                self.next_tok()
            }
        }
    }
}

struct PhpParser {
    toks: Vec<PhpTok>,
    pos: usize,
}

impl PhpParser {
    fn new(toks: Vec<PhpTok>) -> Self {
        Self { toks, pos: 0 }
    }

    fn cur(&self) -> &PhpTok {
        self.toks.get(self.pos).unwrap_or(&PhpTok::EOF)
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn parse_program(&mut self) -> Vec<PhpStmt> {
        let mut out = Vec::new();
        while !matches!(self.cur(), PhpTok::EOF | PhpTok::RBrace) {
            if let Some(s) = self.parse_stmt() {
                out.push(s);
            } else {
                self.bump();
            }
        }
        out
    }

    fn parse_stmt(&mut self) -> Option<PhpStmt> {
        match self.cur() {
            PhpTok::Echo => {
                self.bump();
                let e = self.parse_expr();
                if matches!(self.cur(), PhpTok::Semi) { self.bump(); }
                Some(PhpStmt::Echo(e))
            }
            PhpTok::Print => {
                self.bump();
                let e = self.parse_expr();
                if matches!(self.cur(), PhpTok::Semi) { self.bump(); }
                Some(PhpStmt::Print(e))
            }
            PhpTok::Ident(_) => {
                let e = self.parse_expr();
                if matches!(self.cur(), PhpTok::Semi) { self.bump(); }
                Some(PhpStmt::Expr(e))
            }
            PhpTok::DollarIdent(name) => {
                let n = name.clone();
                self.bump();
                if matches!(self.cur(), PhpTok::Assign) {
                    self.bump();
                    let v = self.parse_expr();
                    if matches!(self.cur(), PhpTok::Semi) { self.bump(); }
                    Some(PhpStmt::Assign { name: n, value: v })
                } else {
                    None
                }
            }
            PhpTok::If => {
                self.bump();
                if matches!(self.cur(), PhpTok::LParen) { self.bump(); }
                let cond = self.parse_expr();
                if matches!(self.cur(), PhpTok::RParen) { self.bump(); }
                if matches!(self.cur(), PhpTok::LBrace) { self.bump(); }
                let then_branch = self.parse_program();
                if matches!(self.cur(), PhpTok::RBrace) { self.bump(); }

                let else_branch = if matches!(self.cur(), PhpTok::Else) {
                    self.bump();
                    if matches!(self.cur(), PhpTok::LBrace) { self.bump(); }
                    let else_stmts = self.parse_program();
                    if matches!(self.cur(), PhpTok::RBrace) { self.bump(); }
                    Some(else_stmts)
                } else {
                    None
                };
                Some(PhpStmt::If { condition: cond, then_branch, else_branch })
            }
            PhpTok::Semi => {
                self.bump();
                None
            }
            _ => None,
        }
    }

    fn parse_expr(&mut self) -> PhpExpr {
        self.parse_equality()
    }

    fn parse_equality(&mut self) -> PhpExpr {
        let mut expr = self.parse_concat();
        loop {
            match self.cur() {
                PhpTok::EqEq => {
                    self.bump();
                    let r = self.parse_concat();
                    expr = PhpExpr::Binary { left: Box::new(expr), op: PhpBinOp::EqEq, right: Box::new(r) };
                }
                PhpTok::BangEq => {
                    self.bump();
                    let r = self.parse_concat();
                    expr = PhpExpr::Binary { left: Box::new(expr), op: PhpBinOp::BangEq, right: Box::new(r) };
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_concat(&mut self) -> PhpExpr {
        let mut expr = self.parse_term();
        while matches!(self.cur(), PhpTok::Dot) {
            self.bump();
            let r = self.parse_term();
            expr = PhpExpr::Binary { left: Box::new(expr), op: PhpBinOp::Concat, right: Box::new(r) };
        }
        expr
    }

    fn parse_term(&mut self) -> PhpExpr {
        let mut expr = self.parse_primary();
        loop {
            match self.cur() {
                PhpTok::Plus => {
                    self.bump();
                    let r = self.parse_primary();
                    expr = PhpExpr::Binary { left: Box::new(expr), op: PhpBinOp::Add, right: Box::new(r) };
                }
                PhpTok::Minus => {
                    self.bump();
                    let r = self.parse_primary();
                    expr = PhpExpr::Binary { left: Box::new(expr), op: PhpBinOp::Sub, right: Box::new(r) };
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_primary(&mut self) -> PhpExpr {
        match self.cur() {
            PhpTok::DollarIdent(n) => {
                let name = n.clone();
                self.bump();
                PhpExpr::Var(name)
            }
            PhpTok::Ident(name) => {
                let n = name.clone();
                self.bump();
                if matches!(self.cur(), PhpTok::LParen) {
                    self.bump();
                    let mut args = Vec::new();
                    if !matches!(self.cur(), PhpTok::RParen) {
                        loop {
                            args.push(self.parse_expr());
                            if matches!(self.cur(), PhpTok::Comma) {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                    }
                    if matches!(self.cur(), PhpTok::RParen) {
                        self.bump();
                    }
                    PhpExpr::Call { name: n, args }
                } else {
                    // Bare identifiers not supported in this subset; treat as empty string
                    PhpExpr::String("".to_string())
                }
            }
            PhpTok::String(s) => {
                let v = s.clone();
                self.bump();
                PhpExpr::String(v)
            }
            PhpTok::Int(i) => {
                let v = *i;
                self.bump();
                PhpExpr::Int(v)
            }
            PhpTok::Float(f) => {
                let v = *f;
                self.bump();
                PhpExpr::Float(v)
            }
            PhpTok::True => { self.bump(); PhpExpr::Bool(true) }
            PhpTok::False => { self.bump(); PhpExpr::Bool(false) }
            PhpTok::LParen => {
                self.bump();
                let e = self.parse_expr();
                if matches!(self.cur(), PhpTok::RParen) { self.bump(); }
                e
            }
            _ => {
                self.bump();
                PhpExpr::String("".to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeValue {
    StringVal(String),
    IntVal(i64),
    FloatVal(f64),
    BoolVal(bool),
    JsonVal(JsonValue),
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Null,
}

impl RuntimeValue {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            RuntimeValue::StringVal(s) => json!(s),
            RuntimeValue::IntVal(i) => json!(i),
            RuntimeValue::FloatVal(f) => json!(f),
            RuntimeValue::BoolVal(b) => json!(b),
            RuntimeValue::JsonVal(v) => v.clone(),
            RuntimeValue::Null => json!(null),
            _ => json!(null), // Functions cannot be serialized to JSON currently
        }
    }
}

#[derive(Clone)]
pub struct Environment {
    variables: HashMap<String, RuntimeValue>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            variables: HashMap::new(),
        }
    }


    pub fn define(&mut self, name: String, value: RuntimeValue) {
        self.variables.insert(name, value);
    }

    pub fn get(&self, name: &str) -> Option<RuntimeValue> {
        self.variables.get(name).cloned()
    }
}

#[derive(Clone)]
pub struct Interpreter {
    pub env: Environment,
    pub return_value: Option<RuntimeValue>,
    pub db_pool: sqlx::AnyPool,
    pub hydration_state: HashMap<String, RuntimeValue>,
    pub http_routes: HashMap<String, HttpRoute>,
    pub model_fields: HashMap<String, Vec<(String, crate::ast::Type)>>,
    pub history: Vec<()>,         // Compatibility stub
    pub test_mode: bool,          // Compatibility stub
    pub test_failures: Vec<String>, // Compatibility stub
}

#[derive(Debug, Clone)]
pub enum PhpHandler {
    Raw(String),
    Ast(AstPhpProgram),
}

#[derive(Debug, Clone)]
pub struct HttpRoute {
    pub handler: PhpHandler,
    pub status: u16,
    pub content_type: String,
}

#[derive(Clone)]
struct AppState {
    hydration_state: HashMap<String, RuntimeValue>,
    cached_ssr_html: String,
    cached_ssr_css: String,
    build_hash: String,
    js_hash: String,
    js_content: String,
    build_info_content: String,
    tailwind_needed: bool,
    bootstrap_needed: bool,
}

impl Interpreter {
    pub fn new(_verbose: bool, db_pool: sqlx::AnyPool) -> Self {
        Interpreter {
            env: Environment::new(),
            return_value: None,
            db_pool,
            hydration_state: HashMap::new(),
            http_routes: HashMap::new(),
            model_fields: HashMap::new(),
            history: Vec::new(),
            test_mode: false,
            test_failures: Vec::new(),
        }
    }

    pub fn save_snapshot(&mut self) {}
    pub fn list_vars(&self) -> Vec<(String, RuntimeValue)> {
        self.env.variables.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
    pub fn clear_env(&mut self) { self.env.variables.clear(); }
    pub fn restore_snapshot(&mut self) -> bool { false }

    fn is_safe_sql_ident(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    // Legacy DB helpers removed. using direct sqlx in eval_expr/eval_stmt.

    fn model_columns_for_select(&self, model: &str) -> Vec<String> {
        self.model_fields
            .get(model)
            .map(|v| v.iter().map(|(n, _t)| n.clone()).collect())
            .unwrap_or_default()
    }

    async fn model_all_json(&self, model: &str) -> String {
        if !Self::is_safe_sql_ident(model) {
            return "[]".to_string();
        }
        let cols = self.model_columns_for_select(model);
        let select_cols = if cols.is_empty() {
            "id".to_string()
        } else {
            format!("id, {}", cols.join(", "))
        };
        let query = format!("SELECT {} FROM {}", select_cols, model);
        let rows = match sqlx::query(&query).fetch_all(&self.db_pool).await {
            Ok(r) => r,
            Err(_) => return "[]".to_string(),
        };
        let mut list = Vec::new();
        for row in rows {
            let mut obj = serde_json::Map::new();
            let rid: i64 = row.try_get(0).unwrap_or(0);
            obj.insert("id".to_string(), serde_json::json!(rid));
            for (idx, name) in cols.iter().enumerate() {
                use sqlx::Row;
                let val: serde_json::Value = if let Ok(s) = row.try_get::<String, _>(idx + 1) {
                    serde_json::json!(s)
                } else if let Ok(i) = row.try_get::<i64, _>(idx + 1) {
                    serde_json::json!(i)
                } else if let Ok(f) = row.try_get::<f64, _>(idx + 1) {
                    serde_json::json!(f)
                } else {
                    serde_json::Value::Null
                };
                obj.insert(name.clone(), val);
            }
            list.push(serde_json::Value::Object(obj));
        }
        serde_json::Value::Array(list).to_string()
    }

    async fn model_find_json(&self, model: &str, id: i64) -> String {
        if !Self::is_safe_sql_ident(model) {
            return "null".to_string();
        }
        let cols = self.model_columns_for_select(model);
        let select_cols = if cols.is_empty() {
            "id".to_string()
        } else {
            format!("id, {}", cols.join(", "))
        };
        let query = format!("SELECT {} FROM {} WHERE id = $1 LIMIT 1", select_cols, model);
        let row = match sqlx::query(&query).bind(id).fetch_optional(&self.db_pool).await {
            Ok(Some(r)) => r,
            _ => return "null".to_string(),
        };

        let mut obj = serde_json::Map::new();
        let rid: i64 = row.try_get(0).unwrap_or(id);
        obj.insert("id".to_string(), serde_json::json!(rid));
        for (idx, name) in cols.iter().enumerate() {
            use sqlx::Row;
            let val: serde_json::Value = if let Ok(s) = row.try_get::<String, _>(idx + 1) {
                serde_json::json!(s)
            } else if let Ok(i) = row.try_get::<i64, _>(idx + 1) {
                serde_json::json!(i)
            } else if let Ok(f) = row.try_get::<f64, _>(idx + 1) {
                serde_json::json!(f)
            } else {
                serde_json::Value::Null
            };
            obj.insert(name.clone(), val);
        }
        serde_json::Value::Object(obj).to_string()
    }

    pub async fn eval_program(&mut self, program: Program) {
        for stmt in program.statements {
            self.eval_stmt(stmt).await;
            if self.return_value.is_some() {
                break;
            }
        }
    }

    pub fn eval_stmt<'a>(&'a mut self, stmt: Stmt) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            match stmt {
            Stmt::VarDecl { name, value, .. } => {
                let val = self.eval_expr(value).await;
                self.env.define(name, val);
            }
            Stmt::StateDecl { name, value, .. } => {
                let val = self.eval_expr(value).await;
                self.env.define(name.clone(), val.clone());
                self.hydration_state.insert(name, val);
            }
            Stmt::PrintStmt(expr, _) => {
                let val = self.eval_expr(expr).await;
                match val {
                    RuntimeValue::StringVal(s) => println!("{}", s),
                    RuntimeValue::IntVal(i) => println!("{}", i),
                    RuntimeValue::FloatVal(f) => println!("{}", f),
                    RuntimeValue::BoolVal(b) => println!("{}", b),
                    RuntimeValue::JsonVal(v) => println!("{}", serde_json::to_string(&v).unwrap_or("null".to_string())),
                    RuntimeValue::Function { name, .. } => println!("<fn {}>", name),
                    RuntimeValue::Null => println!("null"),
                }
            }
            Stmt::ExprStmt(expr, _) => {
                self.eval_expr(expr).await;
            }
            Stmt::IfStmt { condition, then_branch, else_branch, .. } => {
                let cond_val = self.eval_expr(condition).await;
                let is_true = match cond_val {
                    RuntimeValue::BoolVal(b) => b,
                    RuntimeValue::IntVal(i) => i != 0,
                    RuntimeValue::FloatVal(f) => f != 0.0,
                    _ => false,
                };
                if is_true {
                    for s in then_branch {
                        self.eval_stmt(s).await;
                        if self.return_value.is_some() { break; }
                    }
                } else if let Some(else_stmts) = else_branch {
                    for s in else_stmts {
                        self.eval_stmt(s).await;
                        if self.return_value.is_some() { break; }
                    }
                }
            }
            Stmt::WhileStmt { condition, body, .. } => {
                loop {
                    let cond_val = self.eval_expr(condition.clone()).await;
                    let is_true = match cond_val {
                        RuntimeValue::BoolVal(b) => b,
                        RuntimeValue::IntVal(i) => i != 0,
                        RuntimeValue::FloatVal(f) => f != 0.0,
                        _ => false,
                    };
                    if !is_true || self.return_value.is_some() {
                        break;
                    }
                    for s in body.clone() {
                        self.eval_stmt(s).await;
                        if self.return_value.is_some() { break; }
                    }
                }
            }
            Stmt::ServerBlock(stmts, _) => {
                println!("[EMA SERVER YURUTULUYOR]");
                for s in stmts {
                    self.eval_stmt(s).await;
                    if self.return_value.is_some() { break; }
                }
            }
            Stmt::ClientBlock(stmts, _) => {
                println!("[EMA CLIENT YURUTULUYOR (WASM HEADLESS)]");
                for s in stmts {
                    self.eval_stmt(s).await;
                    if self.return_value.is_some() { break; }
                }
            }
            Stmt::ModelDecl { name, fields, .. } => {
                // Models are primarily for the DB layer or DB compile-time macros.
                // In runtime memory, we simply register the schema presence.
                println!("[EMA-DB] Model '{}' registered successfully (schema).", name);
                
                let mut columns = Vec::new();
                let mut schema = Vec::new();
                for field in fields {
                    let sql_type = match field.field_type {
                        crate::ast::Type::Str => "TEXT",
                        crate::ast::Type::Int => "INTEGER",
                        crate::ast::Type::Float => "REAL",
                        _ => "TEXT",
                    };
                    columns.push(format!("{} {}", field.name, sql_type));
                    schema.push((field.name.clone(), field.field_type.clone()));
                    println!("  -> Field: {}, Type: {:?}", field.name, field.field_type);
                }
                self.model_fields.insert(name.clone(), schema);

                let query = format!("CREATE TABLE IF NOT EXISTS {} (id INTEGER PRIMARY KEY AUTOINCREMENT, {})", name, columns.join(", "));
                let _ = sqlx::query(&query).execute(&self.db_pool).await;
                println!("[EMA-DB] SQLite table ready: '{}'.", name);
            }
            Stmt::FnDecl { name, params, body, .. } => {
                let func_val = RuntimeValue::Function {
                    name: name.clone(),
                    params: params.into_iter().map(|(n, _)| n).collect(),
                    body,
                };
                self.env.define(name, func_val);
            }
            Stmt::ReturnStmt(opt_expr, _) => {
                let val = if let Some(expr) = opt_expr {
                    self.eval_expr(expr).await
                } else {
                    RuntimeValue::Null
                };
                self.return_value = Some(val);
            }
            _ => {}
        }
    })
    }

    pub fn eval_expr<'a>(&'a mut self, expr: Expr) -> std::pin::Pin<Box<dyn std::future::Future<Output = RuntimeValue> + Send + 'a>> {
        Box::pin(async move {
            match expr {
            Expr::IntLit(val, _) => RuntimeValue::IntVal(val),
            Expr::FloatLit(val, _) => RuntimeValue::FloatVal(val),
            Expr::StringLit(s, _) => RuntimeValue::StringVal(s),
            Expr::BoolLit(b, _) => RuntimeValue::BoolVal(b),
            Expr::Identifier(ident, _) => {
                if let Some(val) = self.env.get(&ident) {
                    val
                } else {
                    panic!("Tanimlanmayan degisken cagirisi: {}", ident);
                }
            }
            Expr::Call { callee, args, .. } => {
                let callee_val = self.eval_expr(*callee).await;

                if let RuntimeValue::Function { name: _, params, body } = callee_val {
                    if args.len() != params.len() {
                        panic!("Argument error: expected {} arguments, got {}", params.len(), args.len());
                    }

                    // Pre-evaluate arguments
                    let mut evaluated_args = Vec::new();
                    for arg in args {
                        evaluated_args.push(self.eval_expr(arg).await);
                    }
                    
                    // Setup local variable scope injection
                    let mut prev_env = HashMap::new();
                    for (i, param) in params.iter().enumerate() {
                        if let Some(old_val) = self.env.get(param) {
                            prev_env.insert(param.clone(), old_val);
                        }
                        self.env.define(param.clone(), evaluated_args[i].clone());
                    }

                    // Execute function body
                    for s in body {
                        self.eval_stmt(s).await;
                        if self.return_value.is_some() {
                            break;
                        }
                    }

                    let ret = self.return_value.take().unwrap_or(RuntimeValue::Null);

                    // Restore previous scope values
                    for param in params {
                        if let Some(old_val) = prev_env.get(&param) {
                            self.env.define(param, old_val.clone());
                        } else {
                            self.env.variables.remove(&param);
                        }
                    }

                    ret
                } else {
                    panic!("Sadece fonksiyon formlari cagrilabilir!");
                }
            }
            Expr::NamespaceCall { namespace, method, args, .. } => {
                // OS Syscall routing via Native Rust Stdlib integration
                match (namespace.as_str(), method.as_str()) {
                    ("std::db", "migrate") => {
                        let dir = if args.is_empty() {
                            "migrations".to_string()
                        } else {
                            match self.eval_expr(args[0].clone()).await {
                                RuntimeValue::StringVal(s) => s,
                                _ => "migrations".to_string(),
                            }
                        };
                        match crate::db::apply_migrations(&self.db_pool, &dir).await {
                            Ok(_) => RuntimeValue::BoolVal(true),
                            Err(e) => panic!("DB migration error: {}", e),
                        }
                    }
                    ("std::fs", "read") => {
                        if args.len() != 1 {
                            panic!("std::fs::read(path) 1 parametre gerektirir!");
                        }
                        let path_arg = self.eval_expr(args[0].clone()).await;
                        if let RuntimeValue::StringVal(path) = path_arg {
                            match std::fs::read_to_string(&path) {
                                Ok(content) => RuntimeValue::StringVal(content),
                                Err(e) => panic!("Dosya okuma hatasi {}: {}", path, e),
                            }
                        } else {
                            panic!("Dosya yolu String olmalidir!");
                        }
                    }
                    ("std::fs", "write") => {
                        if args.len() != 2 {
                            panic!("std::fs::write(path, data) 2 parametre gerektirir!");
                        }
                        let path_arg = self.eval_expr(args[0].clone()).await;
                        let data_arg = self.eval_expr(args[1].clone()).await;

                        if let (RuntimeValue::StringVal(path), RuntimeValue::StringVal(data)) = (path_arg, data_arg) {
                            match std::fs::write(&path, data) {
                                Ok(_) => RuntimeValue::BoolVal(true),
                                Err(e) => panic!("Dosya yazma hatasi {}: {}", path, e),
                            }
                        } else {
                            panic!("std::fs::write argumanlari (String, String) olmalidir!");
                        }
                    }
                    ("std::net", "listen") => {
                        if args.len() != 1 {
                            panic!("std::net::listen(port) 1 parametre gerektirir!");
                        }
                        let port_arg = self.eval_expr(args[0].clone()).await;
                        if let RuntimeValue::IntVal(port) = port_arg {
                            println!("[EMA-NET] Starting server on port {}...", port);
                            println!("[EMA-NET] Dinleme basarili. (Simulasyon: HTTP 200 OK gonderildi)");
                            RuntimeValue::BoolVal(true)
                        } else {
                            panic!("Port numarasi Integer olmalidir!");
                        }
                    }
                    ("std::http", "route") => {
                        if args.len() < 2 || args.len() > 5 {
                            panic!("std::http::route([method,] path, handler[, status][, contentType]) 2-5 parametre gerektirir!");
                        }

                        // Overload:
                        // - route(path, php, [status], [ct])
                        // - route(method, path, php, [status], [ct])   where method is \"GET\"/\"POST\" and path starts with '/'
                        let first = self.eval_expr(args[0].clone()).await;
                        let second = self.eval_expr(args[1].clone()).await;

                        let (method, path, handler_idx) = match (first, second) {
                            (RuntimeValue::StringVal(m), RuntimeValue::StringVal(p))
                                if !m.starts_with('/') && p.starts_with('/') =>
                            {
                                (m.to_uppercase(), p, 2usize)
                            }
                            (RuntimeValue::StringVal(p), _) if p.starts_with('/') => {
                                ("ANY".to_string(), p, 1usize)
                            }
                            _ => panic!("std::http::route icin path String (\"/...\") olmalidir!"),
                        };

                        let handler = match &args[handler_idx] {
                            Expr::EmbeddedBlock { kind: EmbeddedKind::Php, raw, .. } => PhpHandler::Raw(raw.clone()),
                            Expr::PhpAst { program, .. } => PhpHandler::Ast(program.clone()),
                            _ => panic!("std::http::route handler must be php {{ ... }}"),
                        };

                        let mut status: u16 = 200;
                        let mut content_type: String = "text/plain; charset=utf-8".to_string();

                        let opt_start = handler_idx + 1;

                        if args.len() >= opt_start + 1 {
                            let v = self.eval_expr(args[opt_start].clone()).await;
                            match v {
                                RuntimeValue::IntVal(i) => status = i.clamp(100, 599) as u16,
                                RuntimeValue::FloatVal(f) => status = (f as i64).clamp(100, 599) as u16,
                                RuntimeValue::StringVal(s) => content_type = s,
                                _ => {}
                            }
                        }
                        if args.len() >= opt_start + 2 {
                            let v = self.eval_expr(args[opt_start + 1].clone()).await;
                            if let RuntimeValue::StringVal(s) = v {
                                content_type = s;
                            }
                        }

                        let key = format!("{} {}", method, path);
                        self.http_routes.insert(key.clone(), HttpRoute { handler, status, content_type });
                        println!("[EMA-HTTP] Route registered: {}", key);
                        RuntimeValue::BoolVal(true)
                    }
                    ("std::http", "serve") => {
                        let port_val = self.eval_expr(args[0].clone()).await;
                        let port = match port_val {
                            RuntimeValue::IntVal(p) => p,
                            _ => panic!("std::http::serve(port) icin port Integer olmalidir!"),
                        };

                        let build_info_content = std::fs::read_to_string("frontend.build.json").unwrap_or_else(|_| "{}".to_string());
                        let build_info: serde_json::Value = serde_json::from_str(&build_info_content).unwrap_or(serde_json::json!({}));
                        
                        let tailwind_needed = build_info.get("tailwind").and_then(|v| v.as_bool()).unwrap_or(false);
                        let bootstrap_needed = build_info.get("bootstrap").and_then(|v| v.as_bool()).unwrap_or(false);

                        let shared_state = Arc::new(AppState {
                            hydration_state: self.hydration_state.clone(),
                            cached_ssr_html: std::fs::read_to_string("frontend.ssr.html").unwrap_or_default(),
                            cached_ssr_css: std::fs::read_to_string("frontend.ssr.css").unwrap_or_default(),
                            build_hash: build_info.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            js_hash: build_info.get("jsHash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            js_content: std::fs::read_to_string("frontend.js").unwrap_or_default(),
                            build_info_content,
                            tailwind_needed,
                            bootstrap_needed,
                        });

                        async fn handler(
                            path_uri: axum::extract::OriginalUri,
                            method_obj: axum::http::Method,
                            State(state): State<Arc<AppState>>,
                            body_resp: String,
                        ) -> impl IntoResponse {
                            let path = path_uri.path();
                            let method = method_obj.to_string().to_uppercase();

                            if path == "/frontend.js" {
                                return ([(header::CONTENT_TYPE, "application/javascript")], state.js_content.clone()).into_response();
                            }
                            if path == "/frontend.build.json" {
                                return ([(header::CONTENT_TYPE, "application/json")], state.build_info_content.clone()).into_response();
                            }

                            if path == "/" || path == "/index.html" {
                                let mut h_map = serde_json::Map::new();
                                for (k, v) in &state.hydration_state {
                                    h_map.insert(k.clone(), v.to_json());
                                }
                                let h_json = serde_json::Value::Object(h_map).to_string();

                                let tailwind_tag = if state.tailwind_needed { r#"<script src="https://unpkg.com/@tailwindcss/browser@4"></script>"# } else { "" };
                                let bootstrap_tag = if state.bootstrap_needed { r#"<link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css" rel="stylesheet"><script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js"></script>"# } else { "" };

                                let html = format!(
                                    r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>body {{ margin: 0; background: #050a0f; color: white; font-family: sans-serif; display: flex; justify-content: center; align-items: center; min-height: 100vh; }}</style>
    <style>{}</style>
    {}
    {}
</head>
<body>
    <div id="ema-root">{}</div>
    <script>window.__EMA_HYDRATION__ = {}; window.__EMA_BUILD__ = {{ hash: "{}" }};</script>
    <script src="/frontend.js?v={}"></script>
</body>
</html>"#, state.cached_ssr_css, tailwind_tag, bootstrap_tag, state.cached_ssr_html, h_json, state.build_hash, state.js_hash);
                                return ([(header::CONTENT_TYPE, "text/html")], html).into_response();
                            }

                            (StatusCode::NOT_FOUND, "404 Not Found").into_response()
                        }

                        let app = Router::new()
                            .fallback(any(handler))
                            .with_state(shared_state);

                        let addr = format!("0.0.0.0:{}", port);
                        println!("[EMA-HTTP] Port {} üzerinde sunucu baslatiliyor...", port);
                        
                        let listener = tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind port");
                        axum::serve(listener, app).await.expect("Failed to start axum server");

                        RuntimeValue::Null
                    }
                    (ns, method_str) if ns.chars().next().unwrap().is_uppercase() => {
                        // Custom Native DB Model Interception `Model::insert(...)`
                        match method_str {
                            "insert" => {
                                let mut placeholders = Vec::new();
                                for i in 1..=args.len() {
                                    placeholders.push(format!("${}", i));
                                }
                                let query = format!("INSERT INTO {} VALUES (NULL, {})", ns, placeholders.join(", "));
                                let mut q = sqlx::query(&query);
                                for arg in args {
                                    let val = self.eval_expr(arg.clone()).await;
                                    q = match val {
                                        RuntimeValue::StringVal(s) => q.bind(s),
                                        RuntimeValue::IntVal(i) => q.bind(i),
                                        RuntimeValue::FloatVal(f) => q.bind(f),
                                        RuntimeValue::BoolVal(b) => q.bind(b as i64),
                                        _ => q.bind(None::<String>),
                                    };
                                }
                                match q.execute(&self.db_pool).await {
                                    Ok(_) => {
                                        println!("[EMA-DB] [{}] INSERT executed", ns);
                                        RuntimeValue::BoolVal(true)
                                    },
                                    Err(e) => panic!("DB insert error ({}): {}", ns, e),
                                }
                            }
                            "all" => {
                                // Returns JSON array string of rows.
                                RuntimeValue::StringVal(self.model_all_json(&ns).await)
                            }
                            "find" => {
                                if args.len() != 1 {
                                    panic!("Model::find(id) expects 1 argument");
                                }
                                let id_val = self.eval_expr(args[0].clone()).await;
                                let id = match id_val {
                                    RuntimeValue::IntVal(i) => i,
                                    RuntimeValue::FloatVal(f) => f as i64,
                                    RuntimeValue::StringVal(s) => s.parse::<i64>().unwrap_or(0),
                                    _ => 0,
                                };
                                RuntimeValue::StringVal(self.model_find_json(&ns, id).await)
                            }
                            _ => panic!("Unsupported model method: {}::{}", ns, method_str),
                        }
                    }
                    _ => panic!("Unsupported standard library call: {}::{}", namespace, method),
                }
            }
            Expr::Binary { left, op, right, .. } => {
                let left_val = self.eval_expr(*left).await;
                let right_val = self.eval_expr(*right).await;

                match (left_val, op, right_val) {
                    (RuntimeValue::IntVal(l), BinaryOp::Add, RuntimeValue::IntVal(r)) => RuntimeValue::IntVal(l + r),
                    (RuntimeValue::IntVal(l), BinaryOp::Sub, RuntimeValue::IntVal(r)) => RuntimeValue::IntVal(l - r),
                    (RuntimeValue::IntVal(l), BinaryOp::Mul, RuntimeValue::IntVal(r)) => RuntimeValue::IntVal(l * r),
                    (RuntimeValue::IntVal(l), BinaryOp::Div, RuntimeValue::IntVal(r)) => {
                        if r == 0 { panic!("Sifira bolme hatasi (Division by zero)!"); }
                        RuntimeValue::IntVal(l / r)
                    },
                    (RuntimeValue::IntVal(l), BinaryOp::EqEq, RuntimeValue::IntVal(r)) => RuntimeValue::BoolVal(l == r),
                    (RuntimeValue::IntVal(l), BinaryOp::BangEq, RuntimeValue::IntVal(r)) => RuntimeValue::BoolVal(l != r),
                    (RuntimeValue::IntVal(l), BinaryOp::Less, RuntimeValue::IntVal(r)) => RuntimeValue::BoolVal(l < r),
                    (RuntimeValue::IntVal(l), BinaryOp::LessEq, RuntimeValue::IntVal(r)) => RuntimeValue::BoolVal(l <= r),
                    (RuntimeValue::IntVal(l), BinaryOp::Greater, RuntimeValue::IntVal(r)) => RuntimeValue::BoolVal(l > r),
                    (RuntimeValue::IntVal(l), BinaryOp::GreaterEq, RuntimeValue::IntVal(r)) => RuntimeValue::BoolVal(l >= r),

                    (RuntimeValue::FloatVal(l), BinaryOp::Add, RuntimeValue::FloatVal(r)) => RuntimeValue::FloatVal(l + r),
                    (RuntimeValue::FloatVal(l), BinaryOp::Sub, RuntimeValue::FloatVal(r)) => RuntimeValue::FloatVal(l - r),
                    (RuntimeValue::FloatVal(l), BinaryOp::Mul, RuntimeValue::FloatVal(r)) => RuntimeValue::FloatVal(l * r),
                    (RuntimeValue::FloatVal(l), BinaryOp::Div, RuntimeValue::FloatVal(r)) => RuntimeValue::FloatVal(l / r),
                    (RuntimeValue::FloatVal(l), BinaryOp::EqEq, RuntimeValue::FloatVal(r)) => RuntimeValue::BoolVal(l == r),
                    (RuntimeValue::FloatVal(l), BinaryOp::BangEq, RuntimeValue::FloatVal(r)) => RuntimeValue::BoolVal(l != r),
                    (RuntimeValue::FloatVal(l), BinaryOp::Less, RuntimeValue::FloatVal(r)) => RuntimeValue::BoolVal(l < r),
                    (RuntimeValue::FloatVal(l), BinaryOp::LessEq, RuntimeValue::FloatVal(r)) => RuntimeValue::BoolVal(l <= r),
                    (RuntimeValue::FloatVal(l), BinaryOp::Greater, RuntimeValue::FloatVal(r)) => RuntimeValue::BoolVal(l > r),
                    (RuntimeValue::FloatVal(l), BinaryOp::GreaterEq, RuntimeValue::FloatVal(r)) => RuntimeValue::BoolVal(l >= r),
                    
                    (RuntimeValue::StringVal(l), BinaryOp::Add, RuntimeValue::StringVal(r)) => RuntimeValue::StringVal(format!("{}{}", l, r)),
                    (RuntimeValue::StringVal(l), BinaryOp::EqEq, RuntimeValue::StringVal(r)) => RuntimeValue::BoolVal(l == r),
                    (RuntimeValue::StringVal(l), BinaryOp::BangEq, RuntimeValue::StringVal(r)) => RuntimeValue::BoolVal(l != r),

                    _ => panic!("Gecersiz ikili islem (Invalid binary operation)!"),
                }
            }
            Expr::UiElement { .. } => {
                // UI Elements are handled by WasmBuilder for the frontend.
                // In native runtime, they are ignored or treated as null.
                RuntimeValue::Null
            }
            Expr::EmbeddedBlock { kind, raw, span: _ } => {
                match kind {
                    EmbeddedKind::Php => self.eval_php_block(&raw).await,
                    _ => RuntimeValue::Null,
                }
            }
            Expr::PhpAst { program, .. } => {
                self.eval_php_program_ast(&program).await
            }
            Expr::HtmlAst { .. } | Expr::CssAst { .. } | Expr::JsAst { .. } => {
                // Parsed client-side artifacts are handled by wasm_builder.
                RuntimeValue::Null
            }
            Expr::Interpolation(inner, _) => {
                self.eval_expr(*inner).await
            }
            _ => RuntimeValue::Null,
        }
    })
    }

    async fn eval_php_block(&mut self, raw: &str) -> RuntimeValue {
        let mut code = raw.trim().to_string();
        if code.starts_with("<?php") {
            code = code.trim_start_matches("<?php").to_string();
        }
        if code.ends_with("?>") {
            code = code.trim_end_matches("?>").to_string();
        }
        let mut lex = PhpLexer::new(&code);
        let mut toks = Vec::new();
        loop {
            let t = lex.next_tok();
            let done = matches!(t, PhpTok::EOF);
            toks.push(t);
            if done {
                break;
            }
        }
        let mut parser = PhpParser::new(toks);
        let prog = parser.parse_program();
        let mut out = None;
        for s in prog {
            self.eval_php_stmt(s, &mut out).await;
        }
        RuntimeValue::Null
    }

    fn lower_php_program(&self, prog: &AstPhpProgram) -> Vec<PhpStmt> {
        prog.body.iter().map(|s| self.lower_php_stmt(s)).collect()
    }

    fn lower_php_stmt(&self, s: &AstPhpStmt) -> PhpStmt {
        match s {
            AstPhpStmt::Echo(e, _) => PhpStmt::Echo(self.lower_php_expr(e)),
            AstPhpStmt::Print(e, _) => PhpStmt::Print(self.lower_php_expr(e)),
            AstPhpStmt::Expr(e, _) => PhpStmt::Expr(self.lower_php_expr(e)),
            AstPhpStmt::Assign { name, value, .. } => PhpStmt::Assign { name: name.clone(), value: self.lower_php_expr(value) },
            AstPhpStmt::If { condition, then_branch, else_branch, .. } => PhpStmt::If {
                condition: self.lower_php_expr(condition),
                then_branch: then_branch.iter().map(|x| self.lower_php_stmt(x)).collect(),
                else_branch: else_branch.as_ref().map(|b| b.iter().map(|x| self.lower_php_stmt(x)).collect()),
            },
            AstPhpStmt::While { condition, body, .. } => PhpStmt::While {
                condition: self.lower_php_expr(condition),
                body: body.iter().map(|x| self.lower_php_stmt(x)).collect(),
            },
            AstPhpStmt::For { init, condition, update, body, .. } => PhpStmt::For {
                init: init.as_ref().map(|s| Box::new(self.lower_php_stmt(s))),
                condition: condition.as_ref().map(|e| self.lower_php_expr(e)),
                update: update.as_ref().map(|e| self.lower_php_expr(e)),
                body: body.iter().map(|x| self.lower_php_stmt(x)).collect(),
            },
        }
    }

    fn lower_php_expr(&self, e: &AstPhpExpr) -> PhpExpr {
        match e {
            AstPhpExpr::Var(n, _) => PhpExpr::Var(n.clone()),
            AstPhpExpr::String(s, _) => PhpExpr::String(s.clone()),
            AstPhpExpr::Int(i, _) => PhpExpr::Int(*i),
            AstPhpExpr::Float(f, _) => PhpExpr::Float(*f),
            AstPhpExpr::Bool(b, _) => PhpExpr::Bool(*b),
            AstPhpExpr::Null(_) => PhpExpr::Null,
            AstPhpExpr::Call { name, args, .. } => PhpExpr::Call {
                name: name.clone(),
                args: args.iter().map(|a| self.lower_php_expr(a)).collect(),
            },
            AstPhpExpr::Binary { left, op, right, .. } => {
                let bop = match op.as_str() {
                    "." => PhpBinOp::Concat,
                    "+" => PhpBinOp::Add,
                    "-" => PhpBinOp::Sub,
                    "==" => PhpBinOp::EqEq,
                    "!=" => PhpBinOp::BangEq,
                    "<" => PhpBinOp::Less,
                    "<=" => PhpBinOp::LessEq,
                    ">" => PhpBinOp::Greater,
                    ">=" => PhpBinOp::GreaterEq,
                    "&&" => PhpBinOp::AndAnd,
                    "||" => PhpBinOp::OrOr,
                    _ => PhpBinOp::Concat,
                };
                PhpExpr::Binary { left: Box::new(self.lower_php_expr(left)), op: bop, right: Box::new(self.lower_php_expr(right)) }
            }
            AstPhpExpr::ArrayLit { items, .. } => PhpExpr::ArrayLit(items.iter().map(|(_, x)| self.lower_php_expr(x)).collect()),
            AstPhpExpr::ObjectLit { props, .. } => PhpExpr::ObjectLit(
                props.iter().map(|(k, v)| (k.clone(), self.lower_php_expr(v))).collect()
            ),
            AstPhpExpr::Index { target, index, .. } => PhpExpr::Index {
                target: Box::new(self.lower_php_expr(target)),
                index: Box::new(self.lower_php_expr(index)),
            },
            _ => PhpExpr::Null,
        }
    }

    pub async fn eval_php_program_ast(&mut self, prog: &AstPhpProgram) -> RuntimeValue {
        let stmts = self.lower_php_program(prog);
        let mut out = None;
        for s in stmts {
            self.eval_php_stmt(s, &mut out).await;
        }
        RuntimeValue::Null
    }

    pub fn eval_php_stmt<'a>(&'a mut self, stmt: PhpStmt, out: &'a mut Option<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            match stmt {
                PhpStmt::Echo(e) => {
                    let v = self.eval_php_expr_ast(e).await;
                    self.php_emit(v, out);
                }
                PhpStmt::Print(e) => {
                    let v = self.eval_php_expr_ast(e).await;
                    self.php_emit(v, out);
                }
                PhpStmt::Expr(e) => {
                    self.eval_php_expr_ast(e).await;
                }
                PhpStmt::Assign { name, value } => {
                    let v = self.eval_php_expr_ast(value).await;
                    self.env.define(name, v);
                }
                PhpStmt::If { condition, then_branch, else_branch } => {
                    let cond = self.eval_php_expr_ast(condition).await;
                    let is_true = match cond {
                        RuntimeValue::BoolVal(b) => b,
                        RuntimeValue::IntVal(i) => i != 0,
                        RuntimeValue::FloatVal(f) => f != 0.0,
                        RuntimeValue::StringVal(s) => !s.is_empty(),
                        RuntimeValue::Null => false,
                        _ => false,
                    };
                    if is_true {
                        for s in then_branch {
                            self.eval_php_stmt(s, out).await;
                        }
                    } else if let Some(stmts) = else_branch {
                        for s in stmts {
                            self.eval_php_stmt(s, out).await;
                        }
                    }
                }
                PhpStmt::While { condition, body } => {
                    let mut iters = 0usize;
                    loop {
                        iters += 1;
                        if iters > 100_000 {
                            panic!("PHP while-loop exceeded iteration limit");
                        }
                        let cond = self.eval_php_expr_ast(condition.clone()).await;
                        let is_true = match cond {
                            RuntimeValue::BoolVal(b) => b,
                            RuntimeValue::IntVal(i) => i != 0,
                            RuntimeValue::FloatVal(f) => f != 0.0,
                            RuntimeValue::StringVal(s) => !s.is_empty(),
                            RuntimeValue::JsonVal(v) => !v.is_null() && v != JsonValue::Bool(false) && v != JsonValue::String(String::new()),
                            RuntimeValue::Null => false,
                            _ => false,
                        };
                        if !is_true {
                            break;
                        }
                        for s in body.clone() {
                            self.eval_php_stmt(s, out).await;
                        }
                    }
                }
                PhpStmt::For { init, condition, update, body } => {
                    if let Some(s) = init {
                        self.eval_php_stmt(*s, out).await;
                    }
                    let mut iters = 0usize;
                    loop {
                        iters += 1;
                        if iters > 100_000 {
                            panic!("PHP for-loop exceeded iteration limit");
                        }
                        if let Some(cond_e) = condition.clone() {
                            let cond_v = self.eval_php_expr_ast(cond_e).await;
                            let is_true = match cond_v {
                                RuntimeValue::BoolVal(b) => b,
                                RuntimeValue::IntVal(i) => i != 0,
                                RuntimeValue::FloatVal(f) => f != 0.0,
                                RuntimeValue::StringVal(s) => !s.is_empty(),
                                RuntimeValue::JsonVal(v) => !v.is_null() && v != JsonValue::Bool(false) && v != JsonValue::String(String::new()),
                                RuntimeValue::Null => false,
                                _ => false,
                            };
                            if !is_true { break; }
                        }
                        for s in body.clone() {
                            self.eval_php_stmt(s, out).await;
                        }
                        if let Some(upd) = update.clone() {
                            self.eval_php_expr_ast(upd).await;
                        }
                    }
                }
            }
        })
    }

    fn php_emit(&self, v: RuntimeValue, out: &mut Option<String>) {
        if let Some(buf) = out.as_mut() {
            buf.push_str(&self.php_to_string(v));
            buf.push('\n');
            return;
        }
        self.print_runtime_value(v);
    }

    async fn eval_php_http_handler(
        &mut self,
        handler: &PhpHandler,
        query: &HashMap<String, String>,
        post: &HashMap<String, String>,
        json_body: &(Option<String>, HashMap<String, String>),
        method: &str,
    ) -> String {
        // Inject query params as php vars: $_GET_name  (actually "$_GET_name")
        for (k, v) in query {
            self.env.define(format!("_GET_{}", k), RuntimeValue::StringVal(v.clone()));
        }
        for (k, v) in post {
            self.env.define(format!("_POST_{}", k), RuntimeValue::StringVal(v.clone()));
        }
        if let Some(raw_json) = &json_body.0 {
            self.env.define("_JSON_raw".to_string(), RuntimeValue::StringVal(raw_json.clone()));
        }
        for (k, v) in &json_body.1 {
            self.env.define(format!("_JSON_{}", k), RuntimeValue::StringVal(v.clone()));
        }
        self.env.define("_METHOD".to_string(), RuntimeValue::StringVal(method.to_string()));

        match handler {
            PhpHandler::Raw(raw) => {
                let mut code = raw.trim().to_string();
                if code.starts_with("<?php") {
                    code = code.trim_start_matches("<?php").to_string();
                }
                if code.ends_with("?>") {
                    code = code.trim_end_matches("?>").to_string();
                }

                let mut lex = PhpLexer::new(&code);
                let mut toks = Vec::new();
                loop {
                    let t = lex.next_tok();
                    let done = matches!(t, PhpTok::EOF);
                    toks.push(t);
                    if done {
                        break;
                    }
                }
                let mut parser = PhpParser::new(toks);
                let prog = parser.parse_program();

                let mut out = Some(String::new());
                for s in prog {
                    self.eval_php_stmt(s, &mut out).await;
                }
                out.unwrap_or_default()
            }
            PhpHandler::Ast(prog) => {
                let stmts = self.lower_php_program(prog);
                let mut out = Some(String::new());
                for s in stmts {
                    self.eval_php_stmt(s, &mut out).await;
                }
                out.unwrap_or_default()
            }
        }
    }

    fn split_url(url: &str) -> (String, HashMap<String, String>) {
        let (path, qs) = match url.split_once('?') {
            Some((p, q)) => (p, q),
            None => (url, ""),
        };
        let mut map = HashMap::new();
        for pair in qs.split('&') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            let (k, v) = match pair.split_once('=') {
                Some((a, b)) => (a, b),
                None => (pair, ""),
            };
            map.insert(Self::url_decode(k), Self::url_decode(v));
        }
        (path.to_string(), map)
    }

    fn url_decode(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '+' => out.push(' '),
                '%' => {
                    let h1 = chars.next();
                    let h2 = chars.next();
                    if let (Some(a), Some(b)) = (h1, h2) {
                        let hex = format!("{}{}", a, b);
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            out.push(byte as char);
                        }
                    }
                }
                _ => out.push(c),
            }
        }
        out
    }

    fn parse_form_urlencoded(body: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for pair in body.split('&') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            let (k, v) = match pair.split_once('=') {
                Some((a, b)) => (a, b),
                None => (pair, ""),
            };
            map.insert(Self::url_decode(k), Self::url_decode(v));
        }
        map
    }

    fn parse_json_shallow(body: &str) -> (Option<String>, HashMap<String, String>) {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return (None, HashMap::new());
        }
        let raw = Some(trimmed.to_string());
        let mut map = HashMap::new();
        let parsed: Result<JsonValue, _> = serde_json::from_str(trimmed);
        if let Ok(JsonValue::Object(obj)) = parsed {
            for (k, v) in obj {
                match v {
                    JsonValue::String(s) => { map.insert(k, s); }
                    JsonValue::Number(n) => { map.insert(k, n.to_string()); }
                    JsonValue::Bool(b) => { map.insert(k, if b { "true".to_string() } else { "false".to_string() }); }
                    JsonValue::Null => { map.insert(k, "null".to_string()); }
                    _ => {}
                }
            }
        }
        (raw, map)
    }

    pub fn eval_php_expr_ast<'a>(&'a mut self, expr: PhpExpr) -> std::pin::Pin<Box<dyn std::future::Future<Output = RuntimeValue> + Send + 'a>> {
        Box::pin(async move {
            match expr {
                PhpExpr::Var(name) => self.env.get(&name).unwrap_or(RuntimeValue::Null),
                PhpExpr::String(s) => RuntimeValue::StringVal(s),
                PhpExpr::Int(i) => RuntimeValue::IntVal(i),
                PhpExpr::Float(f) => RuntimeValue::FloatVal(f),
                PhpExpr::Bool(b) => RuntimeValue::BoolVal(b),
                PhpExpr::Null => RuntimeValue::Null,
                PhpExpr::ArrayLit(items) => {
                    let mut list = Vec::new();
                    for e in items {
                        list.push(self.eval_php_expr_ast(e).await.to_json());
                    }
                    RuntimeValue::JsonVal(JsonValue::Array(list))
                }
                PhpExpr::ObjectLit(props) => {
                    let mut obj = serde_json::Map::new();
                    for (k, v) in props {
                        obj.insert(k, self.eval_php_expr_ast(v).await.to_json());
                    }
                    RuntimeValue::JsonVal(JsonValue::Object(obj))
                }
                PhpExpr::Index { target, index } => {
                    let t = self.eval_php_expr_ast(*target).await.to_json();
                    let idx_v = self.eval_php_expr_ast(*index).await;
                    match (t, idx_v) {
                        (JsonValue::Array(a), RuntimeValue::IntVal(i)) => {
                            let i = i as usize;
                            a.get(i).cloned().map(RuntimeValue::JsonVal).unwrap_or(RuntimeValue::Null)
                        }
                        (JsonValue::Array(a), RuntimeValue::StringVal(s)) => {
                            let i = s.parse::<usize>().unwrap_or(usize::MAX);
                            a.get(i).cloned().map(RuntimeValue::JsonVal).unwrap_or(RuntimeValue::Null)
                        }
                        (JsonValue::Object(o), RuntimeValue::StringVal(k)) => {
                            o.get(&k).cloned().map(RuntimeValue::JsonVal).unwrap_or(RuntimeValue::Null)
                        }
                        _ => RuntimeValue::Null,
                    }
                }
                PhpExpr::Call { name, args } => {
                    let mut evaled = Vec::new();
                    for a in args {
                        evaled.push(self.eval_php_expr_ast(a).await);
                    }
                    match name.as_str() {
                        "json_encode" => {
                            let v = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            let json_v = v.to_json();
                            let s = serde_json::to_string(&json_v).unwrap_or("null".to_string());
                            RuntimeValue::StringVal(s)
                        }
                        "db_all" => {
                            let model = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            let m = self.php_to_string(model);
                            RuntimeValue::StringVal(self.model_all_json(&m).await)
                        }
                        "db_find" => {
                            let model = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            let idv = evaled.get(1).cloned().unwrap_or(RuntimeValue::Null);
                            let m = self.php_to_string(model);
                            let id = match idv {
                                RuntimeValue::IntVal(i) => i,
                                RuntimeValue::FloatVal(f) => f as i64,
                                RuntimeValue::StringVal(s) => s.parse::<i64>().unwrap_or(0),
                                RuntimeValue::BoolVal(b) => if b { 1 } else { 0 },
                                RuntimeValue::Null => 0,
                                _ => 0,
                            };
                            RuntimeValue::StringVal(self.model_find_json(&m, id).await)
                        }
                        "db_insert" => {
                            let model = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            let m = self.php_to_string(model);
                            let mut placeholders = Vec::new();
                            let values_to_bind: Vec<RuntimeValue> = evaled.into_iter().skip(1).collect();
                            for i in 1..=values_to_bind.len() {
                                placeholders.push(format!("${}", i));
                            }
                            let query = format!("INSERT INTO {} VALUES (NULL, {})", m, placeholders.join(", "));
                            let mut q = sqlx::query(&query);
                            for val in values_to_bind {
                                q = match val {
                                    RuntimeValue::StringVal(s) => q.bind(s),
                                    RuntimeValue::IntVal(i) => q.bind(i),
                                    RuntimeValue::FloatVal(f) => q.bind(f),
                                    RuntimeValue::BoolVal(b) => q.bind(b as i64),
                                    _ => q.bind(None::<String>),
                                };
                            }
                            match q.execute(&self.db_pool).await {
                                Ok(_) => RuntimeValue::BoolVal(true),
                                Err(_) => RuntimeValue::BoolVal(false),
                            }
                        }
                        "isset" => {
                            let v = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            RuntimeValue::BoolVal(!matches!(v, RuntimeValue::Null))
                        }
                        "str" => {
                            let v = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            RuntimeValue::StringVal(self.php_to_string(v))
                        }
                        "int" => {
                            let v = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            match v {
                                RuntimeValue::IntVal(i) => RuntimeValue::IntVal(i),
                                RuntimeValue::FloatVal(f) => RuntimeValue::IntVal(f as i64),
                                RuntimeValue::BoolVal(b) => RuntimeValue::IntVal(if b { 1 } else { 0 }),
                                RuntimeValue::StringVal(s) => RuntimeValue::IntVal(s.trim().parse::<i64>().unwrap_or(0)),
                                RuntimeValue::Null => RuntimeValue::IntVal(0),
                                _ => RuntimeValue::IntVal(0),
                            }
                        }
                        "json_object" => {
                            let mut obj = serde_json::Map::new();
                            let mut i = 0usize;
                            while i + 1 < evaled.len() {
                                let k = self.php_to_string(evaled[i].clone());
                                let v = evaled[i + 1].clone().to_json();
                                obj.insert(k, v);
                                i += 2;
                            }
                            let json_v = JsonValue::Object(obj);
                            let s = serde_json::to_string(&json_v).unwrap_or("{}".to_string());
                            RuntimeValue::StringVal(s)
                        }
                        "json_array" => {
                            let arr: Vec<JsonValue> = evaled.into_iter().map(|v| v.to_json()).collect();
                            let s = serde_json::to_string(&JsonValue::Array(arr)).unwrap_or("[]".to_string());
                            RuntimeValue::StringVal(s)
                        }
                        "hydrate" => {
                            let key = evaled.get(0).cloned().unwrap_or(RuntimeValue::Null);
                            let val = evaled.get(1).cloned().unwrap_or(RuntimeValue::Null);
                            let k = self.php_to_string(key);
                            self.hydration_state.insert(k, val);
                            RuntimeValue::Null
                        }
                        _ => RuntimeValue::Null,
                    }
                }
                PhpExpr::Binary { left, op, right } => {
                    let l = self.eval_php_expr_ast(*left).await;
                    let r = self.eval_php_expr_ast(*right).await;
                    match op {
                        PhpBinOp::Concat => RuntimeValue::StringVal(format!("{}{}", self.php_to_string(l), self.php_to_string(r))),
                        PhpBinOp::Add => match (l, r) {
                            (RuntimeValue::IntVal(a), RuntimeValue::IntVal(b)) => RuntimeValue::IntVal(a + b),
                            (RuntimeValue::FloatVal(a), RuntimeValue::FloatVal(b)) => RuntimeValue::FloatVal(a + b),
                            (RuntimeValue::IntVal(a), RuntimeValue::FloatVal(b)) => RuntimeValue::FloatVal(a as f64 + b),
                            (RuntimeValue::FloatVal(a), RuntimeValue::IntVal(b)) => RuntimeValue::FloatVal(a + b as f64),
                            _ => RuntimeValue::Null,
                        },
                        PhpBinOp::Sub => match (l, r) {
                            (RuntimeValue::IntVal(a), RuntimeValue::IntVal(b)) => RuntimeValue::IntVal(a - b),
                            (RuntimeValue::FloatVal(a), RuntimeValue::FloatVal(b)) => RuntimeValue::FloatVal(a - b),
                            (RuntimeValue::IntVal(a), RuntimeValue::FloatVal(b)) => RuntimeValue::FloatVal(a as f64 - b),
                            (RuntimeValue::FloatVal(a), RuntimeValue::IntVal(b)) => RuntimeValue::FloatVal(a - b as f64),
                            _ => RuntimeValue::Null,
                        },
                        PhpBinOp::EqEq => RuntimeValue::BoolVal(self.php_equals(&l, &r)),
                        PhpBinOp::BangEq => RuntimeValue::BoolVal(!self.php_equals(&l, &r)),
                        PhpBinOp::Less => RuntimeValue::BoolVal(self.php_cmp(&l, &r, |a, b| a < b)),
                        PhpBinOp::LessEq => RuntimeValue::BoolVal(self.php_cmp(&l, &r, |a, b| a <= b)),
                        PhpBinOp::Greater => RuntimeValue::BoolVal(self.php_cmp(&l, &r, |a, b| a > b)),
                        PhpBinOp::GreaterEq => RuntimeValue::BoolVal(self.php_cmp(&l, &r, |a, b| a >= b)),
                        PhpBinOp::AndAnd => RuntimeValue::BoolVal(self.php_truthy(&l) && self.php_truthy(&r)),
                        PhpBinOp::OrOr => RuntimeValue::BoolVal(self.php_truthy(&l) || self.php_truthy(&r)),
                    }
                }
            }
        })
    }

    fn php_truthy(&self, v: &RuntimeValue) -> bool {
        match v {
            RuntimeValue::BoolVal(b) => *b,
            RuntimeValue::IntVal(i) => *i != 0,
            RuntimeValue::FloatVal(f) => *f != 0.0,
            RuntimeValue::StringVal(s) => !s.is_empty(),
            RuntimeValue::JsonVal(v) => !v.is_null() && v != &JsonValue::Bool(false) && v != &JsonValue::String(String::new()),
            RuntimeValue::Null => false,
            _ => false,
        }
    }

    fn php_cmp<F: Fn(f64, f64) -> bool>(&self, a: &RuntimeValue, b: &RuntimeValue, f: F) -> bool {
        let to_num = |v: &RuntimeValue| match v {
            RuntimeValue::IntVal(i) => Some(*i as f64),
            RuntimeValue::FloatVal(x) => Some(*x),
            RuntimeValue::StringVal(s) => s.parse::<f64>().ok(),
            RuntimeValue::BoolVal(b) => Some(if *b { 1.0 } else { 0.0 }),
            RuntimeValue::Null => Some(0.0),
            _ => None,
        };
        match (to_num(a), to_num(b)) {
            (Some(x), Some(y)) => f(x, y),
            _ => false,
        }
    }

    fn php_equals(&self, a: &RuntimeValue, b: &RuntimeValue) -> bool {
        match (a, b) {
            (RuntimeValue::Null, RuntimeValue::Null) => true,
            (RuntimeValue::Null, RuntimeValue::StringVal(s)) => s.is_empty(),
            (RuntimeValue::StringVal(s), RuntimeValue::Null) => s.is_empty(),
            (RuntimeValue::BoolVal(x), RuntimeValue::BoolVal(y)) => x == y,
            (RuntimeValue::IntVal(x), RuntimeValue::IntVal(y)) => x == y,
            (RuntimeValue::FloatVal(x), RuntimeValue::FloatVal(y)) => x == y,
            (RuntimeValue::StringVal(x), RuntimeValue::StringVal(y)) => x == y,
            (RuntimeValue::IntVal(x), RuntimeValue::FloatVal(y)) => (*x as f64) == *y,
            (RuntimeValue::FloatVal(x), RuntimeValue::IntVal(y)) => *x == (*y as f64),
            _ => false,
        }
    }

    fn php_to_string(&self, v: RuntimeValue) -> String {
        match v {
            RuntimeValue::StringVal(s) => s,
            RuntimeValue::IntVal(i) => i.to_string(),
            RuntimeValue::FloatVal(f) => f.to_string(),
            RuntimeValue::BoolVal(b) => if b { "true".to_string() } else { "false".to_string() },
            RuntimeValue::JsonVal(v) => serde_json::to_string(&v).unwrap_or("null".to_string()),
            RuntimeValue::Null => "null".to_string(),
            RuntimeValue::Function { name, .. } => format!("<fn {}>", name),
        }
    }

    fn print_runtime_value(&self, val: RuntimeValue) {
        match val {
            RuntimeValue::StringVal(s) => println!("{}", s),
            RuntimeValue::IntVal(i) => println!("{}", i),
            RuntimeValue::FloatVal(f) => println!("{}", f),
            RuntimeValue::BoolVal(b) => println!("{}", b),
            RuntimeValue::JsonVal(v) => println!("{}", serde_json::to_string(&v).unwrap_or("null".to_string())),
            RuntimeValue::Function { name, .. } => println!("<fn {}>", name),
            RuntimeValue::Null => println!("null"),
        }
    }
}
