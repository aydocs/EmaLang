use crate::ast::{CssStylesheet, Expr, HtmlAttrValue, HtmlNode, JsExpr, JsParam, JsPattern, JsProgram, JsStmt, JsTemplatePart, JsVarKind, Program, Stmt, EmbeddedKind};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub struct WasmBuilder {
    js_output: String,
    client_scripts: Vec<String>,
    state_vars: HashSet<String>,
    state_inits: Vec<String>,
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
        js.push_str("    root.querySelectorAll('[data-ema-bind]').forEach(el => {\n");
        js.push_str("      const key = el.getAttribute('data-ema-bind');\n");
        js.push_str("      if (!key) return;\n");
        js.push_str("      const expr = el.getAttribute('data-ema-expr');\n");
        js.push_str("      if (expr && !this.computed[key]) {\n");
        js.push_str("        try { this.computed[key] = new Function('state', 'return (' + expr + ')'); } catch (e) {}\n");
        js.push_str("      }\n");
        js.push_str("      this.bindings.push({ key, node: el });\n");
        js.push_str("    });\n");
        js.push_str("  },\n");
        js.push_str("  wireDomEvents: function(root) {\n");
        js.push_str("    if (!root) return;\n");
        js.push_str("    root.querySelectorAll('[data-ema-onclick]').forEach(el => {\n");
        js.push_str("      const spec = el.getAttribute('data-ema-onclick');\n");
        js.push_str("      if (!spec) return;\n");
        js.push_str("      el.onclick = () => {\n");
        js.push_str("        const mInc = spec.match(/^\\s*inc\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*\\)\\s*$/);\n");
        js.push_str("        const mDec = spec.match(/^\\s*dec\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*\\)\\s*$/);\n");
        js.push_str("        const mTog = spec.match(/^\\s*toggle\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*\\)\\s*$/);\n");
        js.push_str("        const mSet = spec.match(/^\\s*set\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*,\\s*(.+)\\s*\\)\\s*$/);\n");
        js.push_str("        if (mInc) { const k = mInc[1]; this.setState(k, ((this.state[k] ?? 0) + 1)); return; }\n");
        js.push_str("        if (mDec) { const k = mDec[1]; this.setState(k, ((this.state[k] ?? 0) - 1)); return; }\n");
        js.push_str("        if (mTog) { const k = mTog[1]; this.setState(k, !(this.state[k] ?? false)); return; }\n");
        js.push_str("        if (mSet) {\n");
        js.push_str("          const k = mSet[1];\n");
        js.push_str("          let raw = String(mSet[2]).trim();\n");
        js.push_str("          let v = raw;\n");
        js.push_str("          if (/^\\d+(\\.\\d+)?$/.test(raw)) v = Number(raw);\n");
        js.push_str("          else if (raw === 'true') v = true;\n");
        js.push_str("          else if (raw === 'false') v = false;\n");
        js.push_str("          else if ((raw.startsWith('\"') && raw.endsWith('\"')) || (raw.startsWith(\"'\") && raw.endsWith(\"'\"))) v = raw.slice(1, -1);\n");
        js.push_str("          this.setState(k, v);\n");
        js.push_str("          return;\n");
        js.push_str("        }\n");
        js.push_str("      };\n");
        js.push_str("    });\n");
        js.push_str("    const wireValueEvent = (attr, evtName) => {\n");
        js.push_str("      root.querySelectorAll('[' + attr + ']').forEach(el => {\n");
        js.push_str("        const spec = el.getAttribute(attr);\n");
        js.push_str("        if (!spec) return;\n");
        js.push_str("        el.addEventListener(evtName, (ev) => {\n");
        js.push_str("          const mSetVal = spec.match(/^\\s*set\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*,\\s*value\\s*\\)\\s*$/);\n");
        js.push_str("          const mSet = spec.match(/^\\s*set\\s*\\(\\s*([A-Za-z_][A-Za-z0-9_]*)\\s*,\\s*(.+)\\s*\\)\\s*$/);\n");
        js.push_str("          if (mSetVal) {\n");
        js.push_str("            const k = mSetVal[1];\n");
        js.push_str("            const t = ev && ev.target ? ev.target : el;\n");
        js.push_str("            const raw = (t && (t.value !== undefined)) ? String(t.value) : '';\n");
        js.push_str("            this.setState(k, raw);\n");
        js.push_str("            return;\n");
        js.push_str("          }\n");
        js.push_str("          if (mSet) {\n");
        js.push_str("            const k = mSet[1];\n");
        js.push_str("            let raw = String(mSet[2]).trim();\n");
        js.push_str("            let v = raw;\n");
        js.push_str("            if (/^\\d+(\\.\\d+)?$/.test(raw)) v = Number(raw);\n");
        js.push_str("            else if (raw === 'true') v = true;\n");
        js.push_str("            else if (raw === 'false') v = false;\n");
        js.push_str("            else if ((raw.startsWith('\"') && raw.endsWith('\"')) || (raw.startsWith(\"'\") && raw.endsWith(\"'\"))) v = raw.slice(1, -1);\n");
        js.push_str("            this.setState(k, v);\n");
        js.push_str("            return;\n");
        js.push_str("          }\n");
        js.push_str("        });\n");
        js.push_str("      });\n");
        js.push_str("    };\n");
        js.push_str("    wireValueEvent('data-ema-oninput', 'input');\n");
        js.push_str("    wireValueEvent('data-ema-onchange', 'change');\n");
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
        js.push_str("    // If SSR already populated #ema-root and hashes match, bind to it without clearing.\n");
        js.push_str("    if (canHydrate && root && root.childNodes && root.childNodes.length > 0) {\n");
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
        }
    }

    pub fn build_frontend(&mut self, program: &Program) -> String {
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
        self.js_output.push_str("        b.node.textContent = (v === undefined || v === null) ? '' : String(v);\n");
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
        
        self.js_output.clone()
    }

    pub fn build_ssr_html(&self, program: &Program) -> String {
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

    pub fn build_ssr_css(&self, program: &Program) -> String {
        // SSR CSS bundle: combine global client css blocks and component-scoped css props.
        let mut out = String::new();
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

    fn collect_ssr_css_from_expr(&self, expr: &Expr, out: &mut String) {
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
                let raw = Self::render_css_stylesheet(stylesheet);
                let scoped = Self::scope_css(&raw, "#ema-root");
                out.push_str(&scoped);
                if !scoped.ends_with('\n') {
                    out.push('\n');
                }
            }
            Expr::UiElement { props, children, .. } => {
                if let Some((css_raw, class_name)) = props.get("css").and_then(|v| match v {
                    Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, .. } => {
                        Some((raw.clone(), Self::css_class_name(raw)))
                    }
                    Expr::CssAst { stylesheet, .. } => {
                        let raw = Self::render_css_stylesheet(stylesheet);
                        Some((raw.clone(), Self::css_class_name(&raw)))
                    }
                    _ => None,
                }) {
                    let scoped = Self::scope_css(&css_raw, &format!(".{}", class_name));
                    out.push_str(&scoped);
                    if !scoped.ends_with('\n') {
                        out.push('\n');
                    }
                }
                for c in children {
                    self.collect_ssr_css_from_expr(c, out);
                }
            }
            _ => {}
        }
    }

    fn render_ssr_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::UiElement { tag, props, children, .. } => {
                let mut out = String::new();
                out.push('<');
                out.push_str(tag);
                // Component-scoped CSS: add class deterministically for SSR too
                if let Some((_css_raw, class_name)) = props.get("css").and_then(|v| match v {
                    Expr::EmbeddedBlock { kind: EmbeddedKind::Css, raw, .. } => {
                        Some((raw.clone(), Self::css_class_name(raw)))
                    }
                    Expr::CssAst { stylesheet, .. } => {
                        let raw = Self::render_css_stylesheet(stylesheet);
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
            Stmt::VarDecl { name, value, span: _ } => {
                let val_str = self.compile_expr(value);
                self.js_output.push_str(&format!("{}const {} = {};\n", pad, name, val_str));
            }
            Stmt::StateDecl { name, value, span: _ } => {
                let val_str = self.compile_expr(value);
                self.js_output.push_str(&format!("{}EmaApp.state[\"{}\"] = {};\n", pad, name, val_str));
                self.js_output.push_str(&format!("{}console.log('[EMA-LIVE] Reactive State Registered: {}');\n", pad, name));
            }
            Stmt::ExprStmt(expr, _) => {
                let val_str = self.compile_expr(expr);
                // Wrap in a block so we can reuse `_res` without redeclaration errors.
                self.js_output.push_str(&format!("{}{{ const _res = {};\n", pad, val_str));
                self.js_output.push_str(&format!("{}  if (_res instanceof HTMLElement) {{ parent.appendChild(_res); }}\n", pad));
                self.js_output.push_str(&format!("{}}}\n", pad));
            }
            // Add translation for specific DOM manipulations here later
            _ => {
                self.js_output.push_str(&format!("{}/* [WASM] Atlanan Islem: {:?} */\n", pad, stmt));
            }
        }
    }

    fn compile_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::StringLit(s, _) => format!("\"{}\"", s), // Basic JS string translation
            Expr::IntLit(i, _) => format!("{}", i),
            Expr::FloatLit(f, _) => format!("{}", f),
            Expr::BoolLit(b, _) => if *b { "true".to_string() } else { "false".to_string() },
            Expr::Identifier(ident, _) => {
                // In UI trees, bare identifiers are often used as text (e.g. <h1> Hello </h1>).
                // If it's not a known reactive state var, treat it as a string literal.
                if self.state_vars.contains(ident) {
                    format!("EmaApp.state[\"{}\"]", ident)
                } else {
                    format!("\"{}\"", Self::escape_js_string(ident))
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
                        // JS blocks are lifted into `clientScripts` during build_frontend.
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
                let raw = Self::render_css_stylesheet(stylesheet);
                let scoped = Self::scope_css(&raw, "#ema-root");
                let tpl = Self::escape_js_template(&scoped);
                format!("((() => {{ const styleEl = document.createElement('style'); styleEl.textContent = `{}`; document.head.appendChild(styleEl); return null; }})())", tpl)
            }
            Expr::JsAst { .. } => {
                // Lifted into clientScripts during build_frontend.
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
                    if name == "onclick" {
                        let handler = self.compile_onclick(val);
                        js.push_str(&format!(" el.onclick = () => {{ {} }};", handler));
                    } else if name == "css" {
                        // handled above
                    } else {
                        let val_str = self.compile_expr(val);
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
                // Interpolations become reactive bindings into EmaApp.state when possible.
                // For identifiers: {{ counter }} -> binding to state['counter']
                if let Expr::Identifier(name, _) = inner.as_ref() {
                    format!("((() => {{ const n = document.createTextNode(''); EmaApp.bindings.push({{ key: \"{}\", node: n }}); return n; }})())", name)
                } else {
                    let inner_js = self.compile_expr(inner);
                    format!("document.createTextNode({})", inner_js)
                }
            }
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
                            out.push_str(" data-ema-onclick=\"");
                            let v = match &a.value {
                                HtmlAttrValue::Static(s) => s,
                                _ => "",
                            };
                            out.push_str(&Self::escape_html_attr(v));
                            out.push('"');
                        }
                        if k.to_ascii_lowercase() == "oninput" {
                            out.push_str(" data-ema-oninput=\"");
                            let v = match &a.value {
                                HtmlAttrValue::Static(s) => s,
                                _ => "",
                            };
                            out.push_str(&Self::escape_html_attr(v));
                            out.push('"');
                        }
                        if k.to_ascii_lowercase() == "onchange" {
                            out.push_str(" data-ema-onchange=\"");
                            let v = match &a.value {
                                HtmlAttrValue::Static(s) => s,
                                _ => "",
                            };
                            out.push_str(&Self::escape_html_attr(v));
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
            _ => None,
        }
    }

    fn hash_key(input: &str) -> String {
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        format!("__ema_expr_{}", hasher.finish())
    }

    fn render_css_stylesheet(sheet: &CssStylesheet) -> String {
        let mut out = String::new();
        for r in &sheet.rules {
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
            JsStmt::FunctionDecl { name, params, body, .. } => {
                format!("function {}({}) {}", name, Self::render_js_params(params), Self::render_js_stmt(body))
            }
            JsStmt::TryCatch { try_block, catch_name, catch_block, .. } => {
                format!("try {} catch ({}) {}", Self::render_js_stmt(try_block), catch_name, Self::render_js_stmt(catch_block))
            }
            JsStmt::Throw(e, _) => {
                format!("throw {};", Self::render_js_expr(e))
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
            JsExpr::ArrowFn { params, body, .. } => {
                format!("(({}) => {})", Self::render_js_params(params), Self::render_js_stmt(body))
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

    fn compile_onclick(&self, expr: &Expr) -> String {
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
        // Very small CSS scoper for MVP.
        // - Prefixes selectors with `scope` to prevent leaking styles globally.
        // - Leaves truly-global selectors (`html`, `body`, `:root`) untouched.
        // - Tries to preserve @media blocks by scoping their inner rules.
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

            // @media / @supports passthrough with recursive scoping
            if bytes[i] == b'@' {
                // read at-rule header until '{'
                while i < bytes.len() && bytes[i] != b'{' {
                    out.push(bytes[i] as char);
                    i += 1;
                }
                if i >= bytes.len() {
                    // no body
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
                            i += 2;
                            continue;
                        }
                        if c == q {
                            in_str = None;
                        }
                        i += 1;
                        continue;
                    }
                    if c == b'"' || c == b'\'' {
                        in_str = Some(c);
                        i += 1;
                        continue;
                    }
                    if c == b'{' {
                        depth += 1;
                    } else if c == b'}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    i += 1;
                }
                let body = &input[body_start..i];
                out.push_str(&Self::scope_css_inner(body, scope));
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
                        i += 2;
                        continue;
                    }
                    if c == q {
                        in_str = None;
                    }
                    i += 1;
                    continue;
                }
                if c == b'"' || c == b'\'' {
                    in_str = Some(c);
                    i += 1;
                    continue;
                }
                if c == b'{' {
                    depth += 1;
                } else if c == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                i += 1;
            }
            out.push_str(&input[decl_start..i]);
            if i < bytes.len() && bytes[i] == b'}' {
                out.push('}');
                i += 1;
            }
        }

        out
    }

    fn scope_selector(selector: &str, scope: &str) -> String {
        if selector.is_empty() {
            return selector.to_string();
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
