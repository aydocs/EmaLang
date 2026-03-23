use std::io::{self, BufRead, Write, Read};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::analyzer::{Analyzer, SymbolInfo};

pub async fn run_lsp_server() {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut open_files: HashMap<String, String> = HashMap::new();
    
    loop {
        let mut line = String::new();
        if handle.read_line(&mut line).unwrap() == 0 { break; }
        
        if line.starts_with("Content-Length: ") {
            let length: usize = line["Content-Length: ".len()..].trim().parse().unwrap();
            eprintln!("[LSP] Content-Length: {}", length);
            
            // Skip empty line after Content-Length
            let mut empty_line = String::new();
            handle.read_line(&mut empty_line).unwrap();
            
            let mut body = vec![0u8; length];
            handle.read_exact(&mut body).unwrap();
            
            let msg: Value = serde_json::from_slice(&body).unwrap();
            eprintln!("[LSP] Received: {}", msg);
            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                match method {
                    "initialize" => {
                        let res = json!({
                            "jsonrpc": "2.0",
                            "id": msg["id"],
                            "result": {
                                "capabilities": {
                                    "textDocumentSync": 1, // Full sync
                                    "hoverProvider": true,
                                    "definitionProvider": true
                                }
                            }
                        });
                        send_msg(&res);
                    }
                    "textDocument/didOpen" | "textDocument/didChange" => {
                        if let Some(params) = msg.get("params") {
                            let uri = params["textDocument"]["uri"].as_str().unwrap().to_string();
                            let text = if method == "textDocument/didOpen" {
                                params["textDocument"]["text"].as_str().unwrap().to_string()
                            } else {
                                params["contentChanges"][0]["text"].as_str().unwrap().to_string()
                            };
                            
                            open_files.insert(uri.clone(), text.clone());
                            handle_diagnostics(&uri, &text);
                        }
                    }
                    "textDocument/hover" => {
                        if let Some(params) = msg.get("params") {
                            let uri = params["textDocument"]["uri"].as_str().unwrap();
                            let pos = &params["position"];
                            let line = pos["line"].as_u64().unwrap() as usize + 1;
                            let col = pos["character"].as_u64().unwrap() as usize + 1;
                            
                            if let Some(text) = open_files.get(uri) {
                                let (diagnostics, symbols) = analyze_text(text);
                                
                                // Find symbol at pos
                                let mut result = json!(null);
                                for sym in symbols {
                                    if sym.span.line == line && col >= sym.span.col && col <= sym.span.col + sym.name.len() {
                                        let hover_text = format!("**{}**: `{:?}`", sym.name, sym.ema_type);
                                        result = json!({
                                            "contents": {
                                                "kind": "markdown",
                                                "value": hover_text
                                            }
                                        });
                                        break;
                                    }
                                }
                                
                                let res = json!({
                                    "jsonrpc": "2.0",
                                    "id": msg["id"],
                                    "result": result
                                });
                                send_msg(&res);
                            }
                        }
                    }
                    "textDocument/definition" => {
                        if let Some(params) = msg.get("params") {
                            let uri = params["textDocument"]["uri"].as_str().unwrap();
                            let pos = &params["position"];
                            let line = pos["line"].as_u64().unwrap() as usize + 1;
                            let col = pos["character"].as_u64().unwrap() as usize + 1;

                            if let Some(text) = open_files.get(uri) {
                                let (_, symbols) = analyze_text(text);
                                
                                // Find symbol at pos
                                let mut result = json!(null);
                                if let Some(target_sym) = symbols.iter().find(|s| s.span.line == line && col >= s.span.col && col <= s.span.col + s.name.len()) {
                                    eprintln!("[LSP] Found target symbol: {:?} at {}:{}", target_sym.name, line, col);
                                    // Find declaration (first occurrence or specific type if we add it)
                                    if let Some(decl) = symbols.iter().find(|s| s.name == target_sym.name) {
                                        eprintln!("[LSP] Found declaration: {:?} at {}:{}", decl.name, decl.span.line, decl.span.col);
                                        result = json!({
                                            "uri": uri,
                                            "range": {
                                                "start": { "line": decl.span.line - 1, "character": decl.span.col },
                                                "end": { "line": decl.span.line - 1, "character": decl.span.col + decl.name.len() }
                                            }
                                        });
                                    }
                                } else {
                                    eprintln!("[LSP] No symbol found at {}:{}", line, col);
                                }
                                
                                let res = json!({
                                    "jsonrpc": "2.0",
                                    "id": msg["id"],
                                    "result": result
                                });
                                send_msg(&res);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn analyze_text(text: &str) -> (Vec<crate::analyzer::Diagnostic>, Vec<SymbolInfo>) {
    eprintln!("[LSP] Analyzing text (len={})", text.len());
    let mut lexer = Lexer::new(text);
    let mut tokens = Vec::new();
    loop {
        let t = lexer.next_token();
        tokens.push(t.clone());
        if t.token == crate::lexer::Token::EOF { break; }
    }
    eprintln!("[LSP] Lexing complete: {} tokens", tokens.len());
    
    let mut parser = Parser::new(tokens);
    let ast = parser.parse();
    eprintln!("[LSP] Parsing complete: {} statements", ast.statements.len());
    
    let mut analyzer = Analyzer::new(false);
    let diagnostics = analyzer.analyze(&ast);
    eprintln!("[LSP] Analysis complete: {} diagnostics, {} symbols", diagnostics.len(), analyzer.symbols.len());
    (diagnostics, analyzer.symbols)
}

fn handle_diagnostics(uri: &str, text: &str) {
    let (diagnostics, _) = analyze_text(text);
    
    let lsp_diagnostics: Vec<Value> = diagnostics.into_iter().map(|d| {
        json!({
            "range": {
                "start": { "line": d.line - 1, "character": d.col },
                "end": { "line": d.line - 1, "character": d.col + 5 } // Placeholder
            },
            "severity": if d.severity == "error" { 1 } else { 2 },
            "message": d.message,
            "source": "ema"
        })
    }).collect();
    
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": lsp_diagnostics
        }
    });
    send_msg(&notification);
}

fn send_msg(msg: &Value) {
    let body = serde_json::to_string(msg).unwrap();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut stdout = io::stdout();
    stdout.write_all(header.as_bytes()).unwrap();
    stdout.write_all(body.as_bytes()).unwrap();
    stdout.flush().unwrap();
}
