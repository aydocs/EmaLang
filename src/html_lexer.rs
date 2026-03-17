use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum HtmlTok {
    Less(Span),
    Greater(Span),
    Slash(Span),
    Eq(Span),
    Ident(String, Span),
    String(String, Span),
    Text(String, Span),
    Bang(Span),
    DashDash(Span),
    EOF(Span),
}

// MVP tokenizer: used by the HTML parser; not intended to be fully spec-compliant.
pub struct HtmlLexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> HtmlLexer<'a> {
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

    fn starts_with(&self, s: &str) -> bool { self.input[self.pos..].starts_with(s) }

    pub fn next(&mut self) -> HtmlTok {
        let sp = self.span();
        if self.pos >= self.input.len() {
            return HtmlTok::EOF(sp);
        }

        let ch = self.cur().unwrap();
        match ch {
            '<' => { self.bump(); HtmlTok::Less(sp) }
            '>' => { self.bump(); HtmlTok::Greater(sp) }
            '/' => { self.bump(); HtmlTok::Slash(sp) }
            '=' => { self.bump(); HtmlTok::Eq(sp) }
            '!' => { self.bump(); HtmlTok::Bang(sp) }
            '"' | '\'' => {
                let q = ch;
                self.bump();
                let mut s = String::new();
                while let Some(c) = self.cur() {
                    if c == q { break; }
                    s.push(c);
                    self.bump();
                }
                if self.cur() == Some(q) { self.bump(); }
                HtmlTok::String(s, sp)
            }
            _ => {
                // Outside of tags, treat as text until '<'
                let mut s = String::new();
                while let Some(c) = self.cur() {
                    if c == '<' { break; }
                    s.push(c);
                    self.bump();
                }
                HtmlTok::Text(s, sp)
            }
        }
    }
}

