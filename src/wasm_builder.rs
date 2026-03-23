use crate::ast::{CssStylesheet, Expr, HtmlAttrValue, HtmlNode, JsExpr, JsParam, JsPattern, JsProgram, JsStmt, JsTemplatePart, JsVarKind, Program, Stmt, EmbeddedKind, BinaryOp};
use wasm_encoder::{
    CodeSection, ExportSection, Function, FunctionSection, ImportSection, Instruction,
    Module, TypeSection, ValType, MemorySection, MemoryType,
};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub struct WasmBuilder {
    js_output: String,
    client_scripts: Vec<String>,
    state_vars: HashSet<String>,
    state_inits: Vec<String>,
    wasm_module: Module,
    components: std::collections::HashMap<String, (Vec<String>, Vec<Stmt>)>,
    pub tailwind_needed: bool,
    pub bootstrap_needed: bool,
}

impl WasmBuilder {
    pub fn new() -> Self {
        let mut js = String::new();
        // Base API injections for the UI bindings and Emscripten style loaders
        js.push_str("/**\n");
        js.push_str(" * EMA Universal Ecosystem - Auto-Generated WASM / JS Wrapper\n");
        js.push_str(" * Build Target: Web Browser (Frontend)\n");
        js.push_str(" */\n\n");

        js.push_str("const EmaApp = {\n");
        js.push_str("  state: {},\n");
        js.push_str("  bindings: [],\n");
        js.push_str("  computed: {},\n");
        js.push_str("  hydrateBindingsFromDom: function(root) {\n");
        js.push_str("    this.bindings = [];\n");
        js.push_str("    this.computed = this.computed || {};\n");
        js.push_str("    if (!root) return;\n");
        js.push_str("    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, null, false);\n");
        js.push_str("    let node;\n");
        js.push_str("    while(node = walker.nextNode()) {\n");
        js.push_str("      if (node.nodeType === Node.ELEMENT_NODE) {\n");
        js.push_str("        const key = node.getAttribute('data-ema-bind');\n");
        js.push_str("        if (key) {\n");
        js.push_str("          const expr = node.getAttribute('data-ema-expr');\n");
        js.push_str("          if (expr && !this.computed[key]) {\n");
        js.push_str("             try { this.computed[key] = new Function('state', 'return (' + expr + ')'); } catch (e) {}\n");
        js.push_str("          }\n");
        js.push_str("          this.bindings.push({ key, node: node, prev: undefined });\n");
        js.push_str("        }\n");
        js.push_str("      } else if (node.nodeType === Node.TEXT_NODE && node.parentElement && node.parentElement.hasAttribute('data-ema-bind')) {\n");
        js.push_str("         const key = node.parentElement.getAttribute('data-ema-bind');\n");
        js.push_str("         if (key) this.bindings.push({ key, node: node, prev: undefined });\n");
        js.push_str("      }\n");
        js.push_str("    }\n");
        js.push_str("  },\n");
        js.push_str("  wireDomEvents: function(root) {\n");
        js.push_str("    if (!root || root._ema_wired) return;\n");
        js.push_str("    root._ema_wired = true;\n");
        js.push_str("    root.addEventListener('click', (ev) => {\n");
        js.push_str("      const el = ev.target.closest('[data-ema-click]');\n");
        js.push_str("      if (!el) return;\n");
        js.push_str("      const spec = el.getAttribute('data-ema-click');\n");
        js.push_str("      if (spec) this.executeAction(spec, el, ev);\n");
        js.push_str("    });\n");
        js.push_str("    ['input', 'change'].forEach(evt => {\n");
        js.push_str("      const attr = 'data-ema-on' + evt;\n");
        js.push_str("      root.addEventListener(evt, (ev) => {\n");
        js.push_str("        const el = ev.target.closest('[' + attr + ']');\n");
        js.push_str("        if (el) this.executeAction(el.getAttribute(attr), el, ev);\n");
        js.push_str("      });\n");
        js.push_str("    });\n");
        js.push_str("  },\n");
        js.push_str("  executeAction: function(spec, el, ev) {\n");
        js.push_str("    const mInc = spec.match(/^\\s*inc\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*\\)\\s*$/);\n");
        js.push_str("    const mDec = spec.match(/^\\s*dec\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*\\)\\s*$/);\n");
        js.push_str("    const mTog = spec.match(/^\\s*toggle\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*\\)\\s*$/);\n");
        js.push_str("    const mSetVal = spec.match(/^\\s*set\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*,\\s*value\\s*\\)\\s*$/);\n");
        js.push_str("    const mSet = spec.match(/^\\s*set\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*,\\s*(.+)\\s*\\)\\s*$/);\n");
        js.push_str("    if (mInc) { this.setState(mInc[1], (this.state[mInc[1]]??0)+1); }\n");
        js.push_str("    else if (mDec) { this.setState(mDec[1], (this.state[mDec[1]]??0)-1); }\n");
        js.push_str("    else if (mTog) { this.setState(mTog[1], !(this.state[mTog[1]]??false)); }\n");
        js.push_str("    else if (mSetVal) { this.setState(mSetVal[1], String(el.value || '')); }\n");
        js.push_str("    else if (mSet) {\n");
        js.push_str("      const k = mSet[1]; let raw = String(mSet[2]).trim(); let v = raw;\n");
        js.push_str("      if (/^\\d+(\\.\\d+)?$/.test(raw)) v = Number(raw);\n");
        js.push_str("      else if (raw === 'true') v = true; else if (raw === 'false') v = false;\n");
        js.push_str("      else if ((raw.startsWith('\"') && raw.endsWith('\"')) || (raw.startsWith(\"'\") && raw.endsWith(\"'\"))) v = raw.slice(1, -1);\n");
        js.push_str("      this.setState(k, v);\n");
        js.push_str("    } else {\n");
        js.push_str("      try { (new Function('state', 'el', 'ev', spec)).call(el, this.state, el, ev); } catch(e) { console.error(e); }\n");
        js.push_str("    }\n");
        js.push_str("  },\n");
        js.push_str("  setState: function(key, value) {\n");
        js.push_str("    this.state[key] = value;\n");
        js.push_str("    // MVP: if we have bindings, update text without full rerender.\n");
        js.push_str("    if (this.bindings && this.bindings.length > 0) {\n");
        js.push_str("      this.updateBindings();\n");
        js.push_str("      this.runClientScripts();\n");
        js.push_str("      return;\n");
        js.push_str("    }\n");
        js.push_str("    this.render();\n");
        js.push_str("  },\n");
        js.push_str("  hydrate: function(v) {\n");
        js.push_str("    console.log('[EMA Hydration] Hydrating state...', v);\n");
        js.push_str("    Object.assign(this.state, v);\n");
        js.push_str("    if (this.bindings && this.bindings.length > 0) {\n");
        js.push_str("      this.updateBindings();\n");
        js.push_str("      this.runClientScripts();\n");
        js.push_str("      return;\n");
        js.push_str("    }\n");
        js.push_str("    this.render();\n");
        js.push_str("  },\n");
        js.push_str("  init: function() {\n");
        js.push_str("    console.log('[EMA WASM Engine] Loading UI components...');\n");
        js.push_str("    const root = document.getElementById('ema-root') || document.body;\n");
        js.push_str("    this.initState();\n");
        js.push_str("    if (window.__EMA_HYDRATION__) Object.assign(this.state, window.__EMA_HYDRATION__);\n");
        js.push_str("    const expected = (this.build && this.build.hash) ? String(this.build.hash) : '';\n");
        js.push_str("    const actual = (window.__EMA_BUILD__ && window.__EMA_BUILD__.hash) ? String(window.__EMA_BUILD__.hash) : '';\n");
        js.push_str("    const canHydrate = expected && actual && expected === actual;\n");
        js.push_str("    if (!canHydrate && actual) console.warn('[EMA] SSR build hash mismatch; client render fallback', { expected, actual });\n");
        // Inject base layout styles
        js.push_str("  const baseStyle = document.createElement('style');\n");
        js.push_str("  baseStyle.textContent = `\n");
        js.push_str("    :root {\n");
        js.push_str("      --ema-primary: #3498db;\n");
        js.push_str("      --ema-bg: #ffffff;\n");
        js.push_str("      --ema-text: #2c3e50;\n");
        js.push_str("      --ema-transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);\n");
        js.push_str("    }\n");
        js.push_str("    *, *::before, *::after { box-sizing: border-box; }\n");
        js.push_str("    html, body { margin: 0; padding: 0; font-family: 'Inter', system-ui, sans-serif; color: var(--ema-text); background: var(--ema-bg); }\n");
        js.push_str("    #ema-root { isolation: isolate; }\n");
        js.push_str("    .ema-fade { transition: opacity 0.3s ease; }\n");
        js.push_str("    .ema-slide { transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1); }\n");
        js.push_str("    .ema-scale { transition: transform 0.2s cubic-bezier(0.175, 0.885, 0.32, 1.275); }\n");
        js.push_str("    .ema-scale:active { transform: scale(0.95); }\n");
        js.push_str("    [data-ema-css-var] { transition: var(--ema-transition); }\n");
        js.push_str("  `;\n");
        js.push_str("  document.head.appendChild(baseStyle);\n");
        
        js.push_str("  if (canHydrate && root && root.childNodes && root.childNodes.length > 0) {\n");
        js.push_str("      this.hydrateBindingsFromDom(root);\n");
        js.push_str("      this.wireDomEvents(root);\n");
        js.push_str("      this.updateBindings();\n");
        js.push_str("      this.runClientScripts();\n");
        js.push_str("      return;\n");
        js.push_str("    }\n");
        js.push_str("    this.render();\n");
        js.push_str("  },\n");
        js.push_str("  render: function() {\n");
        js.push_str("    const root = document.getElementById('ema-root') || document.body;\n");
        js.push_str("    root.innerHTML = '';\n");
        js.push_str("    this.buildUI(root);\n");
        js.push_str("    // Collect bindings from DOM for HtmlAst/SSR-like spans.\n");
        js.push_str("    this.hydrateBindingsFromDom(root);\n");
        js.push_str("    this.wireDomEvents(root);\n");
        js.push_str("    this.updateBindings();\n");
        js.push_str("    this.runClientScripts();\n");
        js.push_str("  },\n");
        js.push_str("  buildUI: function(parent) {\n");

        WasmBuilder {
            js_output: js,
            client_scripts: Vec::new(),
            state_vars: HashSet::new(),
            state_inits: Vec::new(),
            wasm_module: Module::new(),
            components: std::collections::HashMap::new(),
            tailwind_needed: false,
            bootstrap_needed: false,
        }
    }

