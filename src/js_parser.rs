use crate::ast::{JsExpr, JsParam, JsPattern, JsProgram, JsStmt, JsTemplatePart, JsVarKind, Span};
use crate::js_lexer::{JsLexer, JsTok};

pub fn parse_js(raw: &str, span: Span) -> Result<JsProgram, String> {
    let mut lx = JsLexer::new(raw, span.clone());
    let mut toks = Vec::new();
    loop {
        let t = lx.next();
        let done = matches!(t, JsTok::EOF(_));
        toks.push(t);
        if done { break; }
    }
    let mut p = Parser { toks, pos: 0 };
    let mut body = Vec::new();
    while !p.is_eof() {
        match p.parse_stmt() {
            Ok(s) => body.push(s),
            Err(e) => {
                let sp = p.peek_span();
                return Err(format!("{}:{}: {}", sp.line, sp.col, e));
            }
        }
    }
    Ok(JsProgram { body, span })
}

struct Parser {
    toks: Vec<JsTok>,
    pos: usize,
}

impl Parser {
    fn is_eof(&self) -> bool {
        matches!(self.peek(), JsTok::EOF(_))
    }

    fn peek(&self) -> JsTok {
        self.toks.get(self.pos).cloned().unwrap_or(JsTok::EOF(Span { line: 0, col: 0 }))
    }

    fn peek_span(&self) -> Span {
        match self.peek() {
            JsTok::Ident(_, sp)
            | JsTok::String(_, sp)
            | JsTok::Number(_, sp)
            | JsTok::TemplateChunk(_, sp) => sp,
            JsTok::True(sp)
            | JsTok::False(sp)
            | JsTok::Null(sp)
            | JsTok::Const(sp)
            | JsTok::Let(sp)
            | JsTok::Var(sp)
            | JsTok::If(sp)
            | JsTok::Else(sp)
            | JsTok::For(sp)
            | JsTok::While(sp)
            | JsTok::Return(sp)
            | JsTok::Function(sp)
            | JsTok::Try(sp)
            | JsTok::Catch(sp)
            | JsTok::Throw(sp)
            | JsTok::Class(sp)
            | JsTok::Constructor(sp)
            | JsTok::Extends(sp)
            | JsTok::Static(sp)
            | JsTok::Switch(sp)
            | JsTok::Case(sp)
            | JsTok::Default(sp)
            | JsTok::Break(sp)
            | JsTok::Continue(sp)
            | JsTok::New(sp)
            | JsTok::This(sp)
            | JsTok::Super(sp)
            | JsTok::LParen(sp)
            | JsTok::RParen(sp)
            | JsTok::LBrace(sp)
            | JsTok::RBrace(sp)
            | JsTok::LBracket(sp)
            | JsTok::RBracket(sp)
            | JsTok::Dot(sp)
            | JsTok::Ellipsis(sp)
            | JsTok::Comma(sp)
            | JsTok::Semi(sp)
            | JsTok::Colon(sp)
            | JsTok::Question(sp)
            | JsTok::Plus(sp)
            | JsTok::PlusPlus(sp)
            | JsTok::Minus(sp)
            | JsTok::MinusMinus(sp)
            | JsTok::Star(sp)
            | JsTok::Slash(sp)
            | JsTok::Bang(sp)
            | JsTok::Eq(sp)
            | JsTok::EqEq(sp)
            | JsTok::BangEq(sp)
            | JsTok::Less(sp)
            | JsTok::LessEq(sp)
            | JsTok::Greater(sp)
            | JsTok::GreaterEq(sp)
            | JsTok::AndAnd(sp)
            | JsTok::OrOr(sp)
            | JsTok::EqEqEq(sp)
            | JsTok::BangEqEq(sp)
            | JsTok::Arrow(sp)
            | JsTok::Async(sp)
            | JsTok::Await(sp)
            | JsTok::Backtick(sp)
            | JsTok::DollarLBrace(sp)
            | JsTok::EOF(sp) => sp,
        }
    }

    fn bump(&mut self) -> JsTok {
        let t = self.peek();
        self.pos += 1;
        t
    }

    fn expect_semi_opt(&mut self) {
        if matches!(self.peek(), JsTok::Semi(_)) {
            self.bump();
        }
    }

