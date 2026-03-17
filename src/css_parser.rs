use crate::ast::{CssRule, CssStylesheet, Span};
use crate::css_lexer::{CssLexer, CssTok};

pub fn parse_css(raw: &str, span: Span) -> Result<CssStylesheet, String> {
    let mut lx = CssLexer::new(raw, span.clone());
    let mut rules = Vec::new();

    loop {
        let mut selectors = String::new();
        // read selectors until '{'
        loop {
            match lx.next() {
                CssTok::EOF(_) => return Ok(CssStylesheet { rules, span }),
                CssTok::LBrace(_) => break,
                CssTok::RBrace(_) => continue,
                CssTok::Semi(_) => { selectors.clear(); },
                CssTok::Ident(s, _) => { selectors.push_str(&s); selectors.push(' '); }
                CssTok::Hash(_) => selectors.push('#'),
                CssTok::Dot(_) => selectors.push('.'),
                CssTok::Comma(_) => selectors.push(','),
                CssTok::Colon(_) => selectors.push(':'),
                CssTok::String(s, _) => { selectors.push('"'); selectors.push_str(&s); selectors.push('"'); }
                CssTok::Other(ch, _) => selectors.push(ch),
            }
        }
        let sel_str = selectors.trim().to_string();
        if sel_str.is_empty() {
            // consume decls anyway
        }
        let selectors_vec: Vec<String> = sel_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut decls: Vec<(String, String)> = Vec::new();
        // parse declarations until '}'
        loop {
            match lx.next() {
                CssTok::EOF(_) => break,
                CssTok::RBrace(_) => break,
                CssTok::Ident(prop, _) => {
                    // expect ':'
                    let mut tok = lx.next();
                    while matches!(tok, CssTok::Other(_, _)) { tok = lx.next(); }
                    if !matches!(tok, CssTok::Colon(_)) {
                        continue;
                    }
                    // read value until ';' or '}'
                    let mut val = String::new();
                    let mut hit_rbrace = false;
                    loop {
                        match lx.next() {
                            CssTok::Semi(_) => break,
                            CssTok::RBrace(_) => {
                                decls.push((prop.clone(), val.trim().to_string()));
                                hit_rbrace = true;
                                break;
                            }
                            CssTok::EOF(_) => break,
                            CssTok::Ident(s, _) => { val.push_str(&s); val.push(' '); }
                            CssTok::Hash(_) => val.push('#'),
                            CssTok::Dot(_) => val.push('.'),
                            CssTok::Comma(_) => val.push(','),
                            CssTok::Colon(_) => val.push(':'),
                            CssTok::String(s, _) => { val.push('"'); val.push_str(&s); val.push('"'); }
                            CssTok::Other(ch, _) => val.push(ch),
                            CssTok::LBrace(_) => val.push('{'),
                        }
                    }
                    if !hit_rbrace {
                        decls.push((prop, val.trim().to_string()));
                    }
                }
                _ => {}
            }
        }

        rules.push(CssRule { selectors: selectors_vec, declarations: decls, span: span.clone() });
    }
}

