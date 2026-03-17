use crate::ast::{HtmlAttr, HtmlAttrValue, HtmlNode, Span};
use crate::parser::parse_expr_from_str;
use crate::html_lexer::{HtmlLexer, HtmlTok};

pub fn parse_html(raw: &str, span: Span) -> Result<HtmlNode, String> {
    let mut lx = HtmlLexer::new(raw, span.clone());
    while !matches!(lx.next(), HtmlTok::EOF(_)) {
        // We re-lex in a simple way: the lexer emits Text including tag bodies.
        // For MVP, parse a single root <div> wrapper if multiple top-level nodes exist.
        // We implement a tiny tag parser on the raw string directly for robustness.
        break;
    }
    let parsed = parse_nodes(raw, &span)?;
    let mut nodes = parsed;
    if nodes.len() == 1 {
        Ok(nodes.remove(0))
    } else {
        Ok(HtmlNode::Element { tag: "div".to_string(), attrs: vec![], children: nodes, span })
    }
}

fn parse_nodes(input: &str, span: &Span) -> Result<Vec<HtmlNode>, String> {
    // Minimal stack-based HTML parser: supports elements, attributes, text, and comments.
    // Not fully spec-compliant; intended for predictable EMA templates.
    let mut i = 0usize;
    let bytes = input.as_bytes();
    let mut out: Vec<HtmlNode> = Vec::new();
    let mut stack: Vec<(String, Vec<HtmlAttr>, Vec<HtmlNode>, Span)> = Vec::new();

    let push_node = |node: HtmlNode, out: &mut Vec<HtmlNode>, stack: &mut Vec<(String, Vec<HtmlAttr>, Vec<HtmlNode>, Span)>| {
        if let Some((_t, _a, children, _sp)) = stack.last_mut() {
            children.push(node);
        } else {
            out.push(node);
        }
    };

    while i < bytes.len() {
        // Template directives and interpolations at the stream level (can span tags).
        if input[i..].starts_with("{{") {
            let after = i + 2;
            if let Some(end) = input[after..].find("}}") {
                let abs_end = after + end;
                let inside = input[after..abs_end].trim();
                if let Ok(expr) = parse_expr_from_str(inside) {
                    push_node(HtmlNode::Interpolation { expr: Box::new(expr), span: span.clone() }, &mut out, &mut stack);
                } else {
                    push_node(HtmlNode::Text { text: input[i..abs_end + 2].to_string(), span: span.clone() }, &mut out, &mut stack);
                }
                i = abs_end + 2;
                continue;
            }
        }
        if input[i..].starts_with("{%") {
            let after = i + 2;
            let Some(end) = input[after..].find("%}") else {
                push_node(HtmlNode::Text { text: input[i..].to_string(), span: span.clone() }, &mut out, &mut stack);
                break;
            };
            let abs_end = after + end;
            let directive = input[after..abs_end].trim().to_string();
            let body_start = abs_end + 2;

            if directive.starts_with("if ") {
                let cond_src = directive.trim_start_matches("if").trim();
                let cond = parse_expr_from_str(cond_src).map_err(|e| format!("Invalid if condition: {}", e))?;
                let (then_src, else_src, next_i) = scan_if_block(input, body_start)?;
                let then_children = parse_nodes(then_src, span)?;
                let else_children = parse_nodes(else_src.unwrap_or(""), span)?;
                push_node(
                    HtmlNode::If {
                        condition: Box::new(cond),
                        then_children,
                        else_children,
                        span: span.clone(),
                    },
                    &mut out,
                    &mut stack,
                );
                i = next_i;
                continue;
            }
            if directive.starts_with("for ") {
                let rest = directive.trim_start_matches("for").trim();
                let Some((lhs, rhs)) = rest.split_once(" in ") else {
                    return Err("Invalid for directive. Expected: {% for item in expr %}".to_string());
                };
                let item = lhs.trim().to_string();
                let list = parse_expr_from_str(rhs.trim()).map_err(|e| format!("Invalid for list expr: {}", e))?;
                let (body_src, next_i) = scan_for_block(input, body_start)?;
                let body = parse_nodes(body_src, span)?;
                push_node(
                    HtmlNode::ForEach {
                        item,
                        index: None,
                        list: Box::new(list),
                        body,
                        span: span.clone(),
                    },
                    &mut out,
                    &mut stack,
                );
                i = next_i;
                continue;
            }

            // Unknown directive: preserve as text
            push_node(HtmlNode::Text { text: input[i..abs_end + 2].to_string(), span: span.clone() }, &mut out, &mut stack);
            i = abs_end + 2;
            continue;
        }

        if bytes[i] == b'<' {
            // Comment <!-- ... -->
            if input[i..].starts_with("<!--") {
                if let Some(end) = input[i + 4..].find("-->") {
                    let text = input[i + 4..i + 4 + end].to_string();
                    push_node(HtmlNode::Comment { text, span: span.clone() }, &mut out, &mut stack);
                    i = i + 4 + end + 3;
                    continue;
                } else {
                    return Err("Unclosed HTML comment".to_string());
                }
            }

            // End tag </tag>
            if input[i..].starts_with("</") {
                let j = input[i + 2..].find('>').ok_or("Unclosed end tag")?;
                let tag = input[i + 2..i + 2 + j].trim().to_string();
                i = i + 2 + j + 1;

                let (open_tag, attrs, children, sp) = stack.pop().ok_or("Unexpected end tag")?;
                if open_tag != tag {
                    return Err(format!("Mismatched end tag: expected </{}>, found </{}>", open_tag, tag));
                }
                push_node(HtmlNode::Element { tag: open_tag, attrs, children, span: sp }, &mut out, &mut stack);
                continue;
            }

            // Start tag <tag ...>
            let close = input[i..].find('>').ok_or("Unclosed start tag")?;
            let inside = &input[i + 1..i + close];
            let self_closing = inside.trim_end().ends_with('/');
            let inside = inside.trim().trim_end_matches('/').trim();
            let mut parts = inside.split_whitespace();
            let tag = parts.next().ok_or("Empty tag")?.to_string();
            let mut attrs: Vec<HtmlAttr> = Vec::new();

            let mut rest = inside[tag.len()..].trim();
            while !rest.is_empty() {
                // parse key="value" or key='value' or key=value or key
                let mut key = String::new();
                let mut klen = 0usize;
                for (idx, ch) in rest.char_indices() {
                    if ch.is_whitespace() || ch == '=' { break; }
                    key.push(ch);
                    klen = idx + ch.len_utf8();
                }
                rest = rest[klen..].trim_start();
                if key.is_empty() { break; }
                if rest.starts_with('=') {
                    rest = rest[1..].trim_start();
                    let (val, consumed) = if rest.starts_with('"') {
                        if let Some(endq) = rest[1..].find('"') {
                            (rest[1..1 + endq].to_string(), 1 + endq + 1)
                        } else { return Err("Unclosed attribute quote".to_string()); }
                    } else if rest.starts_with('\'') {
                        if let Some(endq) = rest[1..].find('\'') {
                            (rest[1..1 + endq].to_string(), 1 + endq + 1)
                        } else { return Err("Unclosed attribute quote".to_string()); }
                    } else {
                        // unquoted value until whitespace
                        let mut v = String::new();
                        let mut vlen = 0usize;
                        for (idx, ch) in rest.char_indices() {
                            if ch.is_whitespace() { break; }
                            v.push(ch);
                            vlen = idx + ch.len_utf8();
                        }
                        (v, vlen)
                    };
                    attrs.push(HtmlAttr { name: key, value: HtmlAttrValue::Static(val), span: span.clone() });
                    rest = rest[consumed..].trim_start();
                } else {
                    attrs.push(HtmlAttr { name: key, value: HtmlAttrValue::BoolTrue, span: span.clone() });
                }
            }

            i = i + close + 1;
            let void_tag = is_void_tag(&tag);
            if self_closing || void_tag {
                push_node(HtmlNode::Element { tag, attrs, children: vec![], span: span.clone() }, &mut out, &mut stack);
            } else {
                // Raw-text elements: script/style - capture everything until </tag>
                if is_raw_text_tag(&tag) {
                    let close_pat = format!("</{}>", tag);
                    if let Some(end) = input[i..].to_ascii_lowercase().find(&close_pat.to_ascii_lowercase()) {
                        let raw_text = input[i..i + end].to_string();
                        let mut children = Vec::new();
                        children.push(HtmlNode::Text { text: raw_text, span: span.clone() });
                        push_node(HtmlNode::Element { tag, attrs, children, span: span.clone() }, &mut out, &mut stack);
                        i = i + end + close_pat.len();
                        continue;
                    }
                }
                stack.push((tag, attrs, Vec::new(), span.clone()));
            }
            continue;
        }

        // Text until next '<'
        let start = i;
        while i < bytes.len() && bytes[i] != b'<' { i += 1; }
        let text = input[start..i].to_string();
        if !text.is_empty() {
            for n in split_template_nodes(&text, span)? {
                push_node(n, &mut out, &mut stack);
            }
        }
    }

    while let Some((tag, attrs, children, sp)) = stack.pop() {
        // Auto-close remaining tags (MVP)
        push_node(HtmlNode::Element { tag, attrs, children, span: sp }, &mut out, &mut Vec::new());
    }

    Ok(out)
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn is_raw_text_tag(tag: &str) -> bool {
    matches!(tag.to_ascii_lowercase().as_str(), "script" | "style")
}

#[allow(dead_code)]
fn decode_entities(s: &str) -> String {
    // Minimal entity decoding; extend later.
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn split_template_nodes(text: &str, span: &Span) -> Result<Vec<HtmlNode>, String> {
    // Supports:
    // - {{ expr }}
    // Note: block directives {% if/for %} are handled at the stream level in `parse_nodes`
    // so they can span across tags.
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < text.len() {
        let rest = &text[i..];
        let next_interp = rest.find("{{").map(|p| i + p);
        let next = next_interp;
        let Some(n) = next else {
            if i < text.len() {
                out.push(HtmlNode::Text { text: text[i..].to_string(), span: span.clone() });
            }
            break;
        };
        if n > i {
            out.push(HtmlNode::Text { text: text[i..n].to_string(), span: span.clone() });
        }
        if text[n..].starts_with("{{") {
            let after = n + 2;
            let Some(end) = text[after..].find("}}") else {
                out.push(HtmlNode::Text { text: text[n..].to_string(), span: span.clone() });
                break;
            };
            let abs_end = after + end;
            let inside = text[after..abs_end].trim();
            if let Ok(expr) = parse_expr_from_str(inside) {
                out.push(HtmlNode::Interpolation { expr: Box::new(expr), span: span.clone() });
            } else {
                out.push(HtmlNode::Text { text: text[n..abs_end + 2].to_string(), span: span.clone() });
            }
            i = abs_end + 2;
            continue;
        }
    }
    Ok(out)
}

fn scan_if_block<'a>(text: &'a str, start: usize) -> Result<(&'a str, Option<&'a str>, usize), String> {
    let mut depth = 1usize;
    let mut i = start;
    let then_start = start;
    let mut then_end = None;
    let mut else_start = None;

    while i < text.len() {
        if let Some(pos) = text[i..].find("{%") {
            let dstart = i + pos;
            let after = dstart + 2;
            let Some(dend_rel) = text[after..].find("%}") else { return Err("Unclosed directive in if block".to_string()); };
            let dend = after + dend_rel;
            let dir = text[after..dend].trim();
            if dir.starts_with("if ") {
                depth += 1;
            } else if dir == "endif" {
                depth -= 1;
                if depth == 0 {
                    let then_slice = &text[then_start..then_end.unwrap_or(dstart)];
                    let else_slice = else_start.map(|es| &text[es..dstart]);
                    return Ok((then_slice, else_slice, dend + 2));
                }
            } else if dir == "else" && depth == 1 {
                then_end = Some(dstart);
                else_start = Some(dend + 2);
            }
            i = dend + 2;
        } else {
            break;
        }
    }
    Err("Missing {% endif %}".to_string())
}

fn scan_for_block<'a>(text: &'a str, start: usize) -> Result<(&'a str, usize), String> {
    let mut depth = 1usize;
    let mut i = start;
    let body_start = start;
    while i < text.len() {
        if let Some(pos) = text[i..].find("{%") {
            let dstart = i + pos;
            let after = dstart + 2;
            let Some(dend_rel) = text[after..].find("%}") else { return Err("Unclosed directive in for block".to_string()); };
            let dend = after + dend_rel;
            let dir = text[after..dend].trim();
            if dir.starts_with("for ") {
                depth += 1;
            } else if dir == "endfor" {
                depth -= 1;
                if depth == 0 {
                    let body_slice = &text[body_start..dstart];
                    return Ok((body_slice, dend + 2));
                }
            }
            i = dend + 2;
        } else {
            break;
        }
    }
    Err("Missing {% endfor %}".to_string())
}