    fn parse_stmt(&mut self) -> Result<JsStmt, String> {
        match self.peek() {
            JsTok::LBrace(sp) => {
                self.bump();
                let mut body = Vec::new();
                while !matches!(self.peek(), JsTok::RBrace(_)) && !self.is_eof() {
                    body.push(self.parse_stmt()?);
                }
                if !matches!(self.peek(), JsTok::RBrace(_)) {
                    return Err("Expected '}'".to_string());
                }
                self.bump();
                Ok(JsStmt::Block { body, span: sp })
            }
            JsTok::Async(sp) => {
                self.bump();
                if matches!(self.peek(), JsTok::Function(_)) {
                    self.bump();
                    let name = match self.bump() {
                        JsTok::Ident(s, _) => s,
                        _ => return Err("Expected function name".to_string()),
                    };
                    let params = self.parse_param_list()?;
                    let body = self.parse_stmt()?;
                    Ok(JsStmt::FunctionDecl { name, params, body: Box::new(body), is_async: true, span: sp })
                } else {
                    // async () => ...
                    let expr = self.parse_arrow_fn(true)?;
                    self.expect_semi_opt();
                    Ok(JsStmt::Expr(expr, sp))
                }
            }
            JsTok::Function(sp) => {
                self.bump();
                let name = match self.bump() {
                    JsTok::Ident(s, _) => s,
                    _ => return Err("Expected function name".to_string()),
                };
                let params = self.parse_param_list()?;
                let body = self.parse_stmt()?;
                Ok(JsStmt::FunctionDecl { name, params, body: Box::new(body), is_async: false, span: sp })
            }
            JsTok::Try(sp) => {
                self.bump();
                let try_block = self.parse_stmt()?;
                match self.bump() {
                    JsTok::Catch(_) => {}
                    _ => return Err("Expected 'catch'".to_string()),
                }
                self.expect_lparen()?;
                let catch_name = match self.bump() {
                    JsTok::Ident(s, _) => s,
                    _ => return Err("Expected identifier in catch(...)".to_string()),
                };
                self.expect_rparen()?;
                let catch_block = self.parse_stmt()?;
                Ok(JsStmt::TryCatch { try_block: Box::new(try_block), catch_name, catch_block: Box::new(catch_block), span: sp })
            }
            JsTok::Throw(sp) => {
                self.bump();
                let e = self.parse_expr(0)?;
                self.expect_semi_opt();
                Ok(JsStmt::Throw(e, sp))
            }
            JsTok::If(sp) => {
                self.bump();
                self.expect_lparen()?;
                let cond = self.parse_expr(0)?;
                self.expect_rparen()?;
                let then_branch = self.parse_stmt()?;
                let else_branch = if matches!(self.peek(), JsTok::Else(_)) {
                    self.bump();
                    Some(Box::new(self.parse_stmt()?))
                } else {
                    None
                };
                Ok(JsStmt::If { condition: cond, then_branch: Box::new(then_branch), else_branch, span: sp })
            }
            JsTok::While(sp) => {
                self.bump();
                self.expect_lparen()?;
                let cond = self.parse_expr(0)?;
                self.expect_rparen()?;
                let body = self.parse_stmt()?;
                Ok(JsStmt::While { condition: cond, body: Box::new(body), span: sp })
            }
            JsTok::For(sp) => {
                self.bump();
                self.expect_lparen()?;
                let init = if matches!(self.peek(), JsTok::Semi(_)) {
                    self.bump();
                    None
                } else if matches!(self.peek(), JsTok::Const(_) | JsTok::Let(_) | JsTok::Var(_)) {
                    Some(Box::new(self.parse_stmt()?))
                } else {
                    let e = self.parse_expr(0)?;
                    self.expect_semi()?;
                    Some(Box::new(JsStmt::Expr(e, sp.clone())))
                };
                let condition = if matches!(self.peek(), JsTok::Semi(_)) {
                    self.bump();
                    None
                } else {
                    let e = self.parse_expr(0)?;
                    self.expect_semi()?;
                    Some(e)
                };
                let update = if matches!(self.peek(), JsTok::RParen(_)) {
                    None
                } else {
                    Some(self.parse_expr(0)?)
                };
                self.expect_rparen()?;
                let body = self.parse_stmt()?;
                Ok(JsStmt::For { init, condition, update, body: Box::new(body), span: sp })
            }
            JsTok::Return(sp) => {
                self.bump();
                if matches!(self.peek(), JsTok::Semi(_)) {
                    self.bump();
                    return Ok(JsStmt::Return(None, sp));
                }
                let v = self.parse_expr(0)?;
                self.expect_semi_opt();
                Ok(JsStmt::Return(Some(v), sp))
            }
            JsTok::Const(sp) | JsTok::Let(sp) | JsTok::Var(sp) => {
                let kind = match self.bump() {
                    JsTok::Const(_) => JsVarKind::Const,
                    JsTok::Let(_) => JsVarKind::Let,
                    JsTok::Var(_) => JsVarKind::Var,
                    _ => unreachable!(),
                };
                let pattern = self.parse_pattern()?;
                let value = if matches!(self.peek(), JsTok::Eq(_)) {
                    self.bump();
                    Some(self.parse_expr(0)?)
                } else {
                    None
                };
                self.expect_semi_opt();
                Ok(JsStmt::VarDecl { kind, pattern, value, span: sp })
            }
            JsTok::Class(sp) => {
                self.bump();
                let name = match self.bump() {
                    JsTok::Ident(s, _) => s,
                    _ => return Err("Expected class name".to_string()),
                };
                let mut extends = None;
                if matches!(self.peek(), JsTok::Extends(_)) {
                    self.bump();
                    extends = Some(match self.bump() {
                        JsTok::Ident(s, _) => s,
                        _ => return Err("Expected identifier after extends".to_string()),
                    });
                }
                match self.bump() {
                    JsTok::LBrace(_) => {}
                    _ => return Err("Expected '{' for class body".to_string()),
                }
                let mut body = Vec::new();
                while !matches!(self.peek(), JsTok::RBrace(_)) && !self.is_eof() {
                    // Method parsing
                    let _is_static = if matches!(self.peek(), JsTok::Static(_)) {
                        self.bump();
                        true
                    } else {
                        false
                    };
                    let method_name = match self.bump() {
                        JsTok::Ident(s, _) => s,
                        JsTok::Constructor(_) => "constructor".to_string(),
                        _ => return Err("Expected method name".to_string()),
                    };
                    let params = self.parse_param_list()?;
                    let method_body = self.parse_stmt()?;
                    body.push(JsStmt::FunctionDecl { 
                        name: method_name, 
                        params, 
                        body: Box::new(method_body), 
                        is_async: false, // Could be async later
                        span: sp.clone() 
                    });
                }
                self.bump(); // }
                Ok(JsStmt::ClassDecl { name, extends, body, span: sp })
            }
            JsTok::Switch(sp) => {
                self.bump();
                self.expect_lparen()?;
                let discriminant = self.parse_expr(0)?;
                self.expect_rparen()?;
                match self.bump() {
                    JsTok::LBrace(_) => {}
                    _ => return Err("Expected '{' for switch".to_string()),
                }
                let mut cases = Vec::new();
                let mut default = None;
                while !matches!(self.peek(), JsTok::RBrace(_)) && !self.is_eof() {
                    match self.bump() {
                        JsTok::Case(_) => {
                            let cond = self.parse_expr(0)?;
                            match self.bump() {
                                JsTok::Colon(_) => {}
                                _ => return Err("Expected ':' after case".to_string()),
                            }
                            let mut stmts = Vec::new();
                            while !matches!(self.peek(), JsTok::Case(_) | JsTok::Default(_) | JsTok::RBrace(_)) {
                                stmts.push(self.parse_stmt()?);
                            }
                            cases.push((cond, stmts));
                        }
                        JsTok::Default(_) => {
                            match self.bump() {
                                JsTok::Colon(_) => {}
                                _ => return Err("Expected ':' after default".to_string()),
                            }
                            let mut stmts = Vec::new();
                            while !matches!(self.peek(), JsTok::Case(_) | JsTok::Default(_) | JsTok::RBrace(_)) {
                                stmts.push(self.parse_stmt()?);
                            }
                            default = Some(stmts);
                        }
                        _ => return Err("Expected case or default".to_string()),
                    }
                }
                self.bump(); // }
                Ok(JsStmt::Switch { discriminant, cases, default, span: sp })
            }
            JsTok::Break(sp) => {
                self.bump();
                self.expect_semi_opt();
                Ok(JsStmt::Break(sp))
            }
            JsTok::Continue(sp) => {
                self.bump();
                self.expect_semi_opt();
                Ok(JsStmt::Continue(sp))
            }
            _ => {
                let sp = match self.peek() { JsTok::EOF(s) => s, t => match t { JsTok::Ident(_, s) => s, JsTok::String(_, s) => s, JsTok::Number(_, s) => s, _ => Span { line: 0, col: 0 } } };
                let expr = self.parse_expr(0)?;
                self.expect_semi_opt();
                Ok(JsStmt::Expr(expr, sp))
            }
        }
    }

