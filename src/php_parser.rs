use crate::ast::{PhpExpr, PhpProgram, PhpStmt, Span};
use crate::php_lexer::{PhpLexer, PhpTok};

pub fn parse_php(raw: &str, span: Span) -> Result<PhpProgram, String> {
    let mut p = Parser { lx: PhpLexer::new(raw, span.clone()), cur: None };
    p.bump();
    let mut body = Vec::new();
    while !matches!(p.cur, Some(PhpTok::EOF(_))) {
        body.push(p.parse_stmt()?);
    }
    Ok(PhpProgram { body, span })
}

struct Parser<'a> {
    lx: PhpLexer<'a>,
    cur: Option<PhpTok>,
}

impl<'a> Parser<'a> {
    fn bump(&mut self) { self.cur = Some(self.lx.next()); }

    fn expect_semi(&mut self) -> Result<(), String> {
        match self.cur.clone().unwrap() {
            PhpTok::Semi(_) => { self.bump(); Ok(()) }
            _ => Err("Expected ';'".to_string()),
        }
    }

    fn parse_stmt(&mut self) -> Result<PhpStmt, String> {
        match self.cur.clone().unwrap() {
            PhpTok::Echo(sp) => { self.bump(); let e = self.parse_expr()?; self.expect_semi()?; Ok(PhpStmt::Echo(e, sp)) }
            PhpTok::Print(sp) => { self.bump(); let e = self.parse_expr()?; self.expect_semi()?; Ok(PhpStmt::Print(e, sp)) }
            PhpTok::If(sp) => self.parse_if(sp),
            PhpTok::While(sp) => self.parse_while(sp),
            PhpTok::For(sp) => self.parse_for(sp),
            PhpTok::DollarIdent(name, sp) => {
                self.bump();
                if matches!(self.cur, Some(PhpTok::Assign(_))) {
                    self.bump();
                    let val = self.parse_expr()?;
                    self.expect_semi()?;
                    Ok(PhpStmt::Assign { name, value: val, span: sp })
                } else {
                    // expression statement starting with $var
                    let e = self.parse_postfix_from(PhpExpr::Var(name, sp.clone()))?;
                    self.expect_semi()?;
                    Ok(PhpStmt::Expr(e, sp))
                }
            }
            _ => {
                let sp = Span { line: 0, col: 0 };
                let e = self.parse_expr()?;
                self.expect_semi()?;
                Ok(PhpStmt::Expr(e, sp))
            }
        }
    }

    fn parse_if(&mut self, sp: Span) -> Result<PhpStmt, String> {
        self.bump(); // consume if
        if !matches!(self.cur, Some(PhpTok::LParen(_))) { return Err("Expected '(' after if".to_string()); }
        self.bump();
        let cond = self.parse_expr()?;
        if !matches!(self.cur, Some(PhpTok::RParen(_))) { return Err("Expected ')' after if condition".to_string()); }
        self.bump();
        let then_branch = self.parse_block()?;
        let mut else_branch: Option<Vec<PhpStmt>> = None;
        // elseif chain becomes nested if in else_branch
        while matches!(self.cur, Some(PhpTok::ElseIf(_))) {
            let esp = match self.cur.clone().unwrap() { PhpTok::ElseIf(s) => s, _ => unreachable!() };
            self.bump(); // elseif
            if !matches!(self.cur, Some(PhpTok::LParen(_))) { return Err("Expected '(' after elseif".to_string()); }
            self.bump();
            let econd = self.parse_expr()?;
            if !matches!(self.cur, Some(PhpTok::RParen(_))) { return Err("Expected ')' after elseif condition".to_string()); }
            self.bump();
            let ethen = self.parse_block()?;
            let nested = PhpStmt::If { condition: econd, then_branch: ethen, else_branch: None, span: esp };
            else_branch = Some(vec![nested]);
            // allow chaining: elseif (...) { } elseif (...) { } else { }
            if matches!(self.cur, Some(PhpTok::ElseIf(_))) {
                // continue loop, but we need to attach next elseif to the last nested if's else.
                // Minimal implementation: wrap by updating else_branch later in lowering/runtime.
                // Here: keep parsing sequentially by turning next elseif into else { if ... }
                // We'll implement by reconstructing: current else_branch holds last nested, and we keep nesting.
                // To do that, we re-enter loop and, if we parse another elseif, we will set it as else_branch of previous nested.
                // Simpler: break and let later parsing handle only one elseif. But we want full chain.
            }
            // If next is elseif, we need to nest it inside the previous nested if.
            if matches!(self.cur, Some(PhpTok::ElseIf(_))) {
                // rewrite else_branch nesting by parsing remaining elseif/else using recursion:
                // create a fake "if" tail: if (...) { ... } <tail>
                // We'll handle by calling parse_if_tail-like helper on current else_branch[0].
                let tail = self.parse_if_tail()?;
                if let Some(v) = else_branch.as_mut() {
                    if let Some(PhpStmt::If { else_branch: eb, .. }) = v.get_mut(0) {
                        *eb = tail;
                    }
                }
                break;
            }
        }
        if matches!(self.cur, Some(PhpTok::Else(_))) {
            self.bump();
            else_branch = Some(self.parse_block()?);
        }
        Ok(PhpStmt::If { condition: cond, then_branch, else_branch, span: sp })
    }

