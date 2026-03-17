use crate::ast::{Stmt, Expr, Program, Type, FieldDecl, BinaryOp, EmbeddedKind};
use crate::css_parser::parse_css;
use crate::html_parser::parse_html;
use crate::js_parser::parse_js;
use crate::php_parser::parse_php;
use crate::lexer::{Token, TokenData};
use std::collections::HashMap;

pub struct Parser {
    tokens: Vec<TokenData>,
    pos: usize,
    in_tag_start: bool,
}

impl Parser {
    pub fn new(tokens: Vec<TokenData>) -> Self {
        Parser { tokens, pos: 0, in_tag_start: false }
    }

    fn current(&self) -> &TokenData {
        static EOF_TOKEN: TokenData = TokenData { token: Token::EOF, line: 0, col: 0 };
        self.tokens.get(self.pos).unwrap_or(&EOF_TOKEN)
    }

    fn eat(&mut self, expected: Token) {
        if self.current().token == expected {
            self.pos += 1;
        } else {
            let cur = self.current();
            panic!(
                "Synchronization error! (Line {}:{}) Expected: {:?}, Found: {:?}",
                cur.line, cur.col, expected, cur.token
            );
        }
    }

    pub fn parse(&mut self) -> Program {
        let mut statements = Vec::new();
        while self.current().token != Token::EOF {
            statements.push(self.parse_statement());
        }
        Program { statements }
    }

    fn parse_statement(&mut self) -> Stmt {
        match self.current().token {
            Token::Var => self.parse_var_decl(),
            Token::State => self.parse_state_decl(),
            Token::Print => self.parse_print_stmt(),
            Token::AtServer => self.parse_server_block(),
            Token::AtClient => self.parse_client_block(),
            Token::Model => self.parse_model_decl(),
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::Fn => self.parse_fn_decl(),
            Token::Return => self.parse_return_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_expr_stmt(&mut self) -> Stmt {
        let start_span = self.current().span();
        let expr = self.parse_expression();
        self.eat(Token::Semi);
        Stmt::ExprStmt(expr, start_span)
    }

    fn parse_var_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Var);
        
        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Expected variable name!"),
        };
        self.pos += 1;

        self.eat(Token::Assign);
        let value = self.parse_expression();
        self.eat(Token::Semi);