    pub fn build_frontend(&mut self, program: &Program) -> (String, Vec<u8>) {
        // 1. Prepare WASM logic
        let mut types = TypeSection::new();
        let mut imports = ImportSection::new();
        let mut functions = FunctionSection::new();
        let mut exports = ExportSection::new();
        let mut globals = wasm_encoder::GlobalSection::new();
        let mut code = CodeSection::new();
        let mut memory = MemorySection::new();

    // (import "env" "ema_dom_create" (func (param i32) (result i32)))
    types.ty().function(vec![ValType::I32], vec![ValType::I32]); // Type 0: i32 -> i32
    imports.import("env", "ema_dom_create", wasm_encoder::EntityType::Function(0));
    
    // Example logic: EMA Internal "Render" function
    types.ty().function(vec![], vec![]); // Type 1: void -> void
    functions.function(1);
    exports.export("ema_render", wasm_encoder::ExportKind::Func, 0);

    let mut f = Function::new(vec![]);
    f.instruction(&Instruction::End);
    code.function(&f);

    memory.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

        self.wasm_module.section(&types);
        self.wasm_module.section(&imports);
        self.wasm_module.section(&functions);
        self.wasm_module.section(&exports);
        self.wasm_module.section(&globals);
        self.wasm_module.section(&memory);
        self.wasm_module.section(&code);

        // 2. Prepare JS Wrapper
        let mut components_js = String::new();
        for stmt in &program.statements {
            if let Stmt::ComponentDecl { name, props, body, .. } = stmt {
                self.components.insert(name.clone(), (props.iter().map(|(n, _)| n.clone()).collect(), body.clone()));
                
                components_js.push_str(&format!("function {}(props) {{\n", name));
                components_js.push_str("  const parent = document.createDocumentFragment();\n");
                for (p_name, _) in props {
                    components_js.push_str(&format!("  const {} = props['{}'];\n", p_name, p_name));
                }
                
                let old_js = std::mem::take(&mut self.js_output);
                for s in body {
                    self.compile_js(s, 2);
                }
                components_js.push_str(&self.js_output);
                self.js_output = old_js;
                
                components_js.push_str("  return parent.childNodes.length === 1 ? parent.firstChild : parent;\n");
                components_js.push_str("}\n\n");
            }
        }
        
        if (!components_js.is_empty()) {
            self.js_output = self.js_output.replace("const EmaApp = {\n", &format!("{}const EmaApp = {{\n", components_js));
        }

        // Add WASM instantiation logic to EmaApp.init
        self.js_output = self.js_output.replace(
            "this.render();",
            "this.fetchAndInstantiateWasm().then(() => this.render());"
        );

        // Inject fetchAndInstantiateWasm method
        self.js_output = self.js_output.replace(
            "  init: function() {",
            "  fetchAndInstantiateWasm: async function() {\n    try {\n      const res = await fetch('frontend.wasm');\n      const { instance } = await WebAssembly.instantiateStreaming(res, {\n        env: {\n          ema_dom_create: (tagPtr) => { /* logic here */ return 0; }\n        }\n      });\n      this.wasm = instance.exports;\n      console.log('[EMA WASM] Binary Module Loaded');\n    } catch (e) { console.warn('[EMA WASM] Loading failed, binary logic disabled', e); }\n  },\n  init: function() {"
        );

        for stmt in &program.statements {
            if let Stmt::ClientBlock(client_stmts, _) = stmt {
                for c_stmt in client_stmts {
                    // Track state vars for identifier lowering to EmaApp.state
                    if let Stmt::StateDecl { name, .. } = c_stmt {
                        self.state_vars.insert(name.clone());
                    }
                    if let Stmt::StateDecl { name, value, .. } = c_stmt {
                        let val_str = self.compile_expr(value);
                        self.state_inits.push(format!(
                            "if (EmaApp.state[\"{k}\"] === undefined) EmaApp.state[\"{k}\"] = {v};",
                            k = Self::escape_js_string(name),
                            v = val_str
                        ));
                    }
                    // Extract embedded JS blocks into lifecycle scripts (avoid script-tag injection + enable per-render rebind)
                    if let Stmt::ExprStmt(Expr::EmbeddedBlock { kind: EmbeddedKind::Js, raw, span: _ }, _) = c_stmt {
                        self.client_scripts.push(raw.clone());
                        continue;
                    }
                    if let Stmt::ExprStmt(Expr::JsAst { program, .. }, _) = c_stmt {
                        self.client_scripts.push(Self::render_js_program(program));
                        continue;
                    }
                    self.compile_js(c_stmt, 4);
                }
            }
        }
        
        // Close buildUI method
        self.js_output.push_str("  },\n");

        // Add initState method (runs even when SSR hydration skips render)
        self.js_output.push_str("  initState: function() {\n");
        for line in &self.state_inits {
            self.js_output.push_str("    ");
            self.js_output.push_str(line);
            self.js_output.push('\n');
        }
        self.js_output.push_str("  },\n");

        // Add runClientScripts method
        self.js_output.push_str("  runClientScripts: function() {\n");
        self.js_output.push_str("    if (!this.clientScripts) return;\n");
        self.js_output.push_str("    for (const fn of this.clientScripts) {\n");
        self.js_output.push_str("      try { fn(this); } catch (e) { console.error('[EMA] client script error', e); }\n");
        self.js_output.push_str("    }\n");
        self.js_output.push_str("  },\n");

        // Add updateBindings method
        self.js_output.push_str("  updateBindings: function() {\n");
        self.js_output.push_str("    if (!this.bindings) return;\n");
        self.js_output.push_str("    for (const b of this.bindings) {\n");
        self.js_output.push_str("      try {\n");
        self.js_output.push_str("        let v = this.state[b.key];\n");
        self.js_output.push_str("        if (v === undefined && this.computed && this.computed[b.key]) {\n");
        self.js_output.push_str("          try { v = this.computed[b.key](this.state); } catch (e) { v = ''; }\n");
        self.js_output.push_str("        }\n");
        self.js_output.push_str("        const val = (v === undefined || v === null) ? '' : String(v);\n");
        self.js_output.push_str("        if (b.prev === val) continue;\n");
        self.js_output.push_str("        b.prev = val;\n");
        self.js_output.push_str("        if (b.node.nodeType === Node.TEXT_NODE) {\n");
        self.js_output.push_str("          b.node.textContent = val;\n");
        self.js_output.push_str("        } else if (b.node.nodeType === Node.ELEMENT_NODE) {\n");
        self.js_output.push_str("           if (b.node.dataset.emaCssVar) {\n");
        self.js_output.push_str("             b.node.style.setProperty(b.node.dataset.emaCssVar, val);\n");
        self.js_output.push_str("           } else {\n");
        self.js_output.push_str("             b.node.textContent = val;\n");
        self.js_output.push_str("           }\n");
        self.js_output.push_str("        }\n");
        self.js_output.push_str("      } catch (e) {}\n");
        self.js_output.push_str("    }\n");
        self.js_output.push_str("  },\n");

        // Attach extracted client scripts (run after each render)
        self.js_output.push_str("  clientScripts: [\n");
        for raw in &self.client_scripts {
            self.js_output.push_str("    function(EmaApp) {\n");
            self.js_output.push_str(raw);
            if !raw.ends_with('\n') {
                self.js_output.push('\n');
            }
            self.js_output.push_str("    },\n");
        }
        self.js_output.push_str("  ]\n");

        // Embed expected build hash for SSR mismatch detection (filled by main.rs).
        self.js_output.push_str(",\n  build: { hash: \"__EMA_EXPECTED_HASH__\" }\n");

        // Close EmaApp object
        self.js_output.push_str("};\n");
        self.js_output.push_str("\n// Bootstrap on load\ndocument.addEventListener('DOMContentLoaded', () => EmaApp.init());\n");
        
        (self.js_output.clone(), self.wasm_module.clone().finish())
    }

