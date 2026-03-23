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

    fn parse_struct_literal(&mut self) -> Expr {
        let span = self.current().span();
        self.eat(Token::LBrace);
        let mut fields = Vec::new();
        if self.current().token != Token::RBrace {
            loop {
                let name = match &self.current().token {
                    Token::Identifier(s) => s.clone(),
                    Token::StringLit(s) => s.clone(),
                    _ => panic!("Struct field name expected"),
                };
                self.pos += 1;
                self.eat(Token::Colon);
                let value = self.parse_expression();
                fields.push((name, value));
                if self.current().token == Token::Comma {
                    self.eat(Token::Comma);
                } else {
                    break;
                }
            }
        }
        self.eat(Token::RBrace);
        Expr::StructLiteral { fields, span }
    }

    fn peek(&self) -> &TokenData {
        static EOF_TOKEN: TokenData = TokenData { token: Token::EOF, line: 0, col: 0 };
        self.tokens.get(self.pos).unwrap_or(&EOF_TOKEN)
    }

    fn current(&self) -> &TokenData {
        static EOF_TOKEN: TokenData = TokenData { token: Token::EOF, line: 0, col: 0 };
        self.tokens.get(self.pos).unwrap_or(&EOF_TOKEN)
    }

    fn eat(&mut self, expected: Token) {
        if self.current().token == expected {
            self.pos += 1;
        } else if expected == Token::Semi && (self.current().token == Token::EOF || self.current().token == Token::RBrace) {
            // REPL-friendly & block-friendly: allow omitting semicolon before } or at EOF.
        } else {
            let cur = self.current();
            panic!(
                "Synchronization error! (Line {}:{}) Expected: {:?}, Found: {:?}",
                cur.line, cur.col, expected, cur.token
            );
        }
    }

    pub fn parse(&mut self) -> Program {
        let mut is_strict = false;
        if self.current().token == Token::AtStrict {
            self.eat(Token::AtStrict);
            is_strict = true;
        }

        let mut statements = Vec::new();
        while self.current().token != Token::EOF {
            statements.push(self.parse_statement());
        }
        Program { is_strict, statements }
    }

    fn parse_statement(&mut self) -> Stmt {
        let token = self.current().token.clone();
        match token {
            Token::Var => self.parse_var_decl(),
            Token::Component => self.parse_component_decl(),
            Token::State => self.parse_state_decl(),
            Token::Import => self.parse_import_stmt(),
            Token::Print => self.parse_print_stmt(),
            Token::AtServer => self.parse_server_block(),
            Token::AtClient => self.parse_client_block(),
            Token::Model => self.parse_model_decl(),
            Token::If => self.parse_if_stmt(),
            Token::While => self.parse_while_stmt(),
            Token::Fn => self.parse_fn_decl(false),
            Token::Async => {
                let span = self.current().span();
                self.eat(Token::Async);
                if self.current().token == Token::Fn {
                    self.parse_fn_decl(true)
                } else {
                    panic!("Line {}:{}: Expected 'fn' after 'async'", span.line, span.col);
                }
            }
            Token::Return => self.parse_return_stmt(),
            Token::Test => self.parse_test_stmt(),
            Token::CssBlock(raw) => {
                let span = self.current().span();
                let raw_val = raw.clone();
                self.pos += 1;
                let expr = match crate::css_parser::parse_css(&raw_val, span.clone()) {
                    Ok(stylesheet) => crate::ast::Expr::CssAst { stylesheet, span: span.clone() },
                    Err(_) => crate::ast::Expr::EmbeddedBlock { kind: crate::ast::EmbeddedKind::Css, raw: raw_val, span: span.clone() },
                };
                Stmt::ExprStmt(expr, span)
            }
            Token::JsBlock(raw) => {
                let span = self.current().span();
                let raw_val = raw.clone();
                self.pos += 1;
                let expr = match crate::js_parser::parse_js(&raw_val, span.clone()) {
                    Ok(program) => crate::ast::Expr::JsAst { program, span: span.clone() },
                    Err(_) => crate::ast::Expr::EmbeddedBlock { kind: crate::ast::EmbeddedKind::Js, raw: raw_val, span: span.clone() },
                };
                Stmt::ExprStmt(expr, span)
            }
            Token::PhpBlock(raw) => {
                let span = self.current().span();
                let raw_val = raw.clone();
                self.pos += 1;
                let expr = match crate::php_parser::parse_php(&raw_val, span.clone()) {
                    Ok(program) => crate::ast::Expr::PhpAst { program, span: span.clone() },
                    Err(_) => {
                        crate::ast::Expr::EmbeddedBlock { kind: crate::ast::EmbeddedKind::Php, raw: raw_val, span: span.clone() }
                    },
                };
                Stmt::ExprStmt(expr, span)
            }
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_expr_stmt(&mut self) -> Stmt {
        let start_span = self.current().span();
        let expr = self.parse_expression();

        if self.current().token == Token::Assign {
            if let Expr::Identifier(ref name, _) = expr {
                self.eat(Token::Assign);
                let value = self.parse_expression();
                self.eat(Token::Semi);
                return Stmt::AssignStmt { name: name.clone(), value, span: start_span };
            } else {
                panic!("Invalid assignment target at line {}:{}", start_span.line, start_span.col);
            }
        }
        if self.current().token == Token::Semi {
            self.eat(Token::Semi);
        } else if !matches!(expr, Expr::UiElement { .. } | Expr::EmbeddedBlock { .. } | Expr::HtmlAst { .. } | Expr::CssAst { .. } | Expr::JsAst { .. } | Expr::PhpAst { .. }) {
            self.eat(Token::Semi);
        }
        Stmt::ExprStmt(expr, start_span)
    }

    fn parse_import_stmt(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Import);
        
        let mut imports = Vec::new();
        if self.current().token == Token::LBrace {
            self.eat(Token::LBrace);
            loop {
                if let Token::Identifier(name) = &self.current().token {
                    imports.push(name.clone());
                    self.pos += 1;
                }
                if self.current().token == Token::Comma {
                    self.eat(Token::Comma);
                } else {
                    break;
                }
            }
            self.eat(Token::RBrace);
        } else if let Token::Identifier(name) = &self.current().token {
            // Support: import Header from "..."
            imports.push(name.clone());
            self.pos += 1;
        }

        self.eat(Token::From);
        let source = match &self.current().token {
            Token::StringLit(s) => s.clone(),
            _ => panic!("Import source string expected"),
        };
        self.pos += 1;
        self.eat(Token::Semi);
        
        Stmt::ImportStmt { imports, source, span }
    }

    fn parse_var_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Var);
        
        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Expected variable name!"),
        };
        self.pos += 1;

        let mut type_ann = None;
        if self.current().token == Token::Colon {
            self.eat(Token::Colon);
            type_ann = Some(self.parse_type());
        }

        self.eat(Token::Assign);
        let value = self.parse_expression();
        self.eat(Token::Semi);

        Stmt::VarDecl { name, type_ann, value, span }
    }

    fn parse_state_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::State);
        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("State name expected!"),
        };
        self.pos += 1;

        let mut type_ann = None;
        if self.current().token == Token::Colon {
            self.eat(Token::Colon);
            type_ann = Some(self.parse_type());
        }

        self.eat(Token::Assign);
        let value = self.parse_expression();
        self.eat(Token::Semi);
        Stmt::StateDecl { name, type_ann, value, span }
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
        
        let next = self.current().token.clone();
        match next {
            Token::PhpBlock(raw) => {
                self.pos += 1;
                let expr = match crate::php_parser::parse_php(&raw, span.clone()) {
                    Ok(program) => crate::ast::Expr::PhpAst { program, span: span.clone() },
                    Err(_) => {
                        crate::ast::Expr::EmbeddedBlock { kind: crate::ast::EmbeddedKind::Php, raw, span: span.clone() }
                    },
                };
                Stmt::ServerBlock(vec![Stmt::ExprStmt(expr, span.clone())], span)
            }
            Token::JsBlock(raw) => {
                self.pos += 1;
                let expr = match crate::js_parser::parse_js(&raw, span.clone()) {
                    Ok(program) => crate::ast::Expr::JsAst { program, span: span.clone() },
                    Err(_) => crate::ast::Expr::EmbeddedBlock { kind: crate::ast::EmbeddedKind::Js, raw, span: span.clone() },
                };
                Stmt::ServerBlock(vec![Stmt::ExprStmt(expr, span.clone())], span)
            }
            Token::LBrace => {
                self.eat(Token::LBrace);
                let mut stmts = Vec::new();
                while self.current().token != Token::RBrace && self.current().token != Token::EOF {
                    stmts.push(self.parse_statement());
                }
                self.eat(Token::RBrace);
                Stmt::ServerBlock(stmts, span)
            }
            _ => {
                let s = self.parse_statement();
                Stmt::ServerBlock(vec![s], span)
            }
        }
    }

    fn parse_model_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Model);

        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Model name expected!"),
        };
        self.pos += 1;

        self.eat(Token::LBrace);
        
        let mut fields = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            let f_span = self.current().span();
            let field_name = match &self.current().token {
                Token::Identifier(n) => n.clone(),
                _ => panic!("Field name (identifier) expected!"),
            };
            self.pos += 1;

            self.eat(Token::Colon);

            let field_type = match &self.current().token {
                Token::StrType => Type::Str,
                Token::IntType => Type::Int,
                Token::FloatType => Type::Float,
                Token::BoolType => Type::Bool,
                Token::Identifier(n) => Type::Custom(n.clone()),
                _ => panic!("Invalid field type!"),
            };
            self.pos += 1;

            fields.push(FieldDecl { name: field_name, field_type, span: f_span });
            
            if self.current().token == Token::Comma {
                self.eat(Token::Comma);
            }
        }

        self.eat(Token::RBrace);
        Stmt::ModelDecl { name, fields, span }
    }

    fn parse_client_block(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::AtClient);
        
        let next = self.current().token.clone();
        match next {
            Token::JsBlock(raw) => {
                self.pos += 1;
                let expr = match crate::js_parser::parse_js(&raw, span.clone()) {
                    Ok(program) => crate::ast::Expr::JsAst { program, span: span.clone() },
                    Err(_) => crate::ast::Expr::EmbeddedBlock { kind: crate::ast::EmbeddedKind::Js, raw, span: span.clone() },
                };
                Stmt::ClientBlock(vec![Stmt::ExprStmt(expr, span.clone())], span)
            }
            Token::LBrace => {
                self.eat(Token::LBrace);
                let mut stmts = Vec::new();
                while self.current().token != Token::RBrace && self.current().token != Token::EOF {
                    stmts.push(self.parse_statement());
                }
                self.eat(Token::RBrace);
                Stmt::ClientBlock(stmts, span)
            }
            _ => {
                let s = self.parse_statement();
                Stmt::ClientBlock(vec![s], span)
            }
        }
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

    fn parse_fn_decl(&mut self, is_async: bool) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Fn);
        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Function name expected!"),
        };
        self.pos += 1;

        self.eat(Token::LParen);
        let mut params = Vec::new();
        if self.current().token != Token::RParen {
            loop {
                let param_name = match &self.current().token {
                    Token::Identifier(n) => n.clone(),
                    _ => panic!("Parameter name expected!"),
                };
                self.pos += 1;

                let mut p_type = None;
                if self.current().token == Token::Colon {
                    self.eat(Token::Colon);
                    p_type = Some(self.parse_type());
                }

                params.push((param_name, p_type));

                if self.current().token == Token::Comma {
                    self.eat(Token::Comma);
                } else {
                    break;
                }
            }
        }
        self.eat(Token::RParen);

        let mut return_type = None;
        if self.current().token == Token::Colon {
            self.eat(Token::Colon);
            return_type = Some(self.parse_type());
        }

        self.eat(Token::LBrace);
        let mut body = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            body.push(self.parse_statement());
        }
        self.eat(Token::RBrace);

        Stmt::FnDecl {
            name,
            params,
            return_type,
            body,
            is_async,
            span,
        }
    }

    fn parse_component_decl(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Component);

        let name = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Expected component name!"),
        };
        self.pos += 1;

        let mut props = Vec::new();

        self.eat(Token::LParen);
        if self.current().token != Token::RParen {
            loop {
                let prop_name = match &self.current().token {
                    Token::Identifier(n) => n.clone(),
                    _ => panic!("Property name expected!"),
                };
                self.pos += 1;

                let mut p_type = Type::Str; // Default if omitted
                if self.current().token == Token::Colon {
                    self.eat(Token::Colon);
                    p_type = self.parse_type();
                }

                props.push((prop_name, p_type));

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

        Stmt::ComponentDecl {
            name,
            props,
            body,
            span,
        }
    }

    fn parse_type(&mut self) -> Type {
        let t = match &self.current().token {
            Token::StrType => Type::Str,
            Token::IntType => Type::Int,
            Token::FloatType => Type::Float,
            Token::BoolType => Type::Bool,
            Token::Identifier(n) => Type::Custom(n.clone()),
            _ => panic!("Expected type! Found: {:?}", self.current().token),
        };
        self.pos += 1;
        t
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
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Expr {
        let mut expr = self.parse_comparison();

        if self.current().token == Token::Question {
            let span = self.current().span();
            self.eat(Token::Question);
            let then_expr = self.parse_expression();
            self.eat(Token::Colon);
            let else_expr = self.parse_expression();
            expr = Expr::Ternary {
                condition: Box::new(expr),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            };
        }
        expr
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
        let token = self.current().token.clone();
        let mut expr = match token {
            Token::Await => {
                self.eat(Token::Await);
                let inner = self.parse_expression();
                Expr::Await(Box::new(inner), span)
            }
            Token::IntLit(val) => {
                let span = self.current().span();
                self.pos += 1;
                Expr::IntLit(val, span)
            }
            Token::FloatLit(val) => {
                let span = self.current().span();
                self.pos += 1;
                Expr::FloatLit(val, span)
            }
            Token::StringLit(val) => {
                let span = self.current().span();
                self.pos += 1;
                Expr::StringLit(val, span)
            }
            Token::LParen => {
                self.eat(Token::LParen);
                let expr = self.parse_expression();
                self.eat(Token::RParen);
                expr
            }
            Token::LBrace => self.parse_struct_literal(),
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
                let mut e = if identifier_str == "true" { 
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
                            _ => panic!("Namespace method expected!"),
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

                        e = Expr::NamespaceCall {
                            namespace: ns.join("::"),
                            method: method_name,
                            args,
                            span,
                        };
                    } 
                    else {
                        panic!("Namespace call expected '('");
                    }
                }
                e
            }
            Token::AtIdentifier(name) => {
                let sp = self.current().span();
                self.pos += 1;
                Expr::StringLit(format!("@{}", name), sp)
            }
            Token::Less => self.parse_ui_element(),
            _ => panic!("Expression expected, found: {:?}", token),
        };

        // Postfix operators loop (Member access and Function calls)
        loop {
            match self.current().token {
                Token::LParen => {
                    let call_span = self.current().span();
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
                        span: call_span,
                    };
                }
                Token::Dot => {
                    let dot_span = self.current().span();
                    self.eat(Token::Dot);
                    let property = match &self.current().token {
                        Token::Identifier(n) => n.clone(),
                        _ => panic!("Line {}:{}: Property name expected after '.'!", self.current().line, self.current().col),
                    };
                    self.pos += 1;
                    expr = Expr::Member {
                        object: Box::new(expr),
                        property,
                        span: dot_span,
                    };
                }
                _ => break,
            }
        }
        expr
    }

    fn parse_ui_element(&mut self) -> Expr {
        let span = self.current().span();
        self.eat(Token::Less);
        let tag = match &self.current().token {
            Token::Identifier(n) => n.clone(),
            _ => panic!("Tag name expected!"),
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
                Token::CssBlock(raw) => {
                    let name = "css".to_string();
                    let raw_val = raw.clone();
                    let span = self.current().span();
                    self.pos += 1;
                    props.insert(name, Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw: raw_val, span });
                }
                Token::Css => {
                    // Allow reserved keyword `css` as a prop name inside tags: css={...}
                    let name = "css".to_string();
                    self.pos += 1;
                    if self.current().token == Token::Colon || self.current().token == Token::Assign {
                        self.pos += 1;
                        let value = self.parse_expression();
                        props.insert(name, value);
                    } else if let Token::CssBlock(_) = &self.current().token {
                        // Handle css{...} form (no =)
                        let span = self.current().span();
                        let raw = match &self.current().token { Token::CssBlock(r) => r.clone(), _ => unreachable!() };
                        self.pos += 1;
                        props.insert(name, Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, span });
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
                    self.pos += 2; // eat </
                    let close_tag = match &self.current().token {
                        Token::Identifier(n) => n.clone(),
                        _ => panic!("Closing tag name expected!"),
                    };
                    self.pos += 1;
                    if close_tag != tag {
                        panic!("Line {}:{}: Tag mismatch! Expected: </{}>, Found: </{}>", self.current().line, self.current().col, tag, close_tag);
                    }
                    self.eat(Token::Greater);
                    break;
                }
                // Nested element
                if !pending_words.is_empty() {
                    let text = pending_words.join(" ");
                    children.push(Expr::StringLit(text, self.current().span()));
                    pending_words.clear();
                }
                children.push(self.parse_ui_element());
            } else if self.current().token == Token::LDoubleBrace {
                if !pending_words.is_empty() {
                    let text = pending_words.join(" ");
                    children.push(Expr::StringLit(text, self.current().span()));
                    pending_words.clear();
                }
                self.eat(Token::LDoubleBrace);
                children.push(Expr::Interpolation(Box::new(self.parse_expression()), self.current().span()));
                self.eat(Token::RDoubleBrace);
            } else if self.current().token == Token::AtClient {
                let start_span = self.current().span();
                let stmt = self.parse_client_block();
                if let Stmt::ClientBlock(stmts, span) = stmt {
                    children.push(Expr::ClientScript(stmts, span));
                }
            } else if self.current().token == Token::AtServer {
                let start_span = self.current().span();
                let stmt = self.parse_server_block();
                if let Stmt::ServerBlock(stmts, span) = stmt {
                    children.push(Expr::ServerScript(stmts, span));
                }
            } else if let Token::JsBlock(raw) = self.current().token.clone() {
                if !pending_words.is_empty() {
                    let text = pending_words.join(" ");
                    children.push(Expr::StringLit(text, self.current().span()));
                    pending_words.clear();
                }
                let span = self.current().span();
                self.pos += 1;
                // Treat JsBlock inside UI as a client script by default if it was shorthand @client
                children.push(Expr::ClientScript(vec![Stmt::ExprStmt(Expr::EmbeddedBlock { kind: EmbeddedKind::Js, raw, span: span.clone() }, span.clone())], span));
            } else if let Token::CssBlock(raw) = self.current().token.clone() {
                if !pending_words.is_empty() {
                    let text = pending_words.join(" ");
                    children.push(Expr::StringLit(text, self.current().span()));
                    pending_words.clear();
                }
                let span = self.current().span();
                self.pos += 1;
                children.push(Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, span });
            } else if let Token::HtmlBlock(raw) = self.current().token.clone() {
                if !pending_words.is_empty() {
                    let text = pending_words.join(" ");
                    children.push(Expr::StringLit(text, self.current().span()));
                    pending_words.clear();
                }
                let span = self.current().span();
                self.pos += 1;
                children.push(Expr::EmbeddedBlock { kind: EmbeddedKind::Html, raw, span });
            } else {
                // Treat as text node word
                let word = match &self.current().token {
                    Token::Identifier(w) => w.clone(),
                    Token::IntLit(i) => i.to_string(),
                    Token::FloatLit(f) => f.to_string(),
                    Token::StringLit(s) => s.clone(),
                    Token::Colon => ":".to_string(),
                    Token::Greater => ">".to_string(),
                    Token::Less => "<".to_string(),
                    Token::Plus => "+".to_string(),
                    Token::Minus => "-".to_string(),
                    Token::Star => "*".to_string(),
                    Token::Slash => "/".to_string(),
                    Token::LParen => "(".to_string(),
                    Token::RParen => ")".to_string(),
                    Token::LBrace => "{".to_string(),
                    Token::RBrace => "}".to_string(),
                    Token::LBracket => "[".to_string(),
                    Token::RBracket => "]".to_string(),
                    Token::Comma => ",".to_string(),
                    Token::Dot => ".".to_string(),
                    Token::Semi => ";".to_string(),
                    Token::Eq => "=".to_string(),
                    Token::Bang => "!".to_string(),
                    Token::Question => "?".to_string(),
                    Token::Hash => "#".to_string(),
                    Token::Percent => "%".to_string(),
                    Token::AtIdentifier(s) => format!("@{}", s),
                    _ => format!("{:?}", self.current().token)
                };
                pending_words.push(word);
                self.pos += 1;
            }
        }
        if !pending_words.is_empty() {
            let text = pending_words.join(" ");
            children.push(Expr::StringLit(text, self.current().span()));
        }
        
        Expr::UiElement { tag, props, children, span }
    }

    fn parse_test_stmt(&mut self) -> Stmt {
        let span = self.current().span();
        self.eat(Token::Test);
        let name = match &self.current().token {
            Token::StringLit(s) => s.clone(),
            _ => panic!("Test description (string) expected at line {}:{}", span.line, span.col),
        };
        self.pos += 1;
        self.eat(Token::LBrace);
        let mut body = Vec::new();
        while self.current().token != Token::RBrace && self.current().token != Token::EOF {
            body.push(self.parse_statement());
        }
        self.eat(Token::RBrace);
        Stmt::Test { name, body, span }
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