    fn parse_if_tail(&mut self) -> Result<Option<Vec<PhpStmt>>, String> {
        // Parses: (elseif (...) {...})* (else {...})?
        if matches!(self.cur, Some(PhpTok::ElseIf(_))) {
            let sp = match self.cur.clone().unwrap() { PhpTok::ElseIf(s) => s, _ => unreachable!() };
            self.bump();
            if !matches!(self.cur, Some(PhpTok::LParen(_))) { return Err("Expected '(' after elseif".to_string()); }
            self.bump();
            let cond = self.parse_expr()?;
            if !matches!(self.cur, Some(PhpTok::RParen(_))) { return Err("Expected ')' after elseif condition".to_string()); }
            self.bump();
            let then_branch = self.parse_block()?;
            let else_branch = self.parse_if_tail()?;
            return Ok(Some(vec![PhpStmt::If { condition: cond, then_branch, else_branch, span: sp }]));
        }
        if matches!(self.cur, Some(PhpTok::Else(_))) {
            self.bump();
            return Ok(Some(self.parse_block()?));
        }
        Ok(None)
    }

    fn parse_while(&mut self, sp: Span) -> Result<PhpStmt, String> {
        self.bump(); // consume while
        if !matches!(self.cur, Some(PhpTok::LParen(_))) { return Err("Expected '(' after while".to_string()); }
        self.bump();
        let cond = self.parse_expr()?;
        if !matches!(self.cur, Some(PhpTok::RParen(_))) { return Err("Expected ')' after while condition".to_string()); }
        self.bump();
        let body = self.parse_block()?;
        Ok(PhpStmt::While { condition: cond, body, span: sp })
    }

    fn parse_for(&mut self, sp: Span) -> Result<PhpStmt, String> {
        self.bump(); // consume for
        if !matches!(self.cur, Some(PhpTok::LParen(_))) { return Err("Expected '(' after for".to_string()); }
        self.bump();

        // init: either assignment ($x=...) or empty
        let init = if matches!(self.cur, Some(PhpTok::Semi(_))) {
            self.bump();
            None
        } else if matches!(self.cur, Some(PhpTok::DollarIdent(_, _))) {
            // Parse "$x = expr" as a statement (without requiring trailing ';' here)
            let (name, nsp) = match self.cur.clone().unwrap() {
                PhpTok::DollarIdent(n, s) => (n, s),
                _ => unreachable!(),
            };
            self.bump();
            if !matches!(self.cur, Some(PhpTok::Assign(_))) { return Err("Expected '=' in for init".to_string()); }
            self.bump();
            let val = self.parse_expr()?;
            if !matches!(self.cur, Some(PhpTok::Semi(_))) { return Err("Expected ';' after for init".to_string()); }
            self.bump();
            Some(Box::new(PhpStmt::Assign { name, value: val, span: nsp }))
        } else {
            // expression init; represent as Expr statement
            let e = self.parse_expr()?;
            if !matches!(self.cur, Some(PhpTok::Semi(_))) { return Err("Expected ';' after for init".to_string()); }
            self.bump();
            Some(Box::new(PhpStmt::Expr(e, sp.clone())))
        };

        let condition = if matches!(self.cur, Some(PhpTok::Semi(_))) {
            self.bump();
            None
        } else {
            let e = self.parse_expr()?;
            if !matches!(self.cur, Some(PhpTok::Semi(_))) { return Err("Expected ';' after for condition".to_string()); }
            self.bump();
            Some(e)
        };

        let update = if matches!(self.cur, Some(PhpTok::RParen(_))) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        if !matches!(self.cur, Some(PhpTok::RParen(_))) { return Err("Expected ')' after for clause".to_string()); }
        self.bump();

        let body = self.parse_block()?;
        Ok(PhpStmt::For { init, condition, update, body, span: sp })
    }