    fn compile_expr_wasm(&self, expr: &Expr, f: &mut Function) {
        match expr {
            Expr::IntLit(i, _) => {
                f.instruction(&Instruction::I32Const(*i as i32));
            }
            Expr::Binary {
                left,
                op,
                right,
                ..
            } => {
                self.compile_expr_wasm(left, f);
                self.compile_expr_wasm(right, f);
                match op {
                    BinaryOp::Add => {
                        f.instruction(&Instruction::I32Add);
                    }
                    BinaryOp::Sub => {
                        f.instruction(&Instruction::I32Sub);
                    }
                    BinaryOp::Mul => {
                        f.instruction(&Instruction::I32Mul);
                    }
                    BinaryOp::Div => {
                        f.instruction(&Instruction::I32DivS);
                    }
                    BinaryOp::EqEq => {
                        f.instruction(&Instruction::I32Eq);
                    }
                    BinaryOp::Less => {
                        f.instruction(&Instruction::I32LtS);
                    }
                    BinaryOp::Greater => {
                        f.instruction(&Instruction::I32GtS);
                    }
                    _ => {
                        f.instruction(&Instruction::I32Const(0));
                    }
                }
            }
            Expr::Identifier(name, _) => {
                // Simplified: treat as a global index if it's a known state var
                // In a full implementation, we'd have a name -> index map.
                f.instruction(&Instruction::GlobalGet(0)); 
            }
            Expr::Ternary { condition, then_expr, else_expr, .. } => {
                self.compile_expr_wasm(condition, f);
                f.instruction(&Instruction::If(wasm_encoder::BlockType::Result(ValType::I32)));
                self.compile_expr_wasm(then_expr, f);
                f.instruction(&Instruction::Else);
                self.compile_expr_wasm(else_expr, f);
                f.instruction(&Instruction::End);
            }
            _ => {
                f.instruction(&Instruction::I32Const(0));
            }
        }
    }

