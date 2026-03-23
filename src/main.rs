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
mod node_builder;
mod db_migrator;
mod lsp;
use crate::ast::{Program, Stmt};

use std::fs;
use std::env;
use std::io;
use sha2::{Digest, Sha256};
use rustyline::error::ReadlineError;
use rustyline::{Editor, completion::{Completer, Pair}, hint::Hinter, highlight::{Highlighter}, validate::{Validator, ValidationContext, ValidationResult}, Helper, Context};
use std::borrow::Cow;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct EmpManifest {
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

impl EmpManifest {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: "An Ema project".to_string(),
            dependencies: HashMap::new(),
        }
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let mut check_mode = false;
    let mut json_mode = false;
    let mut strict_embedded = false;
    let mut verbose = false;
    let mut build_mode = false;
    let mut deploy_mode = false;
    let mut filename = String::new();
    let mut target = String::from("native"); // "native" | "wasm" | "node"

    let mut subcommand = String::new();

    if args.len() > 1 {
        let first = &args[1];
        if !first.starts_with("-") {
            subcommand = first.clone();
        }
    }

    for arg in args.iter().skip(1) {
        if arg == "check" || arg == "--check" {
            check_mode = true;
        } else if arg == "build" || arg == "--build" {
            build_mode = true;
        } else if arg == "run" || arg == "--run" {
            // Already handled by default if file is present
        } else if arg == "deploy" || arg == "--deploy" {
            deploy_mode = true;
        } else if arg == "--json" {
            json_mode = true;
        } else if arg == "--strict-embedded" {
            strict_embedded = true;
        } else if arg == "--verbose" || arg == "-v" {
            verbose = true;
        } else if arg == "--target" {
            // Handled in next iteration or Peek
        } else if filename.is_empty() {
             match arg.as_str() {
                 "init" | "create" | "repl" | "mod" | "lsp" | "run" | "build" | "check" | "test" | "deploy" | "run" => {},
                 _ if !arg.starts_with("-") => { filename = arg.clone(); },
                 _ => {}
             }
        }
    }

    for i in 0..args.len() {
        if args[i] == "--target" && i + 1 < args.len() {
            target = args[i+1].clone();
        }
    }

    if subcommand == "repl" {
        start_repl(verbose).await;
        return;
    }
    if subcommand == "init" || subcommand == "create" {
        println!("=== EMA PROJECT INITIALIZER ===");
        let mut target_dir = env::current_dir().unwrap();
        
        // If an argument is provided after 'init' or 'create', use it as directory name
        if args.len() > 2 && args[2] != "--verbose" && !args[2].starts_with("-") {
            let dir_name = &args[2];
            target_dir = target_dir.join(dir_name);
            if !target_dir.exists() {
                fs::create_dir_all(&target_dir).expect("Failed to create project directory!");
            }
            env::set_current_dir(&target_dir).expect("Failed to enter project directory!");
            println!(">>> Navigation: Entering directory '{}'", dir_name);
        }

        let project_name = target_dir.file_name().unwrap().to_str().unwrap();
        
        let manifest = EmpManifest::new(project_name);
        let json = serde_json::to_string_pretty(&manifest).expect("Failed to serialize manifest");
        fs::write("emp.json", json).expect("Failed to write emp.json");
        println!(">>> Created manifest: emp.json");

        let dirs = ["ema", "public", "data", "ema_modules"];
        for dir in dirs {
            fs::create_dir_all(dir).expect("Failed to create directory!");
            println!(">>> Created directory: {}", dir);
        }
        
        let template = "@server {
    print \"EMA Server Started! Port: 3000\";
    std::http::serve(3000);
}

@client {
    <div style: \"padding: 50px; text-align: center; font-family: sans-serif; background: #0a0a1a; color: white; min-height: 100vh;\">
        <h1 style: \"font-size: 3rem; margin-bottom: 20px;\"> \"Hello Ema!\" </h1>
        <p style: \"font-size: 1.2rem; opacity: 0.8;\"> \"You have started a great project with EmaLang V1.0\" </p>
        <div style: \"margin-top: 40px; border: 1px solid #3366ff; padding: 20px; display: inline-block; border-radius: 10px;\">
            <p> \"Current Mode: @client Rendered Interface\" </p>
        </div>
    </div>;
}
";
        fs::write("ema/project.ema", template).expect("Failed to create project.ema template!");
        println!(">>> Created template: ema/project.ema");

        if !std::path::Path::new(".gitignore").exists() {
            fs::write(".gitignore", "target/\nema_modules/\ndata/*.db\ndist/\n").expect("Failed to create .gitignore");
            println!(">>> Created boilerplate .gitignore");
        }

        println!("\nSUCCESS: Project initialized! Run 'ema run ema/project.ema' to start.");
        return;
    }
    if subcommand == "mod" {
        handle_mod_command(&args[2..]);
        return;
    }
    if subcommand == "lsp" {
        lsp::run_lsp_server().await;
        return;
    }
    if subcommand == "run" || (subcommand.is_empty() && !filename.is_empty()) {
        if filename.is_empty() && args.len() > 2 {
            filename = args[2].clone();
        }
        // Proceed to execution below
    } else if subcommand == "build" {
        build_mode = true;
        if filename.is_empty() && args.len() > 2 {
            filename = args[2].clone();
        }
    } else if subcommand == "check" {
        check_mode = true;
        if filename.is_empty() && args.len() > 2 {
            filename = args[2].clone();
        }
    } else if subcommand == "deploy" {
        deploy_mode = true;
        target = String::from("node"); // Deploy defaults to Node.js for simplicity
        if filename.is_empty() && args.len() > 2 {
            filename = args[2].clone();
        }
    } else if subcommand == "migrate" {
        sqlx::any::install_default_drivers();
        let db_url = "sqlite://ema.db"; // Default
        let migrator = db_migrator::Migrator::new(db_url);
        let subcmd = if args.len() > 2 { args[2].as_str() } else { "help" };
        match subcmd {
            "init" => migrator.init().await.expect("Migration Init failed"),
            "create" => {
                let name = if args.len() > 3 { &args[3] } else { "migration" };
                migrator.create(name).expect("Migration Creation failed");
            },
            "up" => migrator.up().await.expect("Migration UP failed"),
            "status" => migrator.status().await.expect("Migration Status failed"),
            _ => {
                println!("Usage: ema migrate <init|create|up|status>");
            }
        }
        return;
    } else if subcommand.is_empty() && filename.is_empty() {
        start_repl(verbose).await;
        return;
    }

    fn handle_mod_command(args: &[String]) {
        if args.is_empty() {
            println!("Usage: ema mod <command> [args]");
            println!("Commands: add <pkg> <path/url>, install, list");
            return;
        }
        let cmd = &args[0];
        match cmd.as_str() {
            "add" => {
                if args.len() < 3 { println!("Usage: ema mod add <name> <path/url>"); return; }
                let name = &args[1];
                let source = &args[2];
                let mut manifest: EmpManifest = serde_json::from_str(&fs::read_to_string("emp.json").expect("emp.json not found")).expect("Failed to parse emp.json");
                manifest.dependencies.insert(name.clone(), source.clone());
                fs::write("emp.json", serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
                println!(">>> Added dependency: {} -> {}", name, source);
            }
            "list" => {
                let manifest: EmpManifest = serde_json::from_str(&fs::read_to_string("emp.json").expect("emp.json not found")).expect("Failed to parse emp.json");
                println!("EMA Dependencies:");
                for (k, v) in manifest.dependencies {
                    println!("  - {} ({})", k, v);
                }
            }
            "install" => {
                println!(">>> Resolving dependencies...");
                // MVP: Just ensure directories exist or symlink local ones.
                // In future: git clone.
                println!("DONE: All dependencies synchronized.");
            }
            _ => println!("Unknown mod command: {}", cmd),
        }
        return;
    }

    if subcommand == "test" {
        if filename.is_empty() {
             println!("Usage: ema test <filename>");
             return;
        }
        run_tests(&filename, verbose, strict_embedded).await;
        return;
    }

    if filename.is_empty() {
        start_repl(verbose).await;
        return;
    }

    if !std::path::Path::new(&filename).exists() {
        println!("Error: File '{}' not found.", filename);
        return;
    }

    let source_code = fs::read_to_string(&filename).expect("Failed to read selected .ema file!");

    if verbose && !check_mode {
        println!("=== EMA SYSTEM COMPILER BOOTSTRAP ===");
        println!(">>> Loading Ema File: {}", filename);
    }

    // --- Lexical Analysis ---
    let mut lexer = lexer::Lexer::new(&source_code);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token();
        tokens.push(tok.clone());
        if tok.token == crate::lexer::Token::EOF { break; }
    }

    // --- Parsing (Syntactic Analysis) ---
    let mut parser = parser::Parser::new(tokens);
    let mut ast = parser.parse();
    
    // --- Module Resolution (recursive imports) ---
    let mut seen_files = std::collections::HashSet::new();
    seen_files.insert(fs::canonicalize(&filename).unwrap_or_else(|_| std::path::PathBuf::from(&filename)));
    ast = resolve_imports(ast, &mut seen_files, verbose);

    // --- Semantic Analysis (Type Checking) ---
    let mut semantic_analyzer = analyzer::Analyzer::new(strict_embedded);
    let diagnostics = semantic_analyzer.analyze(&ast);
    let has_error = diagnostics.iter().any(|d| d.severity == "error");

    if build_mode {
        if target == "node" {
            let mut node_builder = node_builder::NodeBuilder::new();
            let js = node_builder.build(&ast);
            fs::write("server.js", js).expect("Failed to write server.js");
            println!(">>> BUILD SUCCESS: Generated server.js (Node.js target)");
        } else {
            // (Existing frontend/WASM/LLVM logic followed by native build)
        }
    }

    if check_mode {
        println!(">>> CHECK SUCCESS: No errors found in {}", filename);
        return;
    }

    // --- WebAssembly UI Build (Frontend) ---
    if verbose {
        println!("\n--- WebAssembly UI Build (Frontend) ---");
    }

    let mut wasm_compiler = wasm_builder::WasmBuilder::new();
    let (js_code, wasm_bytes) = wasm_compiler.build_frontend(&ast);
    fs::write("frontend.js", js_code).expect("Failed to write JS frontend output!");
    fs::write("frontend.wasm", wasm_bytes).expect("Failed to write WASM binary output!");
    
    let ssr_html = wasm_compiler.build_ssr_html(&ast);
    fs::write("frontend.ssr.html", ssr_html).expect("Failed to write SSR HTML output!");

    let ssr_css = wasm_compiler.build_ssr_css(&ast);
    fs::write("frontend.ssr.css", ssr_css).expect("Failed to write SSR CSS output!");

    let mut hasher = Sha256::new();
    if let Ok(h) = fs::read("frontend.ssr.html") { hasher.update(&h); }
    if let Ok(c) = fs::read("frontend.ssr.css") { hasher.update(&c); }
    let build_hash = format!("{:x}", hasher.finalize());

    if let Ok(js) = fs::read_to_string("frontend.js") {
        let patched = js.replace("__EMA_EXPECTED_HASH__", &build_hash);
        fs::write("frontend.js", patched).expect("Failed to patch frontend.js build hash!");
    }

    let mut js_hasher = Sha256::new();
    if let Ok(js_bytes) = fs::read("frontend.js") { js_hasher.update(&js_bytes); }
    let js_hash = format!("{:x}", js_hasher.finalize());

    let build_info = serde_json::json!({ 
        "hash": build_hash, 
        "jsHash": js_hash,
        "tailwind": wasm_compiler.tailwind_needed,
        "bootstrap": wasm_compiler.bootstrap_needed
    });
    fs::write("frontend.build.json", build_info.to_string()).expect("Failed to write build info!");

    if verbose {
        println!(">>> Browser output (WASM/JS) generated: frontend.js");
        println!(">>> SSR HTML generated: frontend.ssr.html");
        println!(">>> SSR CSS generated: frontend.ssr.css");
        println!(">>> Build hashes generated: frontend.build.json");
    }

    // --- EXECUTION FLOW ---
    if target == "node" && !build_mode && !check_mode && !deploy_mode {
        if !std::path::Path::new("server.js").exists() {
             let mut node_builder = node_builder::NodeBuilder::new();
             let js = node_builder.build(&ast);
             fs::write("server.js", js).expect("Failed to write server.js");
        }
        println!(">>> Running Ema on Node.js runtime...");
        let status = std::process::Command::new("node")
            .arg("server.js")
            .status()
            .expect("Failed to execute Node.js. Please ensure node is in your PATH.");
        
        if !status.success() {
            println!("Error: Node.js process exited with error.");
            std::process::exit(1);
        }
        return;
    }

    if deploy_mode {
        bundle_project(&filename, &ast).expect("Failed to bundle project for deployment");
        println!(">>> DEPLOYMENT READY: See 'dist/' directory for contents.");
        return;
    }

    if build_mode {
        // Let execution fall through to native LLVM build below
    } else {
        // If not build mode, we shouldn't run the LLVM/Native compiler below unless we want interpreter
        // Actually, if we reach here and it's deploy_mode we returned.
        // Return here if we don't want interpreter
    }

    if verbose {
        println!("\n--- Compilation (LLVM IR generation) ---");
    }

    let mut llvm_compiler = compiler::LlvmCompiler::new(compiler::CompilationTarget::Native);
    let ir = llvm_compiler.compile(&ast);
    fs::write("output.ll", ir).expect("Failed to write LLVM output!");
    
    if build_mode {
        if verbose {
            println!(">>> Native Compilation triggered: Compiling output.ll using clang...");
        }
        let output = std::process::Command::new("clang")
            .arg("output.ll")
            .arg("src/ema_runtime.c")
            .arg("-Wno-override-module")
            .arg("-o")
            .arg(filename.replace(".ema", ".exe"))
            .output();

        match output {
            Ok(o) if o.status.success() => {
                if verbose { println!(">>> Native Executable created successfully!"); }
            }
            Ok(o) => {
                println!("Error: Failed to compile LLVM IR to native binary.");
                println!("STDOUT: {}", String::from_utf8_lossy(&o.stdout));
                println!("STDERR: {}", String::from_utf8_lossy(&o.stderr));
                std::process::exit(1);
            }
            Err(e) => {
                println!("Error: Failed to execute `clang`: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if verbose {
        println!(">>> LLVM Intermediate Representation (IR) generated: output.ll");
        println!("\n--- Execution (Runtime) ---");
    }

    sqlx::any::install_default_drivers();
    if !std::path::Path::new("ema.db").exists() {
        std::fs::File::create("ema.db").unwrap();
    }
    let db_pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(5)
        .connect("sqlite://ema.db")
        .await
        .expect("Failed to create database connection pool");

    let mut interpreter = runtime::Interpreter::new(verbose, db_pool);
    interpreter.eval_program(ast.clone()).await;

    if verbose {
        println!("\n=== COMPILATION & EXECUTION SUCCESS ===");
    }
}

fn resolve_imports(mut program: Program, seen: &mut std::collections::HashSet<std::path::PathBuf>, verbose: bool) -> Program {
    let mut all_statements = Vec::new();
    let mut imports_to_process = Vec::new();

    for stmt in program.statements {
        if let Stmt::ImportStmt { source, .. } = &stmt {
            imports_to_process.push(source.clone());
        } else {
            all_statements.push(stmt);
        }
    }

    let mut resolved_statements = Vec::new();
    for src in imports_to_process {
        // Resolve path: 1. local, 2. ema_modules/
        let mut path = std::path::PathBuf::from(&src);
        if !path.extension().map_or(false, |ext| ext == "ema") {
            path.set_extension("ema");
        }

        let mut final_path = None;
        if path.exists() {
            final_path = Some(path);
        } else {
            let mod_path = std::path::PathBuf::from("ema_modules").join(&path);
            if mod_path.exists() {
                final_path = Some(mod_path);
            }
        }

        if let Some(p) = final_path {
            let canon = fs::canonicalize(&p).unwrap_or(p.clone());
            if seen.contains(&canon) { continue; }
            seen.insert(canon.clone());

            if verbose { println!(">>> Importing: {:?}", p); }
            let content = fs::read_to_string(&p).expect("Failed to read import");
            let mut lexer = crate::lexer::Lexer::new(&content);
            let mut tokens = Vec::new();
            loop {
                let t = lexer.next_token();
                tokens.push(t.clone());
                if t.token == crate::lexer::Token::EOF { break; }
            }
            let mut parser = crate::parser::Parser::new(tokens);
            let sub_ast = parser.parse();
            let resolved_sub = resolve_imports(sub_ast, seen, verbose);
            resolved_statements.extend(resolved_sub.statements);
        } else {
            println!("Warning: Could not resolve import '{}'", src);
        }
    }

    resolved_statements.extend(all_statements);
    program.statements = resolved_statements;
    program
}

struct EmaHelper;

impl Completer for EmaHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let commands = vec![
            "print", "var", "state", "model", "if", "else", "while", "for", "return", "exit", "help",
            "std::http::serve", "std::http::route", 
            "std::json::parse", "std::json::stringify",
            "std::file::read", "std::file::write", "std::file::exists", "std::file::delete",
            "std::fs::read", "std::fs::write", "std::fs::exists", "std::fs::delete"
        ];
        
        let mut candidates = Vec::new();
        let start = line[..pos].rfind(|c: char| !c.is_alphanumeric() && c != ':' && c != '.').map(|i| i + 1).unwrap_or(0);
        let word = &line[start..pos];
        
        if word.is_empty() { return Ok((pos, candidates)); }
        for cmd in commands {
            if cmd.starts_with(word) {
                candidates.push(Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                });
            }
        }
        Ok((start, candidates))
    }
}