    fn expect_lparen(&mut self) -> Result<(), String> {
        match self.bump() {
            JsTok::LParen(_) => Ok(()),
            _ => Err("Expected '('".to_string()),
        }
    }
    fn expect_rparen(&mut self) -> Result<(), String> {
        match self.bump() {
            JsTok::RParen(_) => Ok(()),
            _ => Err("Expected ')'".to_string()),
        }
    }
    fn expect_semi(&mut self) -> Result<(), String> {
        match self.bump() {
            JsTok::Semi(_) => Ok(()),
            _ => Err("Expected ';'".to_string()),
        }
    }

    fn precedence(tok: &JsTok) -> Option<(u8, &'static str)> {
        match tok {
            JsTok::OrOr(_) => Some((1, "||")),
            JsTok::AndAnd(_) => Some((2, "&&")),
            JsTok::EqEq(_) => Some((3, "==")),
            JsTok::EqEqEq(_) => Some((3, "===")),
            JsTok::BangEq(_) => Some((3, "!=")),
            JsTok::BangEqEq(_) => Some((3, "!==")),
            JsTok::Less(_) => Some((4, "<")),
            JsTok::LessEq(_) => Some((4, "<=")),
            JsTok::Greater(_) => Some((4, ">")),
            JsTok::GreaterEq(_) => Some((4, ">=")),
            JsTok::Plus(_) => Some((5, "+")),
            JsTok::Minus(_) => Some((5, "-")),
            JsTok::Star(_) => Some((6, "*")),
            JsTok::Slash(_) => Some((6, "/")),
            _ => None,
        }
    }

