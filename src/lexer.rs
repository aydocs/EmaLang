#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Var,
    Print,
    AtServer,
    AtClient,
    Model,     // Native DB Struct
    StrType,   // Native String Type
    IntType,   // Native Integer Type
    FloatType, // Native Float Type
    BoolType,  // Native Boolean Type
    If,
    Else,
    While,
    Fn,        // Function Keyword
    Return,    // Return Value Keyword
    Html,      // embedded block keyword
    Css,       // embedded block keyword
    Js,        // embedded block keyword
    Php,       // embedded block keyword
    Plus, Minus, Star, Slash,
    EqEq, BangEq, Less, LessEq, Greater, GreaterEq,
    Identifier(String),
    StringLit(String),
    IntLit(i64),
    FloatLit(f64),
    HtmlBlock(String),
    CssBlock(String),
    JsBlock(String),
    PhpBlock(String),
    Assign,
    Colon,           // :
    DoubleColon,     // ::
    Semi,
    Comma,     // ,
    LBrace,
    RBrace,
    LParen,    // (
    SlashGreater,    // />
    LDoubleBrace,    // {{
    RDoubleBrace,    // }}
    State,           // state keyword
    RParen,    // )
    EOF,
}

use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct TokenData {
    pub token: Token,
    pub line: usize,
    pub col: usize,
}