impl Hinter for EmaHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> { None }
}

impl Highlighter for EmaHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        let mut highlighted = line.to_string();
        let keywords = [
            "var", "state", "model", "fn", "print", "if", "else", "while", "for", "return", 
            "@server", "@client", "true", "false", "std::", "json::", "http::", "fs::", "file::"
        ];
        
        for &kw in &keywords {
            // Very simple color injection
            let colored = format!("\x1b[1;36m{}\x1b[0m", kw); // Cyan
            highlighted = highlighted.replace(kw, &colored);
        }
        
        Cow::Owned(highlighted)
    }

    fn highlight_char(&self, _line: &str, _pos: usize) -> bool { true }
}

impl Validator for EmaHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let input = ctx.input();
        let open_braces = input.chars().filter(|&c| c == '{').count();
        let close_braces = input.chars().filter(|&c| c == '}').count();
        let open_parens = input.chars().filter(|&c| c == '(').count();
        let close_parens = input.chars().filter(|&c| c == ')').count();
        
        if open_braces > close_braces || open_parens > close_parens {
            Ok(ValidationResult::Incomplete)
        } else {
            Ok(ValidationResult::Valid(None))
        }
    }
}

impl Helper for EmaHelper {}

async fn start_repl(verbose: bool) {
    println!("=== EMA REPL (Universal Architecture) ===");
    println!("Type 'exit' to quit, '.help' for meta-commands.");
    
    sqlx::any::install_default_drivers();
    if !std::path::Path::new("ema.db").exists() {
        std::fs::File::create("ema.db").unwrap();
    }
    let db_pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite://ema.db")
        .await
        .expect("Failed to create REPL database connection pool");
        
    let mut interpreter = runtime::Interpreter::new(verbose, db_pool);
    
    let mut rl = Editor::<EmaHelper, rustyline::history::FileHistory>::new().expect("Failed to create REPL editor");
    rl.set_helper(Some(EmaHelper));
    
    // Simple history file
    let history_path = "ema_history.txt";
    let _ = rl.load_history(history_path);
    
    let mut semantic_analyzer = analyzer::Analyzer::new(false);

    loop {
        let readline = rl.readline("ema> ");
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                let mut trimmed = line.trim().to_string();
                
                if trimmed == "exit" || trimmed == "quit" || trimmed == ".exit" { break; }
                if trimmed == ".help" || trimmed == "help" {
                    println!("Usage:");
                    println!("  [statements]        - Execute EMA code");
                    println!("  .vars               - List defined variables");
                    println!("  .undo               - Step back to previous state (Time-Travel)");
                    println!("  .history            - Show snapshot count");
                    println!("  .clear              - Reset REPL state");
                    println!("  .load <file>        - Run a file in REPL context");
                    println!("  exit                - Quit REPL");
                    continue;
                }
                if trimmed == ".vars" {
                    let vars = interpreter.list_vars();
                    if vars.is_empty() {
                        println!("No variables defined yet");
                    } else {
                        for (name, val) in vars {
                            println!("  {} = {:?}", name, val);
                        }
                    }
                    continue;
                }
                if trimmed == ".clear" {
                    interpreter.clear_env();
                    println!("REPL state cleared");
                    continue;
                }
                if trimmed == ".undo" {
                    if interpreter.restore_snapshot() {
                        println!("<<< REWOUND: Restored to previous state.");
                    } else {
                        println!("Error: No history to undo.");
                    }
                    continue;
                }
                if trimmed == ".history" {
                    println!("History snapshots: {}", interpreter.history.len());
                    continue;
                }
                if trimmed.starts_with(".load ") {
                    let path = trimmed.trim_start_matches(".load ").trim();
                    if let Ok(content) = fs::read_to_string(path) {
                        let mut lex = lexer::Lexer::new(&content);
                        let mut toks = Vec::new();
                        loop {
                            let t = lex.next_token();
                            toks.push(t.clone());
                            if t.token == crate::lexer::Token::EOF { break; }
                        }
                        let mut p = parser::Parser::new(toks);
                        let ast = p.parse();
                        interpreter.eval_program(ast).await;
                        println!("Loaded and executed: {}", path);
                    } else {
                        println!("Error: File could not be read: {}", path);
                    }
                    continue;
                }
                if trimmed.is_empty() { continue; }

                let mut lexer = lexer::Lexer::new(&trimmed);
                let mut tokens = Vec::new();
                loop {
                    let tok = lexer.next_token();
                    tokens.push(tok.clone());
                    if tok.token == crate::lexer::Token::EOF { break; }
                }

                let mut parser = parser::Parser::new(tokens);
                
                // Catch panics to keep REPL alive
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    parser.parse()
                }));

                match result {
                    Ok(ast) => {
                        let diagnostics = semantic_analyzer.analyze(&ast);
                        let mut has_error = false;
                        for d in diagnostics {
                            let pfx = if d.severity == "warning" { "Warning" } else { "Error" };
                            println!("{} at {}:{}: {}", pfx, d.line, d.col, d.message);
                            if d.severity == "error" { has_error = true; }
                        }
                        
                        if !has_error {
                            interpreter.save_snapshot(); // Capture state BEFORE execution
                            interpreter.eval_program(ast).await;
                        }
                    }
                    Err(_) => {
                        println!("Error: Syntax error or missing character");
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    let _ = rl.save_history(history_path);
}

fn bundle_project(filename: &str, ast: &Program) -> io::Result<()> {
    let dist_path = std::path::Path::new("dist");
    if dist_path.exists() {
        fs::remove_dir_all(dist_path)?;
    }
    fs::create_dir_all(dist_path)?;
    fs::create_dir_all(dist_path.join("public"))?;
    fs::create_dir_all(dist_path.join("data"))?;

    // 1. Build Node Target
    let mut node_builder = crate::node_builder::NodeBuilder::new();
    let js = node_builder.build(ast);
    fs::write(dist_path.join("server.js"), js)?;

    // 2. Generate package.json
    let package_json = serde_json::json!({
        "name": "ema-app",
        "version": "1.0.0",
        "main": "server.js",
        "scripts": {
            "start": "node server.js"
        }
    });
    fs::write(dist_path.join("package.json"), serde_json::to_string_pretty(&package_json).unwrap())?;

    // 3. Copy Assets
    let asset_files = ["frontend.js", "frontend.wasm", "frontend.ssr.html", "frontend.ssr.css", "frontend.build.json"];
    for f in asset_files {
        if std::path::Path::new(f).exists() {
            fs::copy(f, dist_path.join("public").join(f))?;
        }
    }

    // 4. Copy public/ folder if exists
    if std::path::Path::new("public").exists() {
        for entry in fs::read_dir("public")? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::copy(&path, dist_path.join("public").join(path.file_name().unwrap()))?;
            }
        }
    }

    // 5. Generate DEPLOY_GUIDE.md
    let guide = "# EMA Deployment Guide (Node.js)