    fn parse_expr(&mut self, min_prec: u8) -> Result<JsExpr, String> {
        let mut left = self.parse_prefix()?;

        // assignment
        if matches!(self.peek(), JsTok::Eq(_)) {
            let sp = match self.bump() { JsTok::Eq(s) => s, _ => unreachable!() };
            let rhs = self.parse_expr(0)?;
            left = JsExpr::Assign { target: Box::new(left), value: Box::new(rhs), span: sp };
        }

        loop {
            // ternary
            if matches!(self.peek(), JsTok::Question(_)) && min_prec <= 0 {
                let sp = match self.bump() { JsTok::Question(s) => s, _ => unreachable!() };
                let then_e = self.parse_expr(0)?;
                match self.bump() {
                    JsTok::Colon(_) => {}
                    _ => return Err("Expected ':' in conditional expression".to_string()),
                }
                let else_e = self.parse_expr(0)?;
                left = JsExpr::Conditional { condition: Box::new(left), then_expr: Box::new(then_e), else_expr: Box::new(else_e), span: sp };
                continue;
            }

            let op = self.peek();
            let Some((prec, op_str)) = Self::precedence(&op) else { break; };
            if prec < min_prec { break; }
            let sp = match self.bump() {
                JsTok::Plus(s) | JsTok::Minus(s) | JsTok::Star(s) | JsTok::Slash(s) |
                JsTok::EqEq(s) | JsTok::EqEqEq(s) | JsTok::BangEq(s) | JsTok::BangEqEq(s) |
                JsTok::Less(s) | JsTok::LessEq(s) | JsTok::Greater(s) | JsTok::GreaterEq(s) |
                JsTok::AndAnd(s) | JsTok::OrOr(s) => s,
                _ => Span { line: 0, col: 0 },
            };
            let right = self.parse_expr(prec + 1)?;
            left = JsExpr::Binary { left: Box::new(left), op: op_str.to_string(), right: Box::new(right), span: sp };
        }
        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<JsExpr, String> {
        match self.peek() {
            JsTok::Bang(sp) => { self.bump(); Ok(JsExpr::Unary { op: "!".to_string(), expr: Box::new(self.parse_prefix()?), span: sp }) }
            JsTok::Minus(sp) => { self.bump(); Ok(JsExpr::Unary { op: "-".to_string(), expr: Box::new(self.parse_prefix()?), span: sp }) }
            JsTok::Await(sp) => {
                self.bump();
                let e = self.parse_prefix()?;
                Ok(JsExpr::Await { expr: Box::new(e), span: sp })
            }
            JsTok::PlusPlus(sp) => {
                self.bump();
                let expr = self.parse_prefix()?;
                Ok(JsExpr::Update { op: "++".to_string(), is_prefix: true, expr: Box::new(expr), span: sp })
            }
            JsTok::MinusMinus(sp) => {
                self.bump();
                let expr = self.parse_prefix()?;
                Ok(JsExpr::Update { op: "--".to_string(), is_prefix: true, expr: Box::new(expr), span: sp })
            }
            JsTok::New(sp) => {
                self.bump();
                let callee = self.parse_primary()?; // Simplified: new ClassName(...)
                let mut args = Vec::new();
                if matches!(self.peek(), JsTok::LParen(_)) {
                    self.bump();
                    if !matches!(self.peek(), JsTok::RParen(_)) {
                        loop {
                            args.push(self.parse_expr(0)?);
                            if matches!(self.peek(), JsTok::Comma(_)) {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect_rparen()?;
                }
                Ok(JsExpr::New { callee: Box::new(callee), args, span: sp })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<JsExpr, String> {
        // arrow function: (a,b)=>...
        if matches!(self.peek(), JsTok::LParen(_)) && self.looks_like_arrow() {
            return self.parse_arrow_fn(false);
        }

        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                JsTok::Dot(sp) => {
                    self.bump();
                    let prop = match self.bump() {
                        JsTok::Ident(s, _) => s,
                        _ => return Err("Expected property name".to_string()),
                    };
                    expr = JsExpr::Member { object: Box::new(expr), property: prop, span: sp };
                }
                JsTok::PlusPlus(sp) => {
                    self.bump();
                    expr = JsExpr::Update { op: "++".to_string(), is_prefix: false, expr: Box::new(expr), span: sp };
                }
                JsTok::MinusMinus(sp) => {
                    self.bump();
                    expr = JsExpr::Update { op: "--".to_string(), is_prefix: false, expr: Box::new(expr), span: sp };
                }
                JsTok::LParen(sp) => {
                    self.bump();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), JsTok::RParen(_)) {
                        loop {
                            if matches!(self.peek(), JsTok::Ellipsis(_)) {
                                let sp2 = match self.bump() { JsTok::Ellipsis(s) => s, _ => unreachable!() };
                                let inner = self.parse_expr(0)?;
                                args.push(JsExpr::Spread { expr: Box::new(inner), span: sp2 });
                            } else {
                                args.push(self.parse_expr(0)?);
                            }
                            if matches!(self.peek(), JsTok::Comma(_)) {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                    }
                    match self.bump() {
                        JsTok::RParen(_) => {}
                        _ => return Err("Expected ')'".to_string()),
                    }
                    expr = JsExpr::Call { callee: Box::new(expr), args, span: sp };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn looks_like_arrow(&self) -> bool {
        // ( ... ) =>
        let mut depth = 0i32;
        let mut i = self.pos;
        while let Some(t) = self.toks.get(i) {
            match t {
                JsTok::LParen(_) => { depth += 1; }
                JsTok::RParen(_) => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(self.toks.get(i + 1), Some(JsTok::Arrow(_)));
                    }
                }
                JsTok::EOF(_) => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn parse_arrow_fn(&mut self, is_async: bool) -> Result<JsExpr, String> {
        let sp = match self.peek() { JsTok::LParen(s) => s, _ => Span { line: 0, col: 0 } };
        let params = self.parse_param_list()?;
        match self.bump() { JsTok::Arrow(_) => {}, _ => return Err("Expected '=>'".to_string()) }

        let body = if matches!(self.peek(), JsTok::LBrace(_)) {
            self.parse_stmt()?
        } else {
            let e = self.parse_expr(0)?;
            JsStmt::Return(Some(e), sp.clone())
        };
        Ok(JsExpr::ArrowFn { params, body: Box::new(body), is_async, span: sp })
    }

    fn parse_param_list(&mut self) -> Result<Vec<JsParam>, String> {
        let lsp = match self.bump() {
            JsTok::LParen(s) => s,
            _ => return Err("Expected '('".to_string()),
        };
        let mut params = Vec::new();
        if !matches!(self.peek(), JsTok::RParen(_)) {
            loop {
                let mut is_rest = false;
                if matches!(self.peek(), JsTok::Ellipsis(_)) {
                    self.bump(); // ...
                    is_rest = true;
                }
                let (name, span) = match self.bump() {
                    JsTok::Ident(s, ssp) => (s, ssp),
                    _ => return Err("Expected identifier in params".to_string()),
                };
                let default = if matches!(self.peek(), JsTok::Eq(_)) {
                    self.bump();
                    Some(self.parse_expr(0)?)
                } else {
                    None
                };
                let _ = &lsp;
                params.push(JsParam { name, default, is_rest, span });
                if matches!(self.peek(), JsTok::Comma(_)) {
                    self.bump();
                    continue;
                }
                break;
            }
        }
        match self.bump() {
            JsTok::RParen(_) => Ok(params),
            _ => Err("Expected ')'".to_string()),
        }
    }

    fn parse_pattern(&mut self) -> Result<JsPattern, String> {
        match self.peek() {
            JsTok::Ident(_, _) => match self.bump() {
                JsTok::Ident(s, sp) => Ok(JsPattern::Ident(s, sp)),
                _ => unreachable!(),
            },
            JsTok::LBrace(sp) => {
                self.bump();
                let mut props = Vec::new();
                let mut rest = None;
                if !matches!(self.peek(), JsTok::RBrace(_)) {
                    loop {
                        if matches!(self.peek(), JsTok::Ellipsis(_)) {
                            self.bump();
                            rest = Some(match self.bump() {
                                JsTok::Ident(s, _) => s,
                                _ => return Err("Expected identifier after ... in object pattern".to_string()),
                            });
                        } else {
                            let key = match self.bump() {
                                JsTok::Ident(s, _) => s,
                                _ => return Err("Expected identifier in object pattern".to_string()),
                            };
                            let alias = if matches!(self.peek(), JsTok::Colon(_)) {
                                self.bump();
                                Some(match self.bump() {
                                    JsTok::Ident(s, _) => s,
                                    _ => return Err("Expected identifier after ':' in object pattern".to_string()),
                                })
                            } else {
                                None
                            };
                            props.push((key, alias));
                        }
                        if matches!(self.peek(), JsTok::Comma(_)) {
                            self.bump();
                            if matches!(self.peek(), JsTok::RBrace(_)) { break; }
                            continue;
                        }
                        break;
                    }
                }
                match self.bump() {
                    JsTok::RBrace(_) => Ok(JsPattern::Object { props, rest, span: sp }),
                    _ => Err("Expected '}' in object pattern".to_string()),
                }
            }
            JsTok::LBracket(sp) => {
                self.bump();
                let mut items: Vec<Option<String>> = Vec::new();
                let mut rest = None;
                if !matches!(self.peek(), JsTok::RBracket(_)) {
                    loop {
                        if matches!(self.peek(), JsTok::Comma(_)) {
                            self.bump();
                            items.push(None);
                            continue;
                        }
                        if matches!(self.peek(), JsTok::Ellipsis(_)) {
                            self.bump();
                            rest = Some(match self.bump() {
                                JsTok::Ident(s, _) => s,
                                _ => return Err("Expected identifier after ... in array pattern".to_string()),
                            });
                        } else if matches!(self.peek(), JsTok::Ident(_, _)) {
                            let name = match self.bump() { JsTok::Ident(s, _) => s, _ => unreachable!() };
                            items.push(Some(name));
                        } else {
                            return Err("Expected identifier or ...rest in array pattern".to_string());
                        }
                        if matches!(self.peek(), JsTok::Comma(_)) {
                            self.bump();
                            if matches!(self.peek(), JsTok::RBracket(_)) { break; }
                            continue;
                        }
                        break;
                    }
                }
                match self.bump() {
                    JsTok::RBracket(_) => Ok(JsPattern::Array { items, rest, span: sp }),
                    _ => Err("Expected ']' in array pattern".to_string()),
                }
            }
            _ => Err("Expected identifier or destructuring pattern".to_string()),
        }
    }

    fn parse_primary(&mut self) -> Result<JsExpr, String> {
        match self.bump() {
            JsTok::Ident(s, sp) => Ok(JsExpr::Ident(s, sp)),
            JsTok::String(s, sp) => Ok(JsExpr::String(s, sp)),
            JsTok::Number(n, sp) => Ok(JsExpr::Number(n, sp)),
            JsTok::True(sp) => Ok(JsExpr::Bool(true, sp)),
            JsTok::False(sp) => Ok(JsExpr::Bool(false, sp)),
            JsTok::Null(sp) => Ok(JsExpr::Null(sp)),
            JsTok::This(sp) => Ok(JsExpr::This(sp)),
            JsTok::Super(sp) => Ok(JsExpr::Super(sp)),
            JsTok::Backtick(sp) => {
                let mut parts: Vec<JsTemplatePart> = Vec::new();
                loop {
                    match self.peek() {
                        JsTok::Backtick(_) => {
                            self.bump();
                            break;
                        }
                        JsTok::TemplateChunk(_s, _) => {
                            let s = match self.bump() {
                                JsTok::TemplateChunk(ss, _) => ss,
                                _ => unreachable!(),
                            };
                            if !s.is_empty() {
                                parts.push(JsTemplatePart::Str(s));
                            }
                        }
                        JsTok::DollarLBrace(_) => {
                            self.bump(); // ${
                            let e = self.parse_expr(0)?;
                            match self.bump() {
                                JsTok::RBrace(_) => {}
                                _ => return Err("Expected '}' to close ${...}".to_string()),
                            }
                            parts.push(JsTemplatePart::Expr(e));
                        }
                        t => return Err(format!("Unexpected token in template literal: {:?}", t)),
                    }
                }
                Ok(JsExpr::TemplateLit { parts, span: sp })
            }
            JsTok::LParen(_) => {
                let e = self.parse_expr(0)?;
                match self.bump() {
                    JsTok::RParen(_) => Ok(e),
                    _ => Err("Expected ')'".to_string()),
                }
            }
            JsTok::LBracket(sp) => {
                let mut items = Vec::new();
                if !matches!(self.peek(), JsTok::RBracket(_)) {
                    loop {
                        if matches!(self.peek(), JsTok::Ellipsis(_)) {
                            let sp2 = match self.bump() { JsTok::Ellipsis(s) => s, _ => unreachable!() };
                            let inner = self.parse_expr(0)?;
                            items.push(JsExpr::Spread { expr: Box::new(inner), span: sp2 });
                        } else {
                            items.push(self.parse_expr(0)?);
                        }
                        if matches!(self.peek(), JsTok::Comma(_)) { self.bump(); continue; }
                        break;
                    }
                }
                match self.bump() {
                    JsTok::RBracket(_) => Ok(JsExpr::ArrayLit { items, span: sp }),
                    _ => Err("Expected ']'".to_string()),
                }
            }
            JsTok::LBrace(sp) => {
                let mut props = Vec::new();
                if !matches!(self.peek(), JsTok::RBrace(_)) {
                    loop {
                        let key = match self.bump() {
                            JsTok::Ident(s, _) => s,
                            JsTok::String(s, _) => s,
                            _ => return Err("Expected object key".to_string()),
                        };
                        match self.bump() {
                            JsTok::Colon(_) => {}
                            _ => return Err("Expected ':' in object literal".to_string()),
                        }
                        let val = self.parse_expr(0)?;
                        props.push((key, val));
                        if matches!(self.peek(), JsTok::Comma(_)) { self.bump(); continue; }
                        break;
                    }
                }
                match self.bump() {
                    JsTok::RBrace(_) => Ok(JsExpr::ObjectLit { props, span: sp }),
                    _ => Err("Expected '}' in object literal".to_string()),
                }
            }
            t => Err(format!("Unexpected token in JS: {:?}", t)),
        }
    }
}