    fn parse_block(&mut self) -> Result<Vec<PhpStmt>, String> {
        if !matches!(self.cur, Some(PhpTok::LBrace(_))) { return Err("Expected '{'".to_string()); }
        self.bump();
        let mut out = Vec::new();
        while !matches!(self.cur, Some(PhpTok::RBrace(_))) && !matches!(self.cur, Some(PhpTok::EOF(_))) {
            out.push(self.parse_stmt()?);
        }
        if !matches!(self.cur, Some(PhpTok::RBrace(_))) { return Err("Expected '}'".to_string()); }
        self.bump();
        Ok(out)
    }

    fn parse_expr(&mut self) -> Result<PhpExpr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<PhpExpr, String> {
        let mut left = self.parse_and()?;
        loop {
            if matches!(self.cur, Some(PhpTok::OrOr(_))) {
                let sp = match self.cur.clone().unwrap() { PhpTok::OrOr(s) => s, _ => unreachable!() };
                self.bump();
                let right = self.parse_and()?;
                left = PhpExpr::Binary { left: Box::new(left), op: "||".to_string(), right: Box::new(right), span: sp };
                continue;
            }
            break;
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<PhpExpr, String> {
        let mut left = self.parse_cmp()?;
        loop {
            if matches!(self.cur, Some(PhpTok::AndAnd(_))) {
                let sp = match self.cur.clone().unwrap() { PhpTok::AndAnd(s) => s, _ => unreachable!() };
                self.bump();
                let right = self.parse_cmp()?;
                left = PhpExpr::Binary { left: Box::new(left), op: "&&".to_string(), right: Box::new(right), span: sp };
                continue;
            }
            break;
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<PhpExpr, String> {
        let mut left = self.parse_concat()?;
        loop {
            match self.cur.clone().unwrap() {
                PhpTok::Less(sp) => { self.bump(); let r = self.parse_concat()?; left = PhpExpr::Binary { left: Box::new(left), op: "<".to_string(), right: Box::new(r), span: sp }; }
                PhpTok::LessEq(sp) => { self.bump(); let r = self.parse_concat()?; left = PhpExpr::Binary { left: Box::new(left), op: "<=".to_string(), right: Box::new(r), span: sp }; }
                PhpTok::Greater(sp) => { self.bump(); let r = self.parse_concat()?; left = PhpExpr::Binary { left: Box::new(left), op: ">".to_string(), right: Box::new(r), span: sp }; }
                PhpTok::GreaterEq(sp) => { self.bump(); let r = self.parse_concat()?; left = PhpExpr::Binary { left: Box::new(left), op: ">=".to_string(), right: Box::new(r), span: sp }; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<PhpExpr, String> {
        let mut left = self.parse_add()?;
        loop {
            if matches!(self.cur, Some(PhpTok::Dot(_))) {
                let sp = match self.cur.clone().unwrap() { PhpTok::Dot(s) => s, _ => unreachable!() };
                self.bump();
                let right = self.parse_add()?;
                left = PhpExpr::Binary { left: Box::new(left), op: ".".to_string(), right: Box::new(right), span: sp };
                continue;
            }
            break;
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<PhpExpr, String> {
        let mut left = self.parse_eq()?;
        loop {
            match self.cur.clone().unwrap() {
                PhpTok::Plus(sp) => { self.bump(); let r = self.parse_eq()?; left = PhpExpr::Binary { left: Box::new(left), op: "+".to_string(), right: Box::new(r), span: sp }; }
                PhpTok::Minus(sp) => { self.bump(); let r = self.parse_eq()?; left = PhpExpr::Binary { left: Box::new(left), op: "-".to_string(), right: Box::new(r), span: sp }; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_eq(&mut self) -> Result<PhpExpr, String> {
        let mut left = self.parse_postfix()?;
        loop {
            match self.cur.clone().unwrap() {
                PhpTok::EqEq(sp) => { self.bump(); let r = self.parse_postfix()?; left = PhpExpr::Binary { left: Box::new(left), op: "==".to_string(), right: Box::new(r), span: sp }; }
                PhpTok::BangEq(sp) => { self.bump(); let r = self.parse_postfix()?; left = PhpExpr::Binary { left: Box::new(left), op: "!=".to_string(), right: Box::new(r), span: sp }; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<PhpExpr, String> {
        let primary = self.parse_primary()?;
        self.parse_postfix_from(primary)
    }

    fn parse_postfix_from(&mut self, mut expr: PhpExpr) -> Result<PhpExpr, String> {
        loop {
            if matches!(self.cur, Some(PhpTok::LParen(_))) {
                let sp = match self.cur.clone().unwrap() { PhpTok::LParen(s) => s, _ => unreachable!() };
                self.bump();
                let mut args = Vec::new();
                if !matches!(self.cur, Some(PhpTok::RParen(_))) {
                    loop {
                        args.push(self.parse_expr()?);
                        if matches!(self.cur, Some(PhpTok::Comma(_))) { self.bump(); continue; }
                        break;
                    }
                }
                if !matches!(self.cur, Some(PhpTok::RParen(_))) { return Err("Expected ')'".to_string()); }
                self.bump();
                let name = match expr {
                    PhpExpr::Var(n, _) => n,
                    PhpExpr::String(n, _) => n,
                    PhpExpr::Null(_) => "null".to_string(),
                    _ => return Err("Only simple function calls are supported in PHP subset".to_string()),
                };
                expr = PhpExpr::Call { name, args, span: sp };
                continue;
            }
            if matches!(self.cur, Some(PhpTok::LBracket(_))) {
                let sp = match self.cur.clone().unwrap() { PhpTok::LBracket(s) => s, _ => unreachable!() };
                self.bump();
                let idx = self.parse_expr()?;
                if !matches!(self.cur, Some(PhpTok::RBracket(_))) { return Err("Expected ']'".to_string()); }
                self.bump();
                expr = PhpExpr::Index { target: Box::new(expr), index: Box::new(idx), span: sp };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<PhpExpr, String> {
        match self.cur.clone().unwrap() {
            PhpTok::DollarIdent(n, sp) => { self.bump(); Ok(PhpExpr::Var(n, sp)) }
            PhpTok::Ident(n, sp) => { self.bump(); Ok(PhpExpr::String(n, sp)) } // identifiers as strings in MVP
            PhpTok::String(s, sp) => { self.bump(); Ok(PhpExpr::String(s, sp)) }
            PhpTok::Int(v, sp) => { self.bump(); Ok(PhpExpr::Int(v, sp)) }
            PhpTok::Float(v, sp) => { self.bump(); Ok(PhpExpr::Float(v, sp)) }
            PhpTok::True(sp) => { self.bump(); Ok(PhpExpr::Bool(true, sp)) }
            PhpTok::False(sp) => { self.bump(); Ok(PhpExpr::Bool(false, sp)) }
            PhpTok::LParen(_) => {
                self.bump();
                let e = self.parse_expr()?;
                if !matches!(self.cur, Some(PhpTok::RParen(_))) { return Err("Expected ')'".to_string()); }
                self.bump();
                Ok(e)
            }
            PhpTok::LBracket(sp) => {
                self.bump();
                let mut items = Vec::new();
                if !matches!(self.cur, Some(PhpTok::RBracket(_))) {
                    loop {
                        items.push(self.parse_expr()?);
                        if matches!(self.cur, Some(PhpTok::Comma(_))) { self.bump(); continue; }
                        break;
                    }
                }
                if !matches!(self.cur, Some(PhpTok::RBracket(_))) { return Err("Expected ']'".to_string()); }
                self.bump();
                Ok(PhpExpr::ArrayLit { items, span: sp })
            }
            PhpTok::LBrace(sp) => {
                // object literal in expression context: { "k": expr, a: expr }
                self.bump();
                let mut props = Vec::new();
                if !matches!(self.cur, Some(PhpTok::RBrace(_))) {
                    loop {
                        let key = match self.cur.clone().unwrap() {
                            PhpTok::String(s, _) => { self.bump(); s }
                            PhpTok::Ident(s, _) => { self.bump(); s }
                            _ => return Err("Expected object key".to_string()),
                        };
                        if !matches!(self.cur, Some(PhpTok::Colon(_))) { return Err("Expected ':' in object literal".to_string()); }
                        self.bump();
                        let val = self.parse_expr()?;
                        props.push((key, val));
                        if matches!(self.cur, Some(PhpTok::Comma(_))) { self.bump(); continue; }
                        break;
                    }
                }
                if !matches!(self.cur, Some(PhpTok::RBrace(_))) { return Err("Expected '}' in object literal".to_string()); }
                self.bump();
                Ok(PhpExpr::ObjectLit { props, span: sp })
            }
            PhpTok::EOF(_) => Ok(PhpExpr::Null(Span { line: 0, col: 0 })),
            _ => Err("Unexpected token in PHP expression".to_string()),
        }
    }
}

