use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum CssTok {
    Ident(String, Span),
    LBrace(Span),
    RBrace(Span),
    Colon(Span),
    Semi(Span),
    Comma(Span),
    Hash(Span),
    Dot(Span),
    AtDirective(String, Span),
    String(String, Span),
    Other(char, Span),
    EOF(Span),
}

pub struct CssLexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> CssLexer<'a> {
    pub fn new(input: &'a str, start: Span) -> Self {
        Self { input, pos: 0, line: start.line, col: start.col }
    }

    fn span(&self) -> Span { Span { line: self.line, col: self.col } }
    fn cur(&self) -> Option<char> { self.input[self.pos..].chars().next() }
    fn bump(&mut self) -> Option<char> {
        let ch = self.cur()?;
        self.pos += ch.len_utf8();
        if ch == '\n' { self.line += 1; self.col = 1; } else { self.col += 1; }
        Some(ch)
    }

    pub fn next(&mut self) -> CssTok {
        while let Some(c) = self.cur() {
            if c.is_whitespace() { self.bump(); continue; }
            // skip /* ... */
            if self.input[self.pos..].starts_with("/*") {
                self.bump(); self.bump();
                while self.pos < self.input.len() && !self.input[self.pos..].starts_with("*/") { self.bump(); }
                if self.input[self.pos..].starts_with("*/") { self.bump(); self.bump(); }
                continue;
            }
            break;
        }
        let sp = self.span();
        if self.pos >= self.input.len() { return CssTok::EOF(sp); }
        let c = self.cur().unwrap();
        match c {
            '{' => { self.bump(); CssTok::LBrace(sp) }
            '}' => { self.bump(); CssTok::RBrace(sp) }
            ':' => { self.bump(); CssTok::Colon(sp) }
            ';' => { self.bump(); CssTok::Semi(sp) }
            ',' => { self.bump(); CssTok::Comma(sp) }
            '#' => { self.bump(); CssTok::Hash(sp) }
            '.' => { self.bump(); CssTok::Dot(sp) }
            '@' => {
                self.bump();
                let mut s = String::new();
                while let Some(ch) = self.cur() {
                    if ch.is_alphanumeric() || ch == '-' || ch == '_' { s.push(ch); self.bump(); } else { break; }
                }
                CssTok::AtDirective(s, sp)
            }
            '"' | '\'' => {
                let q = c; self.bump();
                let mut s = String::new();
                while let Some(ch) = self.cur() {
                    if ch == q { break; }
                    s.push(ch); self.bump();
                }
                if self.cur() == Some(q) { self.bump(); }
                CssTok::String(s, sp)
            }
            ch if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
                let mut s = String::new();
                while let Some(ch) = self.cur() {
                    if ch.is_alphanumeric() || ch == '-' || ch == '_' { s.push(ch); self.bump(); } else { break; }
                }
                CssTok::Ident(s, sp)
            }
            other => { self.bump(); CssTok::Other(other, sp) }
        }
    }
}

