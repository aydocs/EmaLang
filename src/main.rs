mod lexer;
mod parser;
mod ast;
mod runtime;
mod compiler;
mod wasm_builder;
mod analyzer;
mod db;
mod html_lexer;
mod html_parser;
mod css_lexer;
mod css_parser;
mod js_lexer;
mod js_parser;
mod php_lexer;
mod php_parser;

use std::fs;
use std::env;
use sha2::{Digest, Sha256};

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut check_mode = false;
    let mut json_mode = false;
    let mut strict_embedded = false;
    let mut filename = String::new();

    for arg in args.iter().skip(1) {
        if arg == "--check" {
            check_mode = true;
        } else if arg == "--json" {
            json_mode = true;
        } else if arg == "--strict-embedded" {
            strict_embedded = true;
        } else if filename.is_empty() {
            filename = arg.clone();
        }
    }

    if filename.is_empty() {
        println!("Usage: ema <file.ema> [--check] [--json]");
        return;
    }

    let source_code = fs::read_to_string(&filename).expect("Failed to read selected .ema file!");

    if !check_mode {
        println!("=== EMA SYSTEM COMPILER BOOTSTRAP ===");
        println!(">>> Ema Dosyasi Yukleniyor: {}", filename);
    }

    // --- Sozcuklere Ayirma (Lexical Analysis) ---
    let mut lexer = lexer::Lexer::new(&source_code);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token();
        tokens.push(tok.clone());
        if tok.token == crate::lexer::Token::EOF { break; }
    }

    // --- Parsing (Syntactic Analysis) ---
    let mut parser = parser::Parser::new(tokens);
    let ast = parser.parse();
    
    // --- Semantic Analysis (Type Checking) ---
    let mut semantic_analyzer = analyzer::Analyzer::new(strict_embedded);
    let diagnostics = semantic_analyzer.analyze(&ast);
    let has_error = diagnostics.iter().any(|d| d.severity == "error");

    if check_mode {
        if json_mode {
            if !diagnostics.is_empty() {
                println!("{}", serde_json::to_string(&diagnostics).unwrap());
            }
        } else {
            if diagnostics.is_empty() {
                println!("OK");
            } else {
                for d in diagnostics {
                    let pfx = if d.severity == "warning" { "Warning" } else { "Error" };
                    println!("{} at {}:{}: {}", pfx, d.line, d.col, d.message);
                }
            }
        }
        std::process::exit(if has_error { 1 } else { 0 });
    }

    // Normal Execution Flow
    println!("\n--- Parse Output ---");
    println!("{:#?}", ast);

    println!("\n--- Semantic Analysis Succeeded ---");

    println!("\n--- WebAssembly UI Build (Frontend) ---");
    let mut wasm_compiler = wasm_builder::WasmBuilder::new();
    let js_code = wasm_compiler.build_frontend(&ast);
    fs::write("frontend.js", js_code).expect("Failed to write JS/WASM frontend output!");
    println!(">>> Browser output (WASM/JS) generated: frontend.js");

    let ssr_html = wasm_compiler.build_ssr_html(&ast);
    fs::write("frontend.ssr.html", ssr_html).expect("Failed to write SSR HTML output!");
    println!(">>> SSR HTML generated: frontend.ssr.html");

    let ssr_css = wasm_compiler.build_ssr_css(&ast);
    fs::write("frontend.ssr.css", ssr_css).expect("Failed to write SSR CSS output!");
    println!(">>> SSR CSS generated: frontend.ssr.css");

    // Build fingerprint for SSR/client mismatch detection
    let mut hasher = Sha256::new();
    if let Ok(h) = fs::read("frontend.ssr.html") { hasher.update(&h); }
    if let Ok(c) = fs::read("frontend.ssr.css") { hasher.update(&c); }
    let build_hash = format!("{:x}", hasher.finalize());

    // Patch frontend.js placeholder with computed hash (so client can verify SSR build).
    if let Ok(js) = fs::read_to_string("frontend.js") {
        let patched = js.replace("__EMA_EXPECTED_HASH__", &build_hash);
        fs::write("frontend.js", patched).expect("Failed to patch frontend.js build hash!");
    }

    // Separate JS fingerprint for caching / cache-busting frontend.js.
    let mut js_hasher = Sha256::new();
    if let Ok(js_bytes) = fs::read("frontend.js") { js_hasher.update(&js_bytes); }
    let js_hash = format!("{:x}", js_hasher.finalize());

    let build_info = serde_json::json!({ "hash": build_hash, "jsHash": js_hash });
    fs::write("frontend.build.json", build_info.to_string()).expect("Failed to write build info!");
    println!(">>> Build hashes generated: frontend.build.json");

    println!("\n--- Compilation (LLVM IR generation) ---");
    let mut llvm_compiler = compiler::LlvmCompiler::new();
    let ir = llvm_compiler.compile(&ast);
    fs::write("output.ll", ir).expect("Failed to write LLVM output!");
    println!(">>> LLVM Intermediate Representation (IR) generated: output.ll");

    println!("\n--- Execution (Runtime) ---");
    let mut interpreter = runtime::Interpreter::new();
    interpreter.eval_program(ast.clone());

    println!("\n=== COMPILATION & EXECUTION SUCCESS ===");
}
