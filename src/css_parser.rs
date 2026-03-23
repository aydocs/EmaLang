use crate::ast::{CssNode, CssRule, CssStylesheet, Span};
use crate::css_lexer::{CssLexer, CssTok};

pub fn parse_css(raw: &str, span: Span) -> Result<CssStylesheet, String> {
    let mut lx = CssLexer::new(raw, span.clone());
    let mut nodes = Vec::new();

    loop {
        let first = lx.next();
        match first {
            CssTok::EOF(_) => return Ok(CssStylesheet { nodes, span }),
            CssTok::AtDirective(name, sp) => {
                let mut params = String::new();
                loop {
                    match lx.next() {
                        CssTok::Semi(_) => {
                            nodes.push(CssNode::AtRule { name, params: params.trim().to_string(), span: sp });
                            break;
                        }
                        CssTok::LBrace(_) => {
                            let mut depth = 1;
                            while depth > 0 {
                                match lx.next() {
                                    CssTok::LBrace(_) => depth += 1,
                                    CssTok::RBrace(_) => depth -= 1,
                                    CssTok::EOF(_) => break,
                                    _ => {}
                                }
                            }
                            nodes.push(CssNode::AtRule { name, params: params.trim().to_string(), span: sp });
                            break;
                        }
                        CssTok::EOF(_) => {
                            nodes.push(CssNode::AtRule { name, params: params.trim().to_string(), span: sp });
                            return Ok(CssStylesheet { nodes, span });
                        }
                        CssTok::Ident(s, _) => { params.push_str(&s); params.push(' '); }
                        CssTok::String(s, _) => { params.push('"'); params.push_str(&s); params.push('"'); }
                        CssTok::Dot(_) => params.push('.'),
                        CssTok::Hash(_) => params.push('#'),
                        CssTok::Other(c, _) => params.push(c),
                        _ => {}
                    }
                }
            }
            _ => {
                // Rule parsing
                let mut selectors = String::new();
                let mut cur = first;
                loop {
                    match cur {
                        CssTok::LBrace(_) => break,
                        CssTok::EOF(_) => return Ok(CssStylesheet { nodes, span }),
                        CssTok::Ident(s, _) => { selectors.push_str(&s); selectors.push(' '); }
                        CssTok::Hash(_) => selectors.push('#'),
                        CssTok::Dot(_) => selectors.push('.'),
                        CssTok::Comma(_) => selectors.push(','),
                        CssTok::Colon(_) => selectors.push(':'),
                        CssTok::String(s, _) => { selectors.push('"'); selectors.push_str(&s); selectors.push('"'); }
                        CssTok::Other(ch, _) => selectors.push(ch),
                        CssTok::RBrace(_) | CssTok::Semi(_) | CssTok::AtDirective(_, _) => {}
                    }
                    cur = lx.next();
                }

                let sel_str = selectors.trim().to_string();
                let selectors_vec: Vec<String> = sel_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let mut decls: Vec<(String, String)> = Vec::new();
                loop {
                    match lx.next() {
                        CssTok::EOF(_) => break,
                        CssTok::RBrace(_) => break,
                        CssTok::Ident(prop, _) => {
                            let mut tok = lx.next();
                            while matches!(tok, CssTok::Other(' ', _) | CssTok::Other('\n', _) | CssTok::Other('\t', _)) { tok = lx.next(); }
                            if !matches!(tok, CssTok::Colon(_)) { continue; }
                            
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
                                    _ => {}
                                }
                            }
                            if !hit_rbrace {
                                decls.push((prop, val.trim().to_string()));
                            }
                        }
                        _ => {}
                    }
                }
                nodes.push(CssNode::Rule(CssRule { selectors: selectors_vec, declarations: decls, span: span.clone() }));
            }
        }
    }
}