    pub fn build_ssr_html(&mut self, program: &Program) -> String {
        // MVP SSR: render UI AST nodes and/or concatenate embedded HTML blocks found in @client.
        // The runtime will inject this into #ema-root for initial paint.
        let mut out = String::new();
        for stmt in &program.statements {
            if let Stmt::ClientBlock(client_stmts, _) = stmt {
                for c_stmt in client_stmts {
                    if let Stmt::ExprStmt(expr, _) = c_stmt {
                        out.push_str(&self.render_ssr_expr(expr));
                        continue;
                    }
                    if let Stmt::ExprStmt(
                        Expr::EmbeddedBlock {
                            kind: EmbeddedKind::Html,
                            raw,
                            span: _,
                        },
                        _,
                    ) = c_stmt
                    {
                        out.push_str(raw);
                        if !raw.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                }
            }
        }
        out
    }

    pub fn build_ssr_css(&mut self, program: &Program) -> String {
        // SSR CSS bundle: combine global client css blocks and component-scoped css props.
        let mut out = String::new();
        out.push_str("/* EMA Premium Reset & Transitions */\n");
        out.push_str(":root {\n");
        out.push_str("  --ema-primary: #3498db;\n");
        out.push_str("  --ema-bg: #ffffff;\n");
        out.push_str("  --ema-text: #2c3e50;\n");
        out.push_str("  --ema-transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);\n");
        out.push_str("}\n");
        out.push_str("*, *::before, *::after { box-sizing: border-box; }\n");
        out.push_str("html, body { margin: 0; padding: 0; font-family: 'Inter', system-ui, sans-serif; color: var(--ema-text); background: var(--ema-bg); }\n");
        out.push_str("#ema-root { isolation: isolate; }\n\n");
        out.push_str(".ema-fade { transition: opacity 0.3s ease; }\n");
        out.push_str(".ema-slide { transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1); }\n");
        out.push_str(".ema-scale { transition: transform 0.2s cubic-bezier(0.175, 0.885, 0.32, 1.275); }\n");
        out.push_str(".ema-scale:active { transform: scale(0.95); }\n");
        out.push_str("[data-ema-css-var] { transition: var(--ema-transition); }\n\n");

        for stmt in &program.statements {
            if let Stmt::ClientBlock(client_stmts, _) = stmt {
                for c_stmt in client_stmts {
                    if let Stmt::ExprStmt(expr, _) = c_stmt {
                        self.collect_ssr_css_from_expr(expr, &mut out);
                    }
                }
            }
        }
        out
    }

    fn collect_ssr_css_from_expr(&mut self, expr: &Expr, out: &mut String) {
        match expr {
            Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, .. } => {
                // Global client css blocks are already scoped to #ema-root in JS builder; reuse same behavior for SSR.
                let scoped = Self::scope_css(raw, "#ema-root");
                out.push_str(&scoped);
                if !scoped.ends_with('\n') {
                    out.push('\n');
                }
            }
            Expr::CssAst { stylesheet, .. } => {
                let raw = self.render_css_stylesheet(stylesheet);
                let scoped = Self::scope_css(&raw, "#ema-root");
                out.push_str(&scoped);
                if !scoped.ends_with('\n') {
                    out.push('\n');
                }
            }
            Expr::UiElement { tag, props, children, .. } => {
                if let Some((_, body)) = self.components.get(tag) {
                    let body_cloned = body.clone();
                    for stmt in body_cloned {
                        if let Stmt::ExprStmt(e, _) = stmt {
                            self.collect_ssr_css_from_expr(&e, out);
                        }
                    }
                }

                if let Some((css_raw, class_name)) = props.get("css").and_then(|v| match v {
                    Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, .. } => {
                        Some((raw.clone(), Self::css_class_name(raw)))
                    }
                    Expr::CssAst { stylesheet, .. } => {
                        let raw = self.render_css_stylesheet(stylesheet);
                        Some((raw.clone(), Self::css_class_name(&raw)))
                    }
                    _ => None,
                }) {
                    let mut final_css = css_raw.trim().to_string();
                    if !final_css.contains('{') && !final_css.is_empty() {
                        final_css = format!("{{ {} }}", final_css);
                    }
                    let scoped = Self::scope_css(&final_css, &format!(".{}", class_name));
                    out.push_str(&scoped);
                    if !scoped.ends_with('\n') {
                        out.push('\n');
                    }
                }
                for c in children {
                    self.collect_ssr_css_from_expr(c, out);
                }
            }
            Expr::Ternary { then_expr, else_expr, .. } => {
                self.collect_ssr_css_from_expr(then_expr, out);
                self.collect_ssr_css_from_expr(else_expr, out);
            }
            _ => {}
        }
    }

    fn render_ssr_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::UiElement { tag, props, children, .. } => {
                if let Some((_, body)) = self.components.get(tag) {
                    let mut expanded = String::new();
                    let body_cloned = body.clone();
                    for stmt in body_cloned {
                        match stmt {
                            Stmt::ExprStmt(e, _) => expanded.push_str(&self.render_ssr_expr(&e)),
                            Stmt::ReturnStmt(Some(e), _) => expanded.push_str(&self.render_ssr_expr(&e)),
                            _ => {}
                        }
                    }
                    return expanded;
                }

                let mut out = String::new();
                out.push('<');
                out.push_str(tag);
                // Component-scoped CSS: add class deterministically for SSR too
                if let Some((_css_raw, class_name)) = props.get("css").and_then(|v| match v {
                    Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, .. } => {
                        Some((raw.clone(), Self::css_class_name(raw)))
                    }
                    Expr::CssAst { stylesheet, .. } => {
                        let raw = self.render_css_stylesheet(stylesheet);
                        Some((raw.clone(), Self::css_class_name(&raw)))
                    }
                    _ => None,
                }) {
                    // Merge with existing class attribute if present
                    let mut class_attr = class_name;
                    if let Some(Expr::StringLit(existing, _)) = props.get("class") {
                        class_attr = format!("{} {}", existing, class_attr);
                    }
                    out.push_str(" class=\"");
                    out.push_str(&Self::escape_html_attr(&class_attr));
                    out.push('"');
                }
                for (k, v) in props {
                    match v {
                        Expr::StringLit(s, _) => {
                            if k == "css" {
                                continue;
                            }
                            out.push(' ');
                            out.push_str(k);
                            out.push_str("=\"");
                            out.push_str(&Self::escape_html_attr(s));
                            out.push('"');
                        }
                        Expr::BoolLit(true, _) => {
                            out.push(' ');
                            out.push_str(k);
                        }
                        _ => {}
                    }
                }
                out.push('>');
                for c in children {
                    out.push_str(&self.render_ssr_expr(c));
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
                out
            }
            Expr::StringLit(s, _) => Self::escape_html_text(s),
            Expr::Identifier(name, _) => Self::escape_html_text(name),
            Expr::Interpolation(inner, _) => {
                if let Expr::Identifier(name, _) = inner.as_ref() {
                    format!(
                        "<span data-ema-bind=\"{}\"></span>",
                        Self::escape_html_attr(name)
                    )
                } else {
                    String::new()
                }
            }
            Expr::EmbeddedBlock { kind: EmbeddedKind::Html, raw, .. } => raw.clone(),
            Expr::HtmlAst { root, .. } => Self::render_html_node(root),
            Expr::ClientScript(_, _) => String::new(),
            Expr::ServerScript(_, _) => String::new(),
            _ => String::new(),
        }
    }

    fn escape_html_text(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn escape_html_attr(s: &str) -> String {
        Self::escape_html_text(s).replace('\"', "&quot;")
    }

    fn compile_js(&mut self, stmt: &Stmt, indent: usize) {
        let pad = " ".repeat(indent);
        match stmt {
            Stmt::PrintStmt(expr, _) => {
                let val_str = self.compile_expr(expr);
                self.js_output.push_str(&format!("{}console.log({});\n", pad, val_str));
            }
            Stmt::VarDecl { name, value, .. } => {
                let val_str = self.compile_expr(value);
                self.js_output.push_str(&format!("{}const {} = {};\n", pad, name, val_str));
            }
            Stmt::StateDecl { name, value, .. } => {
                let val_str = self.compile_expr(value);
                self.js_output.push_str(&format!("{}EmaApp.state[\"{}\"] = {};\n", pad, name, val_str));
                self.js_output.push_str(&format!("{}console.log('[EMA-LIVE] Reactive State Registered: {}');\n", pad, name));
            }
            Stmt::AssignStmt { name, value, .. } => {
                let val_str = self.compile_expr(value);
                if self.state_vars.contains(name) {
                    self.js_output.push_str(&format!("{}EmaApp.setState(\"{}\", {});\n", pad, name, val_str));
                } else {
                    self.js_output.push_str(&format!("{}{} = {};\n", pad, name, val_str));
                }
            }
            Stmt::ExprStmt(expr, _) => {
                let val_str = self.compile_expr(expr);
                // Wrap in a block so we can reuse `_res` without redeclaration errors.
                self.js_output.push_str(&format!("{}{{ const _res = {};\n", pad, val_str));
                self.js_output.push_str(&format!("{}  if (_res instanceof HTMLElement) {{ parent.appendChild(_res); }}\n", pad));
                self.js_output.push_str(&format!("{}}}\n", pad));
            }
            Stmt::FnDecl { name, params, body, is_async, .. } => {
                let p_names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
                let async_pfx = if *is_async { "async " } else { "" };
                self.js_output.push_str(&format!("{}{}function {}({}) {{\n", pad, async_pfx, name, p_names.join(", ")));
                for s in body {
                    self.compile_js(s, indent + 2);
                }
                self.js_output.push_str(&format!("{}}}\n", pad));
            }
            Stmt::ComponentDecl { .. } => {
                // Handled globally in build_frontend. Do nothing here.
            }
            Stmt::ReturnStmt(expr_opt, _) => {
                let val = if let Some(e) = expr_opt {
                    self.compile_expr(e)
                } else {
                    "undefined".to_string()
                };
                self.js_output.push_str(&format!("{}return {};\n", pad, val));
            }
            _ => {
                self.js_output.push_str(&format!("{}/* [WASM] Skipped Operation: {:?} */\n", pad, stmt));
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::StringLit(s, _) => format!("\"{}\"", s), // Basic JS string translation
            Expr::IntLit(i, _) => format!("{}", i),
            Expr::FloatLit(f, _) => format!("{}", f),
            Expr::BoolLit(b, _) => if *b { "true".to_string() } else { "false".to_string() },
            Expr::Await(inner, _) => format!("(await {})", self.compile_expr(inner)),
            Expr::Identifier(ident, _) => {
                // If it's a known reactive state var, use state access.
                if self.state_vars.contains(ident) {
                    format!("EmaApp.state[\"{}\"]", ident)
                } else {
                    // For bootstrap, treat as a local JS identifier. 
                    // Quoting as string was too aggressive for logic blocks.
                    ident.clone()
                }
            }
            Expr::EmbeddedBlock { kind, raw, span: _ } => {
                match kind {
                    EmbeddedKind::Html => {
                        let tpl = Self::escape_js_template(raw);
                        format!(
                            "((() => {{\
 const container = document.createElement('div');\
 const parser = new DOMParser();\
 const doc = parser.parseFromString(`<template>{}</template>`, 'text/html');\
 const template = doc.querySelector('template');\
 if (!template) return container;\
 const frag = template.content.cloneNode(true);\
 frag.querySelectorAll('script').forEach(s => s.remove());\
 frag.querySelectorAll('*').forEach(el => {{\
   // strip inline event handlers (onclick, onload, ...)\n\
   for (const name of Array.from(el.getAttributeNames())) {{\
     if (name.toLowerCase().startsWith('on')) el.removeAttribute(name);\
   }}\
   // strip javascript: URLs\n\
   for (const attr of ['href','src','xlink:href']) {{\
     const v = el.getAttribute(attr);\
     if (v && /^\\s*javascript:/i.test(v)) el.removeAttribute(attr);\
   }}\
 }});\
 container.appendChild(frag);\
 return container;\
 }})())",
                            tpl
                        )
                    }
                    EmbeddedKind::Css => {
                        let scoped = Self::scope_css(raw, "#ema-root");
                        let tpl = Self::escape_js_template(&scoped);
                        format!("((() => {{ const styleEl = document.createElement('style'); styleEl.textContent = `{}`; document.head.appendChild(styleEl); return null; }})())", tpl)
                    }
                    EmbeddedKind::Js => {
                        self.client_scripts.push(raw.clone());
                        "null".to_string()
                    }
                    EmbeddedKind::Php => {
                        "null".to_string()
                    }
                }
            }
            Expr::HtmlAst { root, .. } => {
                let raw = Self::render_html_node(root);
                let tpl = Self::escape_js_template(&raw);
                format!(
                    "((() => {{\
 const container = document.createElement('div');\
 const parser = new DOMParser();\
 const doc = parser.parseFromString(`<template>{}</template>`, 'text/html');\
 const template = doc.querySelector('template');\
 if (!template) return container;\
 const frag = template.content.cloneNode(true);\
 frag.querySelectorAll('script').forEach(s => s.remove());\
 frag.querySelectorAll('*').forEach(el => {{\
   for (const name of Array.from(el.getAttributeNames())) {{\
     if (name.toLowerCase().startsWith('on')) el.removeAttribute(name);\
   }}\
   for (const attr of ['href','src','xlink:href']) {{\
     const v = el.getAttribute(attr);\
     if (v && /^\\s*javascript:/i.test(v)) el.removeAttribute(attr);\
   }}\
 }});\
 container.appendChild(frag);\
 return container;\
 }})())",
                    tpl
                )
            }
            Expr::CssAst { stylesheet, .. } => {
                let raw = self.render_css_stylesheet(stylesheet);
                let scoped = Self::scope_css(&raw, "#ema-root");
                let tpl = Self::escape_js_template(&scoped);
                format!("((() => {{ const styleEl = document.createElement('style'); styleEl.textContent = `{}`; document.head.appendChild(styleEl); return null; }})())", tpl)
            }
            Expr::JsAst { program, .. } => {
                self.client_scripts.push(Self::render_js_program(program));
                "null".to_string()
            }
            Expr::PhpAst { .. } => "null".to_string(),
            Expr::Binary { left, op, right, span: _ } => {
                let l = self.compile_expr(left);
                let r = self.compile_expr(right);
                let operator = match op {
                    crate::ast::BinaryOp::Add => "+",
                    crate::ast::BinaryOp::Sub => "-",
                    crate::ast::BinaryOp::Mul => "*",
                    crate::ast::BinaryOp::Div => "/",
                    crate::ast::BinaryOp::EqEq => "===",
                    crate::ast::BinaryOp::BangEq => "!==",
                    crate::ast::BinaryOp::Less => "<",
                    crate::ast::BinaryOp::LessEq => "<=",
                    crate::ast::BinaryOp::Greater => ">",
                    crate::ast::BinaryOp::GreaterEq => ">=",
                };
                format!("({} {} {})", l, operator, r)
            }
            Expr::Call { callee, args, span: _ } => {
                let callee_str = self.compile_expr(callee);
                let args_str: Vec<String> = args.iter().map(|a| self.compile_expr(a)).collect();
                format!("{}({})", callee_str, args_str.join(", "))
            }
            Expr::UiElement { tag, props, children, span: _ } => {
                if let Some(first_char) = tag.chars().next() {
                    if first_char.is_uppercase() {
                        let mut props_js = "{ ".to_string();
                        for (i, (p_name, p_expr)) in props.iter().enumerate() {
                            props_js.push_str(&format!("\"{}\": {}", p_name, self.compile_expr(p_expr)));
                            if i < props.len() - 1 { props_js.push_str(", "); }
                        }
                        props_js.push_str(" }");

                        let mut children_js = "[".to_string();
                        for (i, child) in children.iter().enumerate() {
                            children_js.push_str(&self.compile_expr(child));
                            if i < children.len() - 1 { children_js.push_str(", "); }
                        }
                        children_js.push_str("]");
                        if !children.is_empty() {
                            if props.is_empty() {
                                props_js = format!("{{ \"children\": {} }}", children_js);
                            } else {
                                props_js = props_js.replace(" }", &format!(", \"children\": {} }}", children_js));
                            }
                        }

                        return format!("{}({})", tag, props_js);
                    }
                }

                let mut js = format!("((() => {{ const el = document.createElement('{}');", tag);
                // Component-scoped CSS: css: css{...}
                if let Some(css_raw) = props.get("css").and_then(|v| match v {
                    Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, .. } => Some(raw.as_str()),
                    _ => None,
                }) {
                    let class_name = Self::css_class_name(css_raw);
                    let scoped = Self::scope_css(css_raw, &format!(".{}", class_name));
                    let tpl = Self::escape_js_template(&scoped);
                    js.push_str(&format!(" el.classList.add('{}');", class_name));
                    js.push_str(&format!(" {{ const styleEl = document.createElement('style'); styleEl.textContent = `{}`; document.head.appendChild(styleEl); }}", tpl));
                }
                for (name, val) in props {
                    if name == "css" { continue; } // handled above
                    if name == "onclick" {
                        js.push_str(&format!(" el.setAttribute('data-ema-click', `{}`);", self.compile_onclick(val)));
                    } else if name == "oninput" {
                        js.push_str(&format!(" el.setAttribute('data-ema-oninput', `{}`);", self.compile_onclick(val)));
                    } else if name == "onchange" {
                        js.push_str(&format!(" el.setAttribute('data-ema-onchange', `{}`);", self.compile_onclick(val)));
                    } else if name.starts_with("--") {
                        if let Expr::Interpolation(inner, _) = val {
                            if let Expr::Identifier(var_name, _) = inner.as_ref() {
                                js.push_str(&format!(" el.dataset.emaCssVar = '{}'; EmaApp.bindings.push({{ key: '{}', node: el }});", name, var_name));
                            } else {
                                let val_str = self.compile_expr(inner);
                                js.push_str(&format!(" el.style.setProperty('{}', {});", name, val_str));
                            }
                        } else {
                            let val_str = self.compile_expr(val);
                            js.push_str(&format!(" el.style.setProperty('{}', {});", name, val_str));
                        }
                    } else {
                        let val_str = self.compile_expr(val);
                        // For HTML attributes, we often need literal strings if it's a constant.
                        // But if it's an identifier, we use its value.
                        js.push_str(&format!(" el.setAttribute('{}', {});", name, val_str));
                    }
                }
                for child in children {
                    let child_js = self.compile_expr(child);
                    js.push_str(&format!(" {{ const c = {}; if (c instanceof HTMLElement) {{ el.appendChild(c); }} else {{ el.appendChild(document.createTextNode(c)); }} }}", child_js));
                }
                js.push_str(" return el; })())");
                js
            }
            Expr::Interpolation(inner, _span) => {
                if let Expr::Identifier(name, _) = inner.as_ref() {
                    format!("((() => {{ const n = document.createTextNode(''); EmaApp.bindings.push({{ key: \"{}\", node: n }}); return n; }})())", name)
                } else {
                    let inner_js = self.compile_expr(inner);
                    format!("document.createTextNode({})", inner_js)
                }
            }
            Expr::Member { object, property, .. } => {
                let obj_js = self.compile_expr(object);
                format!("{}.{}", obj_js, property)
            }
            Expr::Ternary { condition, then_expr, else_expr, .. } => {
                let cond = self.compile_expr(condition);
                let then = self.compile_expr(then_expr);
                let els = self.compile_expr(else_expr);
                format!("({} ? {} : {})", cond, then, els)
            }
            Expr::StructLiteral { fields, .. } => {
                let mut s = "{ ".to_string();
                for (i, (name, val)) in fields.iter().enumerate() {
                    s.push_str(&format!("\"{}\": {}", name, self.compile_expr(val)));
                    if i < fields.len() - 1 { s.push_str(", "); }
                }
                s.push_str(" }");
                s
            }
            Expr::ClientScript(stmts, _) => {
                for s in stmts {
                    let old_js = std::mem::take(&mut self.js_output);
                    self.compile_js(s, 0);
                    let compiled = std::mem::take(&mut self.js_output);
                    self.js_output = old_js;
                    if !compiled.is_empty() {
                        self.client_scripts.push(compiled);
                    }
                }
                "null".to_string()
            }
            Expr::ServerScript(_, _) => "null".to_string(),
            _ => "\"unsupported_expr\"".to_string(),
        }
    }

    fn render_html_node(node: &HtmlNode) -> String {
        match node {
            HtmlNode::Text { text, .. } => Self::escape_html_text(text),
            HtmlNode::Interpolation { expr, .. } => {
                match expr.as_ref() {
                    Expr::Identifier(name, _) => {
                        format!(
                            "<span data-ema-bind=\"{}\"></span>",
                            Self::escape_html_attr(name)
                        )
                    }
                    other => {
                        if let Some(js_expr) = Self::template_expr_to_js(other) {
                            let key = Self::hash_key(&js_expr);
                            format!(
                                "<span data-ema-bind=\"{}\" data-ema-expr=\"{}\"></span>",
                                Self::escape_html_attr(&key),
                                Self::escape_html_attr(&js_expr)
                            )
                        } else {
                            String::new()
                        }
                    }
                }
            }
            HtmlNode::Comment { text, .. } => format!("<!--{}-->", text),
            HtmlNode::If { condition, then_children, else_children, .. } => {
                // SSR fallback: render then-branch unless condition is a boolean literal false.
                let take_then = !matches!(condition.as_ref(), Expr::BoolLit(false, _));
                let children = if take_then { then_children } else { else_children };
                let mut out = String::new();
                for c in children {
                    out.push_str(&Self::render_html_node(c));
                }
                out
            }
            HtmlNode::ForEach { item, list, body, .. } => {
                // SSR fallback: if list is int literal N, repeat N times (capped).
                let mut n = 1i64;
                if let Expr::IntLit(v, _) = list.as_ref() {
                    n = *v;
                }
                n = n.clamp(0, 200);
                let mut out = String::new();
                for idx in 0..n {
                    for c in body {
                        out.push_str(&Self::render_html_node_with_scope(c, item, idx));
                    }
                }
                out
            }
            HtmlNode::Element { tag, attrs, children, .. } => {
                let mut out = String::new();
                out.push('<');
                out.push_str(tag);
                for a in attrs {
                    let k = &a.name;
                    if k.to_ascii_lowercase().starts_with("on") {
                        // Convert inline events into safe EMA data-* handlers.
                        // Example: onclick="inc(counter)"
                        if k.to_ascii_lowercase() == "onclick" {
                            out.push_str(" data-ema-click=\"");
                            let v = match &a.value {
                                HtmlAttrValue::Static(s) => s.clone(),
                                _ => "".to_string(),
                            };
                            out.push_str(&Self::escape_html_attr(&v));
                            out.push('"');
                        }
                        if k.to_ascii_lowercase() == "oninput" {
                            out.push_str(" data-ema-oninput=\"");
                            let v = match &a.value {
                                HtmlAttrValue::Static(s) => s.clone(),
                                _ => "".to_string(),
                            };
                            out.push_str(&Self::escape_html_attr(&v));
                            out.push('"');
                        }
                        if k.to_ascii_lowercase() == "onchange" {
                            out.push_str(" data-ema-onchange=\"");
                            let v = match &a.value {
                                HtmlAttrValue::Static(s) => s.clone(),
                                _ => "".to_string(),
                            };
                            out.push_str(&Self::escape_html_attr(&v));
                            out.push('"');
                        }
                        continue;
                    }
                    out.push(' ');
                    out.push_str(k);
                    match &a.value {
                        HtmlAttrValue::BoolTrue => {}
                        HtmlAttrValue::Static(v) => {
                            out.push_str("=\"");
                            out.push_str(&Self::escape_html_attr(v));
                            out.push('"');
                        }
                        HtmlAttrValue::Template(_) => {
                            // attribute templating handled in later milestones
                        }
                    }
                }
                out.push('>');
                for c in children {
                    out.push_str(&Self::render_html_node(c));
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
                out
            }
        }
    }

    fn render_html_node_with_scope(node: &HtmlNode, item: &str, idx: i64) -> String {
        match node {
            HtmlNode::Interpolation { expr, .. } => {
                if let Expr::Identifier(name, _) = expr.as_ref() {
                    if name == item {
                        return Self::escape_html_text(&idx.to_string());
                    }
                }
                Self::render_html_node(node)
            }
            HtmlNode::Element { tag, attrs, children, .. } => {
                let mut out = String::new();
                out.push('<');
                out.push_str(tag);
                for a in attrs {
                    // attrs templating for loops not yet supported; static only
                    out.push(' ');
                    out.push_str(&a.name);
                    match &a.value {
                        HtmlAttrValue::BoolTrue => {}
                        HtmlAttrValue::Static(v) => {
                            out.push_str("=\"");
                            out.push_str(&Self::escape_html_attr(v));
                            out.push('"');
                        }
                        HtmlAttrValue::Template(_) => {}
                    }
                }
                out.push('>');
                for c in children {
                    out.push_str(&Self::render_html_node_with_scope(c, item, idx));
                }
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
                out
            }
            HtmlNode::If { condition, then_children, else_children, span } => {
                let _ = span;
                let take_then = !matches!(condition.as_ref(), Expr::BoolLit(false, _));
                let children = if take_then { then_children } else { else_children };
                let mut out = String::new();
                for c in children {
                    out.push_str(&Self::render_html_node_with_scope(c, item, idx));
                }
                out
            }
            HtmlNode::ForEach { item: inner_item, list, body, span, index } => {
                let _ = (span, index);
                let mut n = 1i64;
                if let Expr::IntLit(v, _) = list.as_ref() {
                    n = *v;
                }
                n = n.clamp(0, 200);
                let mut out = String::new();
                for j in 0..n {
                    for c in body {
                        out.push_str(&Self::render_html_node_with_scope(c, inner_item, j));
                    }
                }
                out
            }
            _ => Self::render_html_node(node),
        }
    }

    fn template_expr_to_js(expr: &Expr) -> Option<String> {
        // Safe subset for template computed expressions: identifiers, literals, binary ops.
        match expr {
            Expr::Identifier(name, _) => Some(format!("state[\"{}\"]", Self::escape_js_string(name))),
            Expr::IntLit(i, _) => Some(i.to_string()),
            Expr::FloatLit(f, _) => Some(f.to_string()),
            Expr::BoolLit(b, _) => Some(if *b { "true".to_string() } else { "false".to_string() }),
            Expr::StringLit(s, _) => Some(format!("\"{}\"", Self::escape_js_string(s))),
            Expr::Binary { left, op, right, .. } => {
                let l = Self::template_expr_to_js(left)?;
                let r = Self::template_expr_to_js(right)?;
                let operator = match op {
                    crate::ast::BinaryOp::Add => "+",
                    crate::ast::BinaryOp::Sub => "-",
                    crate::ast::BinaryOp::Mul => "*",
                    crate::ast::BinaryOp::Div => "/",
                    crate::ast::BinaryOp::EqEq => "===",
                    crate::ast::BinaryOp::BangEq => "!==",
                    crate::ast::BinaryOp::Less => "<",
                    crate::ast::BinaryOp::LessEq => "<=",
                    crate::ast::BinaryOp::Greater => ">",
                    crate::ast::BinaryOp::GreaterEq => ">=",
                };
                Some(format!("({} {} {})", l, operator, r))
            }
            Expr::Member { object, property, .. } => {
                let obj_js = Self::template_expr_to_js(object)?;
                Some(format!("{}.{}", obj_js, property))
            }
            _ => None,
        }
    }

    fn hash_key(input: &str) -> String {
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        format!("__ema_expr_{}", hasher.finish())
    }

    fn render_css_stylesheet(&mut self, sheet: &CssStylesheet) -> String {
        let mut out = String::new();
        for node in &sheet.nodes {
            match node {
                crate::ast::CssNode::Rule(r) => {
                    if r.selectors.is_empty() {
                        continue;
                    }
                    out.push_str(&r.selectors.join(", "));
                    out.push_str(" {");
                    for (k, v) in &r.declarations {
                        out.push_str(k);
                        out.push(':');
                        out.push_str(v);
                        out.push(';');
                    }
                    out.push_str("}\n");
                }
                crate::ast::CssNode::AtRule { name, params, .. } => {
                    if name == "tailwind" {
                        self.tailwind_needed = true;
                    } else if name == "bootstrap" {
                        self.bootstrap_needed = true;
                    }
                    out.push('@');
                    out.push_str(name);
                    if !params.is_empty() {
                        out.push(' ');
                        out.push_str(params);
                    }
                    out.push_str(";\n");
                }
            }
        }
        out
    }

    fn render_js_program(program: &JsProgram) -> String {
        let mut out = String::new();
        for s in &program.body {
            out.push_str(&Self::render_js_stmt(s));
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }

    fn render_js_stmt(stmt: &JsStmt) -> String {
        match stmt {
            JsStmt::Expr(e, _) => {
                let mut s = Self::render_js_expr(e);
                if !s.trim_end().ends_with(';') {
                    s.push(';');
                }
                s
            }
            JsStmt::VarDecl { kind, pattern, value, .. } => {
                let k = match kind {
                    JsVarKind::Const => "const",
                    JsVarKind::Let => "let",
                    JsVarKind::Var => "var",
                };
                let pat = Self::render_js_pattern(pattern);
                if let Some(v) = value {
                    format!("{} {} = {};", k, pat, Self::render_js_expr(v))
                } else {
                    format!("{} {};", k, pat)
                }
            }
            JsStmt::Return(v, _) => {
                if let Some(e) = v {
                    format!("return {};", Self::render_js_expr(e))
                } else {
                    "return;".to_string()
                }
            }
            JsStmt::Block { body, .. } => {
                let mut out = String::new();
                out.push_str("{\n");
                for s in body {
                    out.push_str(&Self::render_js_stmt(s));
                    out.push('\n');
                }
                out.push('}');
                out
            }
            JsStmt::If { condition, then_branch, else_branch, .. } => {
                let mut out = format!("if ({}) {}", Self::render_js_expr(condition), Self::render_js_stmt(then_branch));
                if let Some(e) = else_branch {
                    out.push_str(" else ");
                    out.push_str(&Self::render_js_stmt(e));
                }
                out
            }
            JsStmt::While { condition, body, .. } => {
                format!("while ({}) {}", Self::render_js_expr(condition), Self::render_js_stmt(body))
            }
            JsStmt::For { init, condition, update, body, .. } => {
                let init_s = init.as_ref().map(|s| Self::render_js_stmt(s).trim_end_matches(';').to_string()).unwrap_or_default();
                let cond_s = condition.as_ref().map(Self::render_js_expr).unwrap_or_default();
                let upd_s = update.as_ref().map(Self::render_js_expr).unwrap_or_default();
                format!("for ({}; {}; {}) {}", init_s, cond_s, upd_s, Self::render_js_stmt(body))
            }
            JsStmt::ClassDecl { name, extends, body, .. } => {
                let ext_str = if let Some(e) = extends { format!(" extends {}", e) } else { "".to_string() };
                let mut out = format!("class {}{} {{\n", name, ext_str);
                for s in body {
                    if let JsStmt::FunctionDecl { name, params, body, is_async, .. } = s {
                        let prefix = if *is_async { "async " } else { "" };
                        out.push_str(&format!("  {}{}({}) {}\n", prefix, name, Self::render_js_params(params), Self::render_js_stmt(body)));
                    } else {
                        out.push_str(&Self::render_js_stmt(s));
                        out.push('\n');
                    }
                }
                out.push_str("}");
                out
            }
            JsStmt::FunctionDecl { name, params, body, is_async, .. } => {
                let prefix = if *is_async { "async " } else { "" };
                format!("{}function {}({}) {}", prefix, name, Self::render_js_params(params), Self::render_js_stmt(body))
            }
            JsStmt::TryCatch { try_block, catch_name, catch_block, .. } => {
                format!("try {} catch ({}) {}", Self::render_js_stmt(try_block), catch_name, Self::render_js_stmt(catch_block))
            }
            JsStmt::Throw(e, _) => {
                format!("throw {};", Self::render_js_expr(e))
            }
            JsStmt::ClassDecl { .. } | JsStmt::Switch { .. } | JsStmt::Break(_) | JsStmt::Continue(_) => {
                "/* not supported in WASM yet */".to_string()
            }
        }
    }

    fn render_js_expr(expr: &JsExpr) -> String {
        match expr {
            JsExpr::Ident(s, _) => s.clone(),
            JsExpr::String(s, _) => format!("\"{}\"", Self::escape_js_string(s)),
            JsExpr::Number(n, _) => format!("{}", n),
            JsExpr::Bool(b, _) => if *b { "true".to_string() } else { "false".to_string() },
            JsExpr::Null(_) => "null".to_string(),
            JsExpr::Member { object, property, .. } => format!("{}.{}", Self::render_js_expr(object), property),
            JsExpr::Call { callee, args, .. } => {
                let a: Vec<String> = args.iter().map(Self::render_js_expr).collect();
                format!("{}({})", Self::render_js_expr(callee), a.join(", "))
            }
            JsExpr::Binary { left, op, right, .. } => format!("({} {} {})", Self::render_js_expr(left), op, Self::render_js_expr(right)),
            JsExpr::Unary { op, expr, .. } => format!("({}{})", op, Self::render_js_expr(expr)),
            JsExpr::Assign { target, value, .. } => format!("({} = {})", Self::render_js_expr(target), Self::render_js_expr(value)),
            JsExpr::Conditional { condition, then_expr, else_expr, .. } => {
                format!("({} ? {} : {})", Self::render_js_expr(condition), Self::render_js_expr(then_expr), Self::render_js_expr(else_expr))
            }
            JsExpr::ObjectLit { props, .. } => {
                let p: Vec<String> = props
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, Self::render_js_expr(v)))
                    .collect();
                format!("{{{}}}", p.join(", "))
            }
            JsExpr::ArrayLit { items, .. } => {
                let p: Vec<String> = items.iter().map(Self::render_js_expr).collect();
                format!("[{}]", p.join(", "))
            }
            JsExpr::ArrowFn { params, body, is_async, .. } => {
                let prefix = if *is_async { "async " } else { "" };
                format!("({}({}) => {})", prefix, Self::render_js_params(params), Self::render_js_stmt(body))
            }
            JsExpr::Await { expr, .. } => {
                format!("(await {})", Self::render_js_expr(expr))
            }
            JsExpr::Spread { expr, .. } => format!("...{}", Self::render_js_expr(expr)),
            JsExpr::TemplateLit { parts, .. } => {
                let mut out = String::new();
                out.push('`');
                for p in parts {
                    match p {
                        JsTemplatePart::Str(s) => out.push_str(&s.replace('`', "\\`")),
                        JsTemplatePart::Expr(e) => {
                            out.push_str("${");
                            out.push_str(&Self::render_js_expr(e));
                            out.push('}');
                        }
                    }
                }
                out.push('`');
                out
            }
            JsExpr::Update { op, is_prefix, expr, .. } => {
                if *is_prefix {
                    format!("{}{}", op, Self::render_js_expr(expr))
                } else {
                    format!("{}{}", Self::render_js_expr(expr), op)
                }
            }
            JsExpr::New { .. } | JsExpr::This(_) | JsExpr::Super(_) => {
                "/* not supported in WASM yet */".to_string()
            }
        }
    }

    fn render_js_pattern(p: &JsPattern) -> String {
        match p {
            JsPattern::Ident(s, _) => s.clone(),
            JsPattern::Object { props, rest, .. } => {
                let mut parts: Vec<String> = props
                    .iter()
                    .map(|(k, alias)| {
                        if let Some(a) = alias {
                            format!("{}: {}", k, a)
                        } else {
                            k.clone()
                        }
                    })
                    .collect();
                if let Some(r) = rest {
                    parts.push(format!("...{}", r));
                }
                format!("{{{}}}", parts.join(", "))
            }
            JsPattern::Array { items, rest, .. } => {
                let mut parts: Vec<String> = items
                    .iter()
                    .map(|o| o.clone().unwrap_or_default())
                    .collect();
                if let Some(r) = rest {
                    parts.push(format!("...{}", r));
                }
                format!("[{}]", parts.join(", "))
            }
        }
    }

    fn render_js_params(params: &[JsParam]) -> String {
        let mut out = Vec::new();
        for p in params {
            let mut s = String::new();
            if p.is_rest {
                s.push_str("...");
            }
            s.push_str(&p.name);
            if let Some(d) = &p.default {
                s.push_str(" = ");
                s.push_str(&Self::render_js_expr(d));
            }
            out.push(s);
        }
        out.join(", ")
    }

    fn compile_onclick(&mut self, expr: &Expr) -> String {
        match expr {
            // Treat string literal as raw JS (not quoted)
            Expr::StringLit(s, _) => {
                let trimmed = s.trim();
                if trimmed.ends_with(';') {
                    trimmed.to_string()
                } else {
                    format!("{};", trimmed)
                }
            }
            // Mini handler syntax: set(counter, counter + 1)  -> EmaApp.setState("counter", <expr>);
            Expr::Call { callee, args, span: _ } => {
                if let Expr::Identifier(name, _) = callee.as_ref() {
                    if name == "set" && args.len() == 2 {
                        if let Some(key) = self.state_key_from_expr(&args[0]) {
                            let value_js = self.compile_expr(&args[1]);
                            return format!("EmaApp.setState(\"{}\", {});", key, value_js);
                        }
                    }
                    if name == "inc" && args.len() == 1 {
                        if let Some(key) = self.state_key_from_expr(&args[0]) {
                            return format!("EmaApp.setState(\"{}\", ((EmaApp.state[\"{}\"] ?? 0) + 1));", key, key);
                        }
                    }
                    if name == "dec" && args.len() == 1 {
                        if let Some(key) = self.state_key_from_expr(&args[0]) {
                            return format!("EmaApp.setState(\"{}\", ((EmaApp.state[\"{}\"] ?? 0) - 1));", key, key);
                        }
                    }
                    if name == "toggle" && args.len() == 1 {
                        if let Some(key) = self.state_key_from_expr(&args[0]) {
                            return format!("EmaApp.setState(\"{}\", !(EmaApp.state[\"{}\"] ?? false));", key, key);
                        }
                    }
                }
                let e = self.compile_expr(expr);
                format!("{};", e)
            }
            // Otherwise compile as expression and evaluate
            Expr::Await(inner, _) => {
                format!("await {};", self.compile_expr(inner))
            }
            _ => {
                let e = self.compile_expr(expr);
                format!("{};", e)
            }
        }
    }

    fn state_key_from_expr(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Identifier(name, _) if self.state_vars.contains(name) => Some(name.clone()),
            Expr::StringLit(s, _) => Some(s.clone()),
            _ => None,
        }
    }

    fn escape_js_template(input: &str) -> String {
        // Escape for use inside JS template literals: `...`
        // - backticks must be escaped
        // - ${ must be escaped to avoid interpolation
        input
            .replace('`', "\\`")
            .replace("${", "\\${")
    }

    fn escape_js_string(input: &str) -> String {
        input
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    fn css_class_name(raw_css: &str) -> String {
        let mut h = DefaultHasher::new();
        raw_css.hash(&mut h);
        format!("ema-css-{:x}", h.finish())
    }

    fn scope_css(raw: &str, scope: &str) -> String {
        let s = raw.trim();
        Self::scope_css_inner(s, scope)
    }

    fn scope_css_inner(input: &str, scope: &str) -> String {
        let mut out = String::new();
        let mut i = 0usize;
        let bytes = input.as_bytes();

        while i < bytes.len() {
            // Skip whitespace
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                out.push(bytes[i] as char);
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }

            // @media / @supports / @keyframes passthrough with recursive scoping
            if bytes[i] == b'@' {
                let rule_start = i;
                // read at-rule header until '{'
                while i < bytes.len() && bytes[i] != b'{' {
                    i += 1;
                }
                let header = input[rule_start..i].trim();
                out.push_str(header);

                if i >= bytes.len() {
                    break;
                }
                // consume '{'
                out.push('{');
                i += 1;

                // read balanced block body
                let body_start = i;
                let mut depth = 1i32;
                let mut in_str: Option<u8> = None;
                while i < bytes.len() {
                    let c = bytes[i];
                    if let Some(q) = in_str {
                        if c == b'\\' {
                            i += 1;
                            if i < bytes.len() { i += 1; }
                            continue;
                        }
                        if c == q { in_str = None; }
                        i += 1;
                        continue;
                    }
                    if c == b'"' || c == b'\'' { in_str = Some(c); i += 1; continue; }
                    if c == b'{' { depth += 1; }
                    else if c == b'}' { depth -= 1; if depth == 0 { break; } }
                    i += 1;
                }
                let i_clamped = i.min(bytes.len());
                let body = &input[body_start..i_clamped];
                
                // If it's a keyframes rule, we scope the rule names inside as 0%, 100% etc (i.e. don't prepend scope)
                if header.to_ascii_lowercase().contains("keyframes") {
                   out.push_str(body); // Keyframes don't need selector scoping
                } else {
                   out.push_str(&Self::scope_css_inner(body, scope));
                }

                if i < bytes.len() && bytes[i] == b'}' {
                    out.push('}');
                    i += 1;
                }
                continue;
            }

            // Normal rule: selectors { declarations }
            let sel_start = i;
            while i < bytes.len() && bytes[i] != b'{' {
                i += 1;
            }
            if i >= bytes.len() {
                // trailing junk
                out.push_str(&input[sel_start..]);
                break;
            }
            let selectors = input[sel_start..i].trim();
            let scoped_selectors = selectors
                .split(',')
                .map(|s| Self::scope_selector(s.trim(), scope))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&scoped_selectors);

            // copy block including balanced braces
            out.push('{');
            i += 1;
            let decl_start = i;
            let mut depth = 1i32;
            let mut in_str: Option<u8> = None;
            while i < bytes.len() {
                let c = bytes[i];
                if let Some(q) = in_str {
                    if c == b'\\' {
                        i += 1;
                        if i < bytes.len() { i += 1; }
                        continue;
                    }
                    if c == q { in_str = None; }
                    i += 1;
                    continue;
                }
                if c == b'"' || c == b'\'' { in_str = Some(c); i += 1; continue; }
                if c == b'{' { depth += 1; }
                else if c == b'}' { depth -= 1; if depth == 0 { break; } }
                i += 1;
            }
            let i_clamped = i.min(bytes.len());
            out.push_str(&input[decl_start..i_clamped]);
            if i < bytes.len() && bytes[i] == b'}' {
                out.push('}');
                i += 1;
            }
        }

        out
    }

    fn scope_selector(selector: &str, scope: &str) -> String {
        if selector.is_empty() {
            return scope.to_string();
        }

        let sel = selector.trim();
        let lower = sel.to_ascii_lowercase();
        let is_global = lower == "html"
            || lower.starts_with("html ")
            || lower == "body"
            || lower.starts_with("body ")
            || lower == ":root"
            || lower.starts_with(":root ");

        if is_global {
            return sel.to_string();
        }
        if sel.starts_with(scope) {
            return sel.to_string();
        }
        format!("{} {}", scope, sel)
    }
}