        Stmt::VarDecl { name, value, span }
    }

    fn parse_state_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::State);
        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("State ismi bekleniyor!"),
        };
        self.pos += 1;
        self.eat(Token::Assign);
        let value = self.parse_expression();
        self.eat(Token::Semi);
        Stmt::StateDecl { name, value, span }
    }

    fn parse_print_stmt(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Print);
        let expr = self.parse_expression();
        self.eat(Token::Semi);
        Stmt::PrintStmt(expr, span)
    }

    fn parse_server_block(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::AtServer);
        self.eat(Token::LBrace);
        let mut stmts = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            stmts.push(self.parse_statement());
        }
        self.eat(Token::RBrace);
        Stmt::ServerBlock(stmts, span)
    }

    fn parse_model_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Model);

        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Model ismi bekleniyor!"),
        };
        self.pos += 1;

        self.eat(Token::LBrace);
        
        let mut fields = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            let f_span = self.current().span();
            let field_name = match &self.current().token {
                Token::Identifier(n) => n.clone(),
                _ => panic!("Alan ismi (Field identifier) bekleniyor!"),
            };
            self.pos += 1;

            self.eat(Token::Colon);

            let field_type = match &self.current().token {
                Token::StrType => Type::Str,
                Token::IntType => Type::Int,
                Token::FloatType => Type::Float,
                Token::BoolType => Type::Bool,
                Token::Identifier(n) => Type::Custom(n.clone()),
                _ => panic!("Gecersiz alan tipi (Invalid field type)!"),
            };
            self.pos += 1;

            fields.push(FieldDecl { name: field_name, field_type, span: f_span });
        }

        self.eat(Token::RBrace);
        Stmt::ModelDecl { name, fields, span }
    }

    fn parse_client_block(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::AtClient);
        self.eat(Token::LBrace);
        let mut stmts = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            stmts.push(self.parse_statement());
        }
        self.eat(Token::RBrace);
        Stmt::ClientBlock(stmts, span)
    }

    fn parse_if_stmt(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::If);
        let condition = self.parse_expression();
        
        self.eat(Token::LBrace);
        let mut then_branch = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            then_branch.push(self.parse_statement());
        }
        self.eat(Token::RBrace);

        let mut else_branch = None;
        if self.current().token == Token::Else {
            self.eat(Token::Else);
            self.eat(Token::LBrace);
            let mut el_branch = Vec::new();
            while self.current().token != Token::RBrace && self.current().token != Token::EOF {
                el_branch.push(self.parse_statement());
            }
            self.eat(Token::RBrace);
            else_branch = Some(el_branch);
        }

        Stmt::IfStmt { condition, then_branch, else_branch, span }
    }

    fn parse_while_stmt(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::While);
        let condition = self.parse_expression();

        self.eat(Token::LBrace);
        let mut body = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            body.push(self.parse_statement());
        }
        self.eat(Token::RBrace);

        Stmt::WhileStmt { condition, body, span }
    }

    fn parse_fn_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Fn);
        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Fonksiyon ismi bekleniyor!"),
        };
        self.pos += 1;

        self.eat(Token::LParen);
        let mut params = Vec::new();
        if self.current().token != Token::RParen {
            loop {
                let param_name = match &self.current().token {
                    Token::Identifier(n) => n.clone(),
                    _ => panic!("Parametre ismi bekleniyor!"),
                };
                params.push(param_name);
                self.pos += 1;

                if self.current().token == Token::Comma {
                    self.eat(Token::Comma);
                } else {
                    break;
                }
            }
        }
        self.eat(Token::RParen);

        self.eat(Token::LBrace);
        let mut body = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            body.push(self.parse_statement());
        }
        self.eat(Token::RBrace);

        Stmt::FnDecl { name, params, body, span }
    }

    fn parse_return_stmt(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Return);
        let value = if self.current().token != Token::Semi {
            Some(self.parse_expression())
        } else {
            None
        };
        self.eat(Token::Semi);
        Stmt::ReturnStmt(value, span)
    }

    fn parse_expression(&mut self) -> Expr {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut expr = self.parse_term();

        while match &self.current().token {
            Token::EqEq | Token::BangEq | Token::LessEq | Token::GreaterEq => true,
            Token::Less => {
                !matches!(self.tokens.get(self.pos + 1), Some(TokenData { token: Token::Slash, .. }))
            }
            Token::Greater => {
                !self.in_tag_start
            }
            _ => false,
        } {
            let start_span = self.current().span();
            let op = match &self.current().token {
                Token::EqEq => BinaryOp::EqEq,
                Token::BangEq => BinaryOp::BangEq,
                Token::Less => BinaryOp::Less,
                Token::LessEq => BinaryOp::LessEq,
                Token::Greater => BinaryOp::Greater,
                Token::GreaterEq => BinaryOp::GreaterEq,
                token => panic!("Unknown operator: {:?}", token),
            };
            self.pos += 1;
            let right = self.parse_term();
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span: start_span,
            };
        }
        expr
    }

    fn parse_term(&mut self) -> Expr {
        let mut expr = self.parse_primary();

        while match self.current().token {
            Token::Plus | Token::Minus | Token::Star | Token::Slash => true,
            _ => false,
        } {
            let start_span = self.current().span();
            let op = match self.current().token {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                _ => unreachable!(),
            };
            self.pos += 1;
            let right = self.parse_primary();
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span: start_span,
            };
        }
        expr
    }

    fn parse_primary(&mut self) -> Expr {
        let span = self.current().span();
        match &self.current().token {
            Token::IntLit(val) => {
                let v = *val;
                self.pos += 1;
                Expr::IntLit(v, span)
            }
            Token::FloatLit(val) => {
                let v = *val;
                self.pos += 1;
                Expr::FloatLit(v, span)
            }
            Token::StringLit(s) => {
                let s_val = s.clone();
                self.pos += 1;
                Expr::StringLit(s_val, span)
            }
            Token::LParen => {
                self.eat(Token::LParen);
                let expr = self.parse_expression();
                self.eat(Token::RParen);
                expr
            }
            Token::LDoubleBrace => {
                self.eat(Token::LDoubleBrace);
                let expr = self.parse_expression();
                self.eat(Token::RDoubleBrace);
                Expr::Interpolation(Box::new(expr), span)
            }
            Token::HtmlBlock(raw) => {
                let raw_val = raw.clone();
                self.pos += 1;
                match parse_html(&raw_val, span.clone()) {
                    Ok(root) => Expr::HtmlAst { root, span },
                    Err(_) => Expr::EmbeddedBlock { kind: EmbeddedKind::Html, raw: raw_val, span },
                }
            }
            Token::CssBlock(raw) => {
                let raw_val = raw.clone();
                self.pos += 1;
                match parse_css(&raw_val, span.clone()) {
                    Ok(stylesheet) => Expr::CssAst { stylesheet, span },
                    Err(_) => Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw: raw_val, span },
                }
            }
            Token::JsBlock(raw) => {
                let raw_val = raw.clone();
                self.pos += 1;
                match parse_js(&raw_val, span.clone()) {
                    Ok(program) => Expr::JsAst { program, span },
                    Err(_) => Expr::EmbeddedBlock { kind: EmbeddedKind::Js, raw: raw_val, span },
                }
            }
            Token::PhpBlock(raw) => {
                let raw_val = raw.clone();
                self.pos += 1;
                match parse_php(&raw_val, span.clone()) {
                    Ok(program) => Expr::PhpAst { program, span },
                    Err(_) => Expr::EmbeddedBlock { kind: EmbeddedKind::Php, raw: raw_val, span },
                }
            }
            Token::Identifier(i) => {
                let identifier_str = i.clone();
                let mut expr = if identifier_str == "true" { 
                    Expr::BoolLit(true, span.clone()) 
                } else if identifier_str == "false" {
                    Expr::BoolLit(false, span.clone())
                } else { 
                    Expr::Identifier(identifier_str.clone(), span.clone()) 
                };
                self.pos += 1;

                if self.current().token == Token::DoubleColon {
                    self.eat(Token::DoubleColon);
                    let mut ns = vec![identifier_str];
                    
                    let method_name;
                    loop {
                        match &self.current().token {
                            Token::Identifier(sub) => {
                                let n = sub.clone();
                                self.pos += 1;
                                if self.current().token == Token::DoubleColon {
                                    ns.push(n);
                                    self.eat(Token::DoubleColon);
                                } else {
                                    method_name = n;
                                    break;
                                }
                            }
                            _ => panic!("Namespace metodu bekleniyor!"),
                        }
                    }

                    if self.current().token == Token::LParen {
                        self.eat(Token::LParen);
                        let mut args = Vec::new();
                        if self.current().token != Token::RParen {
                            loop {
                                args.push(self.parse_expression());
                                if self.current().token == Token::Comma {
                                    self.eat(Token::Comma);
                                } else {
                                    break;
                                }
                            }
                        }
                        self.eat(Token::RParen);

                        expr = Expr::NamespaceCall {
                            namespace: ns.join("::"),
                            method: method_name,
                            args,
                            span,
                        };

                    } 
                    else {
                        // Just an identifier part of namespace? Not supported yet as expr
                        panic!("Namespace call expected '('");
                    }

                    expr
                } else if self.current().token == Token::LParen {
                    self.eat(Token::LParen);
                    let mut args = Vec::new();
                    if self.current().token != Token::RParen {
                        loop {
                            args.push(self.parse_expression());
                            if self.current().token == Token::Comma {
                                self.eat(Token::Comma);
                            } else {
                                break;
                            }
                        }
                    }
                    self.eat(Token::RParen);
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        span,
                    };
                    expr
                } else {
                    expr
                }
            }
            Token::Less => self.parse_ui_element(),
            _ => panic!("Ifade (Expression) bekleniyor, bulundu: {:?}", self.current().token),
        }
    }

    fn parse_ui_element(&mut self) -> Expr {
        let span = self.current().span();
        self.eat(Token::Less);
        let tag = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Tag ismi bekleniyor!"),
        };
        self.pos += 1;

        let mut props = HashMap::new();
        let prev_in_tag = self.in_tag_start;
        self.in_tag_start = true;
        
        loop {
            match &self.current().token {
                Token::Identifier(p_name) => {
                    let name = p_name.clone();
                    self.pos += 1;
                    
                    if self.current().token == Token::Colon || self.current().token == Token::Assign {
                        self.pos += 1;
                        let value = self.parse_expression();
                        props.insert(name, value);
                    } else {
                        props.insert(name, Expr::BoolLit(true, self.current().span()));
                    }
                }
                Token::Css => {
                    // Allow reserved keyword `css` as a prop name inside tags: css: <expr>
                    let name = "css".to_string();
                    self.pos += 1;
                    if self.current().token == Token::Colon || self.current().token == Token::Assign {
                        self.pos += 1;
                        let value = self.parse_expression();
                        props.insert(name, value);
                    } else {
                        props.insert(name, Expr::BoolLit(true, self.current().span()));
                    }
                }
                _ => break,
            }
        }
        
        self.in_tag_start = prev_in_tag;

        if self.current().token == Token::SlashGreater {
            self.eat(Token::SlashGreater);
            return Expr::UiElement { tag, props, children: vec![], span };
        }

        self.eat(Token::Greater);

        let mut children: Vec<Expr> = Vec::new();
        let mut pending_words: Vec<String> = Vec::new();
        while self.current().token != Token::EOF {
            if self.current().token == Token::Less {
                if let Some(TokenData { token: Token::Slash, .. }) = self.tokens.get(self.pos + 1) {
                    if !pending_words.is_empty() {
                        let text = pending_words.join(" ");
                        children.push(Expr::StringLit(text, self.current().span()));
                        pending_words.clear();
                    }
                    self.pos += 2;
                    let close_tag = match &self.current().token {
                        Token::Identifier(n) => n.clone(),
                        _ => panic!("Kapatma tagi ismi bekleniyor!"),
                    };
                    self.pos += 1;
                    if close_tag != tag {
                        panic!(
                            "Line {}:{}: Tag mismatch! Expected: </{}>, Found: </{}>",
                            self.current().line,
                            self.current().col,
                            tag,
                            close_tag
                        );
                    }
                    self.eat(Token::Greater);
                    break;
                }
            }
            // Merge consecutive bare identifiers into a single text node: <h2>Client UI</h2>
            match &self.current().token {
                Token::Identifier(w) => {
                    pending_words.push(w.clone());
                    self.pos += 1;
                }
                _ => {
                    if !pending_words.is_empty() {
                        let text = pending_words.join(" ");
                        children.push(Expr::StringLit(text, self.current().span()));
                        pending_words.clear();
                    }
                    children.push(self.parse_ui_child());
                }
            }
        }
        if !pending_words.is_empty() {
            let text = pending_words.join(" ");
            children.push(Expr::StringLit(text, self.current().span()));
        }
        
        Expr::UiElement { tag, props, children, span }
    }

    fn parse_ui_child(&mut self) -> Expr {
        if self.current().token == Token::Less {
            return self.parse_ui_element();
        }
        self.parse_expression()
    }
}

pub fn parse_expr_from_str(source: &str) -> Result<Expr, String> {
    let mut lexer = crate::lexer::Lexer::new(source);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token();
        let done = tok.token == crate::lexer::Token::EOF;
        tokens.push(tok);
        if done {
            break;
        }
    }
    let mut p = Parser::new(tokens);
    let expr = p.parse_expression();
    if p.current().token != crate::lexer::Token::EOF {
        let cur = p.current();
        return Err(format!(
            "Trailing tokens after expression at {}:{}: {:?}",
            cur.line, cur.col, cur.token
        ));
    }
    Ok(expr)
}
