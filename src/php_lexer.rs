use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum PhpTok {
    DollarIdent(String, Span),
    Ident(String, Span),
    String(String, Span),
    Int(i64, Span),
    Float(f64, Span),
    True(Span),
    False(Span),
    Echo(Span),
    Print(Span),
    If(Span),
    Else(Span),
    ElseIf(Span),
    While(Span),
    For(Span),
    LParen(Span),
    RParen(Span),
    LBrace(Span),
    RBrace(Span),
    LBracket(Span),
    RBracket(Span),
    Comma(Span),
    Semi(Span),
    Assign(Span),
    Dot(Span),
    Plus(Span),
    Minus(Span),
    Colon(Span),
    Less(Span),
    LessEq(Span),
    Greater(Span),
    GreaterEq(Span),
    EqEq(Span),
    BangEq(Span),
    AndAnd(Span),
    OrOr(Span),
    EOF(Span),
}

pub struct PhpLexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> PhpLexer<'a> {
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

    fn skip_ws(&mut self) {
        while matches!(self.cur(), Some(c) if c.is_whitespace()) { self.bump(); }
    }

    fn read_while<F: Fn(char) -> bool>(&mut self, f: F) -> String {
        let mut s = String::new();
        while let Some(c) = self.cur() {
            if !f(c) { break; }
            s.push(c);
            self.bump();
        }
        s
    }

    fn read_string(&mut self, q: char) -> String {
        let mut s = String::new();
        while let Some(c) = self.cur() {
            if c == q { break; }
            s.push(c);
            self.bump();
        }
        s
    }

    pub fn next(&mut self) -> PhpTok {
        self.skip_ws();
        let sp = self.span();
        if self.pos >= self.input.len() { return PhpTok::EOF(sp); }
        let c = self.cur().unwrap();
        match c {
            '(' => { self.bump(); PhpTok::LParen(sp) }
            ')' => { self.bump(); PhpTok::RParen(sp) }
            '{' => { self.bump(); PhpTok::LBrace(sp) }
            '}' => { self.bump(); PhpTok::RBrace(sp) }
            '[' => { self.bump(); PhpTok::LBracket(sp) }
            ']' => { self.bump(); PhpTok::RBracket(sp) }
            ',' => { self.bump(); PhpTok::Comma(sp) }
            ';' => { self.bump(); PhpTok::Semi(sp) }
            '.' => { self.bump(); PhpTok::Dot(sp) }
            ':' => { self.bump(); PhpTok::Colon(sp) }
            '+' => { self.bump(); PhpTok::Plus(sp) }
            '-' => { self.bump(); PhpTok::Minus(sp) }
            '<' => {
                self.bump();
                if self.cur() == Some('=') { self.bump(); PhpTok::LessEq(sp) } else { PhpTok::Less(sp) }
            }
            '>' => {
                self.bump();
                if self.cur() == Some('=') { self.bump(); PhpTok::GreaterEq(sp) } else { PhpTok::Greater(sp) }
            }
            '&' => {
                self.bump();
                if self.cur() == Some('&') { self.bump(); PhpTok::AndAnd(sp) } else { self.next() }
            }
            '|' => {
                self.bump();
                if self.cur() == Some('|') { self.bump(); PhpTok::OrOr(sp) } else { self.next() }
            }
            '=' => {
                self.bump();
                if self.cur() == Some('=') { self.bump(); PhpTok::EqEq(sp) } else { PhpTok::Assign(sp) }
            }
            '!' => {
                self.bump();
                if self.cur() == Some('=') { self.bump(); PhpTok::BangEq(sp) } else { self.next() }
            }
            '"' | '\'' => {
                let q = c;
                self.bump();
                let s = self.read_string(q);
                if self.cur() == Some(q) { self.bump(); }
                PhpTok::String(s, sp)
            }
            '$' => {
                self.bump();
                let name = self.read_while(|ch| ch.is_alphanumeric() || ch == '_');
                PhpTok::DollarIdent(name, sp)
            }
            ch if ch.is_ascii_digit() => {
                let num = self.read_while(|x| x.is_ascii_digit() || x == '.');
                if num.contains('.') {
                    PhpTok::Float(num.parse::<f64>().unwrap_or(0.0), sp)
                } else {
                    PhpTok::Int(num.parse::<i64>().unwrap_or(0), sp)
                }
            }
            ch if ch.is_alphabetic() || ch == '_' => {
                let ident = self.read_while(|x| x.is_alphanumeric() || x == '_');
                match ident.as_str() {
                    "true" => PhpTok::True(sp),
                    "false" => PhpTok::False(sp),
                    "echo" => PhpTok::Echo(sp),
                    "print" => PhpTok::Print(sp),
                    "if" => PhpTok::If(sp),
                    "elseif" => PhpTok::ElseIf(sp),
                    "else" => PhpTok::Else(sp),
                    "while" => PhpTok::While(sp),
                    "for" => PhpTok::For(sp),
                    _ => PhpTok::Ident(ident, sp),
                }
            }
            _ => { self.bump(); self.next() }
        }
    }
}

