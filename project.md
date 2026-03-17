### ROLE:
You are an expert Rust systems engineer and language designer. Your task is to create a universal programming environment called "Ema" that can parse, interpret, and optionally compile HTML, CSS, JavaScript, and PHP code into a single unified runtime using Rust.

### OBJECTIVE:
Build a Rust project skeleton with full lexer, parser, AST, semantic analysis, and interpreter for Ema. The runtime must support:
- HTML element parsing and basic DOM tree.
- CSS parsing and application to DOM elements.
- JavaScript parsing and execution (variables, functions, expressions).
- PHP parsing and execution (variables, functions, basic I/O).
- A shared environment for variables, functions, and objects.
- Modular design to allow adding a WASM compiler in the future.

### REQUIREMENTS:
1. **Lexer**:
   - Tokenize HTML tags, CSS selectors and properties, JS statements, PHP statements.
   - Use `nom` or `pest` for Rust parsing.
   - Output tokens as enums for further AST processing.

2. **Parser**:
   - Convert token stream into an AST.
   - Support modular parsers for HTML, CSS, JS, and PHP.
   - Use recursive descent parsing for statements and expressions.

3. **AST Design**:
   - Use Rust enums and traits to represent nodes.
   - Example nodes: `HtmlElement`, `CssRule`, `JsStatement`, `PhpStatement`.
   - Include nodes for expressions, functions, variables, and DOM tree.

4. **Semantic Analysis**:
   - Handle variable scope and function definitions.
   - Implement basic type checking for JS/PHP dynamic types.
   - Include an `Environment` struct to store runtime variables and objects.

5. **Interpreter / Runtime**:
   - Traverse AST nodes and execute corresponding code.
   - HTML/CSS nodes: build a basic DOM representation.
   - JS/PHP nodes: evaluate expressions and execute functions.
   - Support dynamic objects, arrays, and simple type system (Int, Float, String, Bool, Null, Object).

6. **Modularity**:
   - Separate modules: `lexer`, `parser`, `ast`, `runtime`, `main`.
   - Easy to extend for WASM compilation later.
   - Include tests for each module.

7. **Extras**:
   - Include example code showing HTML + CSS + JS + PHP together.
   - Provide comments explaining each Rust struct, enum, and function.
   - Ensure the code compiles and runs with `cargo run`.

### FINAL OUTPUT:
Provide a **full Rust project skeleton** with:
- Cargo.toml
- `src/main.rs`
- Modules: `lexer.rs`, `parser.rs`, `ast.rs`, `runtime.rs`
- Minimal working example of Ema interpreting a combined HTML+CSS+JS+PHP snippet.
- Include detailed comments on each part, explaining how it can be expanded.