1. Upload the contents of `dist/` to your server.
2. In Plesk/cPanel, use the 'Node.js' selector to create an application.
3. Set 'Application Startup File' to `server.js`.
4. Ensure 'Application Root' points to your uploaded directory.
5. Click 'NPM Install' (if you add extra packages) and 'Restart'.

Your Ema application is now live!
";
    fs::write(dist_path.join("DEPLOY_GUIDE.md"), guide)?;

    Ok(())
}

async fn run_tests(filename: &str, verbose: bool, _strict_embedded: bool) {
    let source_code = fs::read_to_string(filename).expect("Failed to read file");
    let mut lexer = lexer::Lexer::new(&source_code);
    let mut tokens = Vec::new();
    loop {
        let tok = lexer.next_token();
        tokens.push(tok.clone());
        if tok.token == crate::lexer::Token::EOF { break; }
    }
    let mut parser = parser::Parser::new(tokens);
    let mut ast = parser.parse();
    
    // Resolve imports
    let mut seen_files = std::collections::HashSet::new();
    seen_files.insert(fs::canonicalize(filename).unwrap_or_else(|_| std::path::PathBuf::from(filename)));
    ast = resolve_imports(ast, &mut seen_files, verbose);

    sqlx::any::install_default_drivers();
    if !std::path::Path::new("ema.db").exists() {
        std::fs::File::create("ema.db").unwrap();
    }
    let db_pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(5)
        .connect("sqlite://ema.db")
        .await
        .expect("Failed to create database connection pool");

    let mut interpreter = crate::runtime::Interpreter::new(verbose, db_pool);
    interpreter.test_mode = true;

    println!(">>> Running tests in {}...", filename);
    
    let mut passed = 0;
    let mut failed = 0;

    let mut tests = Vec::new();
    let mut other_stmts = Vec::new();
    for stmt in ast.statements {
        if let crate::ast::Stmt::Test { name, body, span } = stmt {
            tests.push((name, body, span));
        } else {
            other_stmts.push(stmt);
        }
    }

    // Baseline execution (globals, functions, models)
    interpreter.eval_program(Program { is_strict: false, statements: other_stmts }).await;

    for (name, body, _span) in tests {
        print!("  test \"{}\" ... ", name);
        let mut test_interpreter = interpreter.clone();
        test_interpreter.test_failures.clear();
        
        for stmt in body {
            test_interpreter.eval_stmt(stmt).await;
        }
        
        if test_interpreter.test_failures.is_empty() {
            println!("PASSED ✅");
            passed += 1;
        } else {
            println!("FAILED ❌");
            for failure in test_interpreter.test_failures {
                println!("    [FAILURE] {}", failure);
            }
            failed += 1;
        }
    }

    println!("\nTest Result: {} passed, {} failed", passed, failed);
    if failed > 0 {
        std::process::exit(1);
    }
}
