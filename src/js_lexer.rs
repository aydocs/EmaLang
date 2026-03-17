use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum JsTok {
    Ident(String, Span),
    String(String, Span),
    Number(f64, Span),
    True(Span),
    False(Span),
    Null(Span),
    Const(Span),
    Let(Span),
    Var(Span),
    If(Span),
    Else(Span),
    For(Span),
    While(Span),
    Return(Span),
    Function(Span),
    Try(Span),
    Catch(Span),
    Throw(Span),
    LParen(Span),
    RParen(Span),
    LBrace(Span),
    RBrace(Span),
    LBracket(Span),
    RBracket(Span),
    Dot(Span),
    Ellipsis(Span),
    Comma(Span),
    Semi(Span),
    Colon(Span),
    Question(Span),
    Plus(Span),
    Minus(Span),
    Star(Span),
    Slash(Span),
    Bang(Span),
    Eq(Span),
    EqEq(Span),
    BangEq(Span),
    Less(Span),
    LessEq(Span),
    Greater(Span),
    GreaterEq(Span),
    AndAnd(Span),
    OrOr(Span),
    Arrow(Span),
    Backtick(Span),
    TemplateChunk(String, Span),
    DollarLBrace(Span), // ${
    EOF(Span),
}

pub struct JsLexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    col: usize,
    mode: JsMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsMode {
    Normal,
    TemplateText,
    TemplateExpr { brace_depth: i32 },
}

impl<'a> JsLexer<'a> {
    pub fn new(input: &'a str, start: Span) -> Self {
        Self { input, pos: 0, line: start.line, col: start.col, mode: JsMode::Normal }
    }

    fn span(&self) -> Span { Span { line: self.line, col: self.col } }
    fn cur(&self) -> Option<char> { self.input[self.pos..].chars().next() }
    fn bump(&mut self) -> Option<char> {
        let ch = self.cur()?;
        self.pos += ch.len_utf8();
        if ch == '\n' { self.line += 1; self.col = 1; } else { self.col += 1; }
        Some(ch)
    }

    fn skip_ws(&mut self) {
        loop {
            while matches!(self.cur(), Some(c) if c.is_whitespace()) { self.bump(); }
            if self.input[self.pos..].starts_with("//") {
                while let Some(c) = self.cur() { self.bump(); if c == '\n' { break; } }
                continue;
            }
            if self.input[self.pos..].starts_with("/*") {
                self.bump(); self.bump();
                while self.pos < self.input.len() && !self.input[self.pos..].starts_with("*/") { self.bump(); }
                if self.input[self.pos..].starts_with("*/") { self.bump(); self.bump(); }
                continue;
            }
            break;
        }
    }

