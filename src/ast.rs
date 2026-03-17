#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddedKind {
    Html,
    Css,
    Js,
    Php,
}

// --- Full webstack AST nodes (parsed forms) ---
#[derive(Debug, Clone, PartialEq)]
pub enum HtmlNode {
    Element {
        tag: String,
        attrs: Vec<HtmlAttr>,
        children: Vec<HtmlNode>,
        span: Span,
    },
    Text {
        text: String,
        span: Span,
    },
    Interpolation {
        expr: Box<Expr>,
        span: Span,
    },
    If {
        condition: Box<Expr>,
        then_children: Vec<HtmlNode>,
        else_children: Vec<HtmlNode>,
        span: Span,
    },
    ForEach {
        item: String,
        index: Option<String>,
        list: Box<Expr>,
        body: Vec<HtmlNode>,
        span: Span,
    },
    Comment {
        text: String,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct HtmlAttr {
    pub name: String,
    pub value: HtmlAttrValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HtmlAttrValue {
    Static(String),
    Template(Vec<HtmlNode>),
    BoolTrue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssStylesheet {
    pub rules: Vec<CssRule>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssRule {
    pub selectors: Vec<String>,
    pub declarations: Vec<(String, String)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsProgram {
    pub body: Vec<JsStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsVarKind {
    Const,
    Let,
    Var,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsParam {
    pub name: String,
    pub default: Option<JsExpr>,
    pub is_rest: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsPattern {
    Ident(String, Span),
    Object {
        props: Vec<(String, Option<String>)>, // key, alias
        rest: Option<String>,
        span: Span,
    },
    Array {
        items: Vec<Option<String>>, // identifiers, None = hole
        rest: Option<String>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsStmt {
    Expr(JsExpr, Span),
    VarDecl { kind: JsVarKind, pattern: JsPattern, value: Option<JsExpr>, span: Span },
    Return(Option<JsExpr>, Span),
    Block { body: Vec<JsStmt>, span: Span },
    If { condition: JsExpr, then_branch: Box<JsStmt>, else_branch: Option<Box<JsStmt>>, span: Span },
    While { condition: JsExpr, body: Box<JsStmt>, span: Span },
    For {
        init: Option<Box<JsStmt>>,
        condition: Option<JsExpr>,
        update: Option<JsExpr>,
        body: Box<JsStmt>,
        span: Span,
    },
    FunctionDecl { name: String, params: Vec<JsParam>, body: Box<JsStmt>, span: Span },
    TryCatch { try_block: Box<JsStmt>, catch_name: String, catch_block: Box<JsStmt>, span: Span },
    Throw(JsExpr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsTemplatePart {
    Str(String),
    Expr(JsExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsExpr {
    Ident(String, Span),
    String(String, Span),
    Number(f64, Span),
    Bool(bool, Span),
    Null(Span),
    Call { callee: Box<JsExpr>, args: Vec<JsExpr>, span: Span },
    Member { object: Box<JsExpr>, property: String, span: Span },
    Binary { left: Box<JsExpr>, op: String, right: Box<JsExpr>, span: Span },
    Unary { op: String, expr: Box<JsExpr>, span: Span },
    Assign { target: Box<JsExpr>, value: Box<JsExpr>, span: Span },
    Conditional { condition: Box<JsExpr>, then_expr: Box<JsExpr>, else_expr: Box<JsExpr>, span: Span },
    ObjectLit { props: Vec<(String, JsExpr)>, span: Span },
    ArrayLit { items: Vec<JsExpr>, span: Span },
    ArrowFn { params: Vec<JsParam>, body: Box<JsStmt>, span: Span },
    Spread { expr: Box<JsExpr>, span: Span },
    TemplateLit { parts: Vec<JsTemplatePart>, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhpProgram {
    pub body: Vec<PhpStmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PhpStmt {
    Echo(PhpExpr, Span),
    Print(PhpExpr, Span),
    Expr(PhpExpr, Span),
    Assign { name: String, value: PhpExpr, span: Span },
    If { condition: PhpExpr, then_branch: Vec<PhpStmt>, else_branch: Option<Vec<PhpStmt>>, span: Span },
    While { condition: PhpExpr, body: Vec<PhpStmt>, span: Span },
    For {
        init: Option<Box<PhpStmt>>,
        condition: Option<PhpExpr>,
        update: Option<PhpExpr>,
        body: Vec<PhpStmt>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PhpExpr {
    Var(String, Span),
    String(String, Span),
    Int(i64, Span),
    Float(f64, Span),
    Bool(bool, Span),
    Null(Span),
    Call { name: String, args: Vec<PhpExpr>, span: Span },
    Binary { left: Box<PhpExpr>, op: String, right: Box<PhpExpr>, span: Span },
    ArrayLit { items: Vec<PhpExpr>, span: Span },
    ObjectLit { props: Vec<(String, PhpExpr)>, span: Span },
    Index { target: Box<PhpExpr>, index: Box<PhpExpr>, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Identifier(String, Span),
    StringLit(String, Span),
    IntLit(i64, Span),
    FloatLit(f64, Span),
    BoolLit(bool, Span),
    EmbeddedBlock {
        kind: EmbeddedKind,
        raw: String,
        span: Span,
    },
    HtmlAst {
        root: HtmlNode,
        span: Span,
    },
    CssAst {
        stylesheet: CssStylesheet,
        span: Span,
    },
    JsAst {
        program: JsProgram,
        span: Span,
    },
    PhpAst {
        program: PhpProgram,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    NamespaceCall {
        namespace: String,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    UiElement {
        tag: String,
        props: HashMap<String, Expr>,
        children: Vec<Expr>,
        span: Span,
    },
    Interpolation(Box<Expr>, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub field_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VarDecl { name: String, value: Expr, span: Span },
    PrintStmt(Expr, Span),
    ExprStmt(Expr, Span),
    StateDecl { name: String, value: Expr, span: Span },
    ServerBlock(Vec<Stmt>, Span),
    ClientBlock(Vec<Stmt>, Span),
    ModelDecl { name: String, fields: Vec<FieldDecl>, span: Span },
    IfStmt {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
        span: Span,
    },
    WhileStmt {
        condition: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    FnDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
        span: Span,
    },
    ReturnStmt(Option<Expr>, Span),
}

use std::collections::HashMap;

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Identifier(_, s) | Expr::StringLit(_, s) | Expr::IntLit(_, s) |
            Expr::FloatLit(_, s) | Expr::BoolLit(_, s) | Expr::Interpolation(_, s) => s.clone(),
            Expr::EmbeddedBlock { span, .. } => span.clone(),
            Expr::HtmlAst { span, .. } | Expr::CssAst { span, .. } | Expr::JsAst { span, .. } | Expr::PhpAst { span, .. } => span.clone(),
            Expr::Call { span, .. } | Expr::NamespaceCall { span, .. } |
            Expr::Binary { span, .. } | Expr::UiElement { span, .. } => span.clone(),
        }
    }
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::VarDecl { span, .. } | Stmt::PrintStmt(_, span) | Stmt::ExprStmt(_, span) |
            Stmt::StateDecl { span, .. } | Stmt::ServerBlock(_, span) | Stmt::ClientBlock(_, span) |
            Stmt::ModelDecl { span, .. } | Stmt::IfStmt { span, .. } | Stmt::WhileStmt { span, .. } |
            Stmt::FnDecl { span, .. } | Stmt::ReturnStmt(_, span) => span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add, Sub, Mul, Div,
    EqEq, BangEq, Less, LessEq, Greater, GreaterEq,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Str,
    Bool,
    Custom(String),
}