impl TokenData {
    pub fn span(&self) -> Span {
        Span { line: self.line, col: self.col }
    }
}

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            col: 1,
        }
    }

    fn current_char(&self) -> Option<char> {
        if self.position >= self.input.len() {
            None
        } else {
            Some(self.input[self.position])
        }
    }

    fn peek_char(&self, offset: usize) -> Option<char> {
        self.input.get(self.position + offset).copied()
    }

    fn advance(&mut self) {
        if let Some(c) = self.current_char() {
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        self.position += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current_char() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_identifier_or_keyword(&mut self) -> Token {
        let mut result = String::new();
        while let Some(c) = self.current_char() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                result.push(c);
                self.advance();
            } else {
                break;
            }
        }

        match result.as_str() {
            "var" => Token::Var,
            "print" => Token::Print,
            "model" => Token::Model,
            "str" => Token::StrType,
            "int" => Token::IntType,
            "float" => Token::FloatType,
            "bool" => Token::BoolType,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "fn" => Token::Fn,
            "return" => Token::Return,
            "state" => Token::State,
            "html" => Token::Html,
            "css" => Token::Css,
            "js" => Token::Js,
            "php" => Token::Php,
            _ => Token::Identifier(result),
        }
    }

    fn read_decorator(&mut self) -> Token {
        self.advance(); // skip '@'
        let mut result = String::new();
        while let Some(c) = self.current_char() {
            if c.is_alphabetic() {
                result.push(c);
                self.advance();
            } else {
                break;
            }
        }

        match result.as_str() {
            "server" => Token::AtServer,
            "client" => Token::AtClient,
            _ => panic!("Unknown decorator: @{}", result),
        }
    }

    fn read_string(&mut self) -> Token {
        self.advance(); // skip opening quote
        let mut result = String::new();
        while let Some(c) = self.current_char() {
            if c == '"' {
                self.advance(); // skip closing quote
                break;
            }
            result.push(c);
            self.advance();
        }
        Token::StringLit(result)
    }

    fn read_number(&mut self) -> Token {
        let mut result = String::new();
        let mut has_dot = false;
        while let Some(c) = self.current_char() {
            if c.is_ascii_digit() {
                result.push(c);
                self.advance();
            } else if c == '.' && !has_dot {
                // Check if next is also dot (for ..)
                if let Some('.') = self.input.get(self.position + 1) {
                    break;
                }
                has_dot = true;
                result.push(c);
                self.advance();
            } else {
                break;
            }
        }
        if has_dot {
            Token::FloatLit(result.parse::<f64>().unwrap())
        } else {
            Token::IntLit(result.parse::<i64>().unwrap())
        }
    }

    fn skip_whitespace_no_newline(&mut self) {
        while let Some(c) = self.current_char() {
            if c == '\n' {
                break;
            }
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_raw_brace_block(&mut self) -> String {
        // Assumes current char is '{'. Consumes the opening '{' and reads until the matching '}'.
        // Supports nested braces and simple string escaping so HTML/CSS/JS/PHP can contain braces.
        if self.current_char() != Some('{') {
            return String::new();
        }
        self.advance(); // consume '{'

        let mut depth: i32 = 1;
        let mut out = String::new();

        let mut in_quote: Option<char> = None; // '"', '\'', '`'
        let mut escaped = false;

        while let Some(c) = self.current_char() {
            if let Some(q) = in_quote {
                out.push(c);
                self.advance();

                if escaped {
                    escaped = false;
                    continue;
                }
                if c == '\\' && q != '`' {
                    escaped = true;
                    continue;
                }
                if c == q {
                    in_quote = None;
                }
                continue;
            }

            match c {
                '"' | '\'' | '`' => {
                    in_quote = Some(c);
                    out.push(c);
                    self.advance();
                }
                '{' => {
                    depth += 1;
                    out.push(c);
                    self.advance();
                }
                '}' => {
                    depth -= 1;
                    self.advance();
                    if depth == 0 {
                        break;
                    }
                    out.push('}');
                }
                _ => {
                    out.push(c);
                    self.advance();
                }
            }
        }

        out
    }

    fn read_heredoc_delimiter(&mut self) -> String {
        // Reads the delimiter right after <<< (until whitespace/newline)
        let mut delim = String::new();
        while let Some(c) = self.current_char() {
            if c.is_whitespace() {
                break;
            }
            delim.push(c);
            self.advance();
        }
        if delim.is_empty() {
            panic!(
                "Line {}:{}: Heredoc delimiter cannot be empty. Example: html <<<HTML\\n...\\nHTML",
                self.line, self.col
            );
        }
        delim
    }

    fn consume_optional_cr(&mut self) {
        if self.current_char() == Some('\r') {
            self.advance();
        }
    }

    fn read_heredoc_body(&mut self, delimiter: &str) -> String {
        // Reads until a line that starts with `delimiter` and is followed by newline/CRLF/EOF.
        // The delimiter line is consumed but not included in output.
        let mut out = String::new();
        let mut at_line_start = true;
        let mut closed = false;

        loop {
            if self.position >= self.input.len() {
                break;
            }

            if at_line_start {
                // Check delimiter match at current position
                let mut matches = true;
                for (i, dc) in delimiter.chars().enumerate() {
                    if self.peek_char(i) != Some(dc) {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    let after = self.peek_char(delimiter.chars().count());
                    if after == Some('\n') || after == Some('\r') || after.is_none() {
                        // consume delimiter
                        for _ in 0..delimiter.chars().count() {
                            self.advance();
                        }
                        self.consume_optional_cr();
                        if self.current_char() == Some('\n') {
                            self.advance();
                        }
                        closed = true;
                        break;
                    }
                }
            }

            let c = self.current_char().unwrap();
            out.push(c);
            self.advance();
            at_line_start = c == '\n';
        }

        if !closed {
            panic!(
                "Line {}:{}: Heredoc terminator not found. Expected a line containing: {}",
                self.line, self.col, delimiter
            );
        }
        out
    }

    pub fn next_token(&mut self) -> TokenData {
        self.skip_whitespace();
        let start_line = self.line;
        let start_col = self.col;

        if self.position >= self.input.len() {
            return TokenData { token: Token::EOF, line: start_line, col: start_col };
        }

        if let Some(c) = self.current_char() {
            if c.is_alphabetic() || c == '_' || c == '-' || c == '#' || c == '%' {
                let token = self.read_identifier_or_keyword();
                // Embedded raw blocks: html { ... }, css { ... }, js { ... }, php { ... }
                return match token {
                    Token::Html | Token::Css | Token::Js | Token::Php => {
                        self.skip_whitespace_no_newline();
                        if self.current_char() == Some('{') {
                            let raw = self.read_raw_brace_block();
                            let block_token = match token {
                                Token::Html => Token::HtmlBlock(raw),
                                Token::Css => Token::CssBlock(raw),
                                Token::Js => Token::JsBlock(raw),
                                Token::Php => Token::PhpBlock(raw),
                                _ => unreachable!(),
                            };
                            return TokenData { token: block_token, line: start_line, col: start_col };
                        }
                        // Heredoc form: html <<<TAG\n...\nTAG
                        if self.current_char() == Some('<')
                            && self.peek_char(1) == Some('<')
                            && self.peek_char(2) == Some('<')
                        {
                            self.advance();
                            self.advance();
                            self.advance();
                            let delimiter = self.read_heredoc_delimiter();
                            self.consume_optional_cr();
                            if self.current_char() == Some('\n') {
                                self.advance();
                            }
                            let raw = self.read_heredoc_body(&delimiter);
                            let block_token = match token {
                                Token::Html => Token::HtmlBlock(raw),
                                Token::Css => Token::CssBlock(raw),
                                Token::Js => Token::JsBlock(raw),
                                Token::Php => Token::PhpBlock(raw),
                                _ => unreachable!(),
                            };
                            return TokenData { token: block_token, line: start_line, col: start_col };
                        }
                        TokenData { token, line: start_line, col: start_col }
                    }
                    _ => TokenData { token, line: start_line, col: start_col },
                }
            }
            if c.is_ascii_digit() {
                let token = self.read_number();
                return TokenData { token, line: start_line, col: start_col };
            }
            let token = match c {
                '@' => self.read_decorator(),
                '"' => self.read_string(),
                '=' => {
                    self.advance();
                    if let Some('=') = self.current_char() {
                        self.advance();
                        Token::EqEq
                    } else {
                        Token::Assign
                    }
                }
                '!' => {
                    self.advance();
                    if let Some('=') = self.current_char() {
                        self.advance();
                        Token::BangEq
                    } else {
                        panic!("Line {}:{}: Beklenmeyen karakter: !", self.line, self.col);
                    }
                }
                '<' => {
                    self.advance();
                    if let Some('=') = self.current_char() {
                        self.advance();
                        Token::LessEq
                    } else {
                        Token::Less
                    }
                }
                '>' => {
                    self.advance();
                    if let Some('=') = self.current_char() {
                        self.advance();
                        Token::GreaterEq
                    } else {
                        Token::Greater
                    }
                }
                '+' => {
                    self.advance();
                    Token::Plus
                }
                '-' => {
                    self.advance();
                    Token::Minus
                }
                '*' => {
                    self.advance();
                    Token::Star
                }
                '/' => {
                    self.advance();
                    if let Some('/') = self.current_char() {
                        while let Some(c) = self.current_char() {
                            self.advance();
                            if c == '\n' {
                                break;
                            }
                        }
                        return self.next_token();
                    }
                    if let Some('>') = self.current_char() {
                        self.advance();
                        Token::SlashGreater
                    } else {
                        Token::Slash
                    }
                }
                ':' => {
                    self.advance();
                    if let Some(':') = self.current_char() {
                        self.advance();
                        Token::DoubleColon
                    } else {
                        Token::Colon
                    }
                }
                ';' => {
                    self.advance();
                    Token::Semi
                }
                '{' => {
                    self.advance();
                    if let Some('{') = self.current_char() {
                        self.advance();
                        Token::LDoubleBrace
                    } else {
                        Token::LBrace
                    }
                }
                '}' => {
                    self.advance();
                    if let Some('}') = self.current_char() {
                        self.advance();
                        Token::RDoubleBrace
                    } else {
                        Token::RBrace
                    }
                }
                '(' => {
                    self.advance();
                    Token::LParen
                }
                ')' => {
                    self.advance();
                    Token::RParen
                }
                ',' => {
                    self.advance();
                    Token::Comma
                }
                _ => panic!("Line {}:{}: Beklenmeyen karakter: {}", self.line, self.col, c),
            };
            return TokenData { token, line: start_line, col: start_col };
        }

        TokenData { token: Token::EOF, line: start_line, col: start_col }
    }
}