    pub fn next(&mut self) -> JsTok {
        // Template literal scanning (outside of normal ws/comments).
        if self.mode == JsMode::TemplateText {
            let sp = self.span();
            if self.pos >= self.input.len() {
                self.mode = JsMode::Normal;
                return JsTok::EOF(sp);
            }
            if self.input[self.pos..].starts_with('`') {
                self.bump();
                self.mode = JsMode::Normal;
                return JsTok::Backtick(sp);
            }
            if self.input[self.pos..].starts_with("${") {
                self.bump();
                self.bump();
                self.mode = JsMode::TemplateExpr { brace_depth: 1 };
                return JsTok::DollarLBrace(sp);
            }
            let mut s = String::new();
            while self.pos < self.input.len() {
                if self.input[self.pos..].starts_with('`') || self.input[self.pos..].starts_with("${") {
                    break;
                }
                if let Some(ch) = self.bump() {
                    s.push(ch);
                } else {
                    break;
                }
            }
            return JsTok::TemplateChunk(s, sp);
        }

        self.skip_ws();
        let sp = self.span();
        if self.pos >= self.input.len() { return JsTok::EOF(sp); }
        let c = self.cur().unwrap();
        match c {
            '`' => {
                self.bump();
                self.mode = JsMode::TemplateText;
                JsTok::Backtick(sp)
            }
            '(' => { self.bump(); JsTok::LParen(sp) }
            ')' => { self.bump(); JsTok::RParen(sp) }
            '{' => {
                self.bump();
                if let JsMode::TemplateExpr { brace_depth } = self.mode {
                    self.mode = JsMode::TemplateExpr { brace_depth: brace_depth + 1 };
                }
                JsTok::LBrace(sp)
            }
            '}' => {
                self.bump();
                if let JsMode::TemplateExpr { brace_depth } = self.mode {
                    let next_depth = brace_depth - 1;
                    if next_depth <= 0 {
                        self.mode = JsMode::TemplateText;
                    } else {
                        self.mode = JsMode::TemplateExpr { brace_depth: next_depth };
                    }
                }
                JsTok::RBrace(sp)
            }
            '[' => { self.bump(); JsTok::LBracket(sp) }
            ']' => { self.bump(); JsTok::RBracket(sp) }
            '.' => {
                if self.input[self.pos..].starts_with("...") {
                    self.bump(); self.bump(); self.bump();
                    JsTok::Ellipsis(sp)
                } else {
                    self.bump();
                    JsTok::Dot(sp)
                }
            }
            ',' => { self.bump(); JsTok::Comma(sp) }
            ';' => { self.bump(); JsTok::Semi(sp) }
            ':' => { self.bump(); JsTok::Colon(sp) }
            '?' => { self.bump(); JsTok::Question(sp) }
            '+' => { self.bump(); JsTok::Plus(sp) }
            '-' => { self.bump(); JsTok::Minus(sp) }
            '*' => { self.bump(); JsTok::Star(sp) }
            '/' => { self.bump(); JsTok::Slash(sp) }
            '!' => {
                self.bump();
                if self.cur() == Some('=') { self.bump(); JsTok::BangEq(sp) } else { JsTok::Bang(sp) }
            }
            '=' => {
                self.bump();
                if self.cur() == Some('=') {
                    self.bump();
                    JsTok::EqEq(sp)
                } else if self.cur() == Some('>') {
                    self.bump();
                    JsTok::Arrow(sp)
                } else {
                    JsTok::Eq(sp)
                }
            }
            '<' => {
                self.bump();
                if self.cur() == Some('=') { self.bump(); JsTok::LessEq(sp) } else { JsTok::Less(sp) }
            }
            '>' => {
                self.bump();
                if self.cur() == Some('=') { self.bump(); JsTok::GreaterEq(sp) } else { JsTok::Greater(sp) }
            }
            '&' => {
                self.bump();
                if self.cur() == Some('&') { self.bump(); JsTok::AndAnd(sp) } else { self.next() }
            }
            '|' => {
                self.bump();
                if self.cur() == Some('|') { self.bump(); JsTok::OrOr(sp) } else { self.next() }
            }
            '"' | '\'' => {
                let q = c; self.bump();
                let mut s = String::new();
                while let Some(ch) = self.cur() {
                    if ch == q { break; }
                    s.push(ch); self.bump();
                }
                if self.cur() == Some(q) { self.bump(); }
                JsTok::String(s, sp)
            }
            ch if ch.is_ascii_digit() => {
                let mut s = String::new();
                while let Some(ch) = self.cur() {
                    if ch.is_ascii_digit() || ch == '.' { s.push(ch); self.bump(); } else { break; }
                }
                JsTok::Number(s.parse::<f64>().unwrap_or(0.0), sp)
            }
            ch if ch.is_alphabetic() || ch == '_' || ch == '$' => {
                let mut s = String::new();
                while let Some(ch) = self.cur() {
                    if ch.is_alphanumeric() || ch == '_' || ch == '$' { s.push(ch); self.bump(); } else { break; }
                }
                match s.as_str() {
                    "true" => JsTok::True(sp),
                    "false" => JsTok::False(sp),
                    "null" => JsTok::Null(sp),
                    "const" => JsTok::Const(sp),
                    "let" => JsTok::Let(sp),
                    "var" => JsTok::Var(sp),
                    "if" => JsTok::If(sp),
                    "else" => JsTok::Else(sp),
                    "for" => JsTok::For(sp),
                    "while" => JsTok::While(sp),
                    "return" => JsTok::Return(sp),
                    "function" => JsTok::Function(sp),
                    "try" => JsTok::Try(sp),
                    "catch" => JsTok::Catch(sp),
                    "throw" => JsTok::Throw(sp),
                    _ => JsTok::Ident(s, sp),
                }
            }
            _ => { self.bump(); self.next() }
        }
    }
}

