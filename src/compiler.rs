use crate::ast::{Program, Stmt, Expr, BinaryOp, EmaType};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompilationTarget {
    Native,
    Wasm,
}

pub struct LlvmCompiler {
    ir_code: String,
    tmp_counter: usize,
    scopes: Vec<HashMap<String, EmaType>>,
    target: CompilationTarget,
}

impl LlvmCompiler {
    pub fn new(target: CompilationTarget) -> Self {
        let mut ir = String::new();
        // LLVM Module Target definitions
        ir.push_str("; ModuleID = 'ema_master_module'\n");
        ir.push_str("source_filename = \"ema_program.ema\"\n");
        
        match target {
            CompilationTarget::Native => {
                ir.push_str("target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128\"\n");
                ir.push_str("target triple = \"x86_64-pc-windows-msvc\"\n\n");
            }
            CompilationTarget::Wasm => {
                ir.push_str("target datalayout = \"e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-n32:64-S128\"\n");
                ir.push_str("target triple = \"wasm32-unknown-unknown\"\n\n");
            }
        }

        // Standard library declarations
        ir.push_str("declare i32 @printf(ptr, ...)\n");
        ir.push_str("declare ptr @ema_malloc(i64)\n");
        ir.push_str("declare void @ema_free(ptr)\n");
        ir.push_str("declare void @ema_retain(ptr)\n");
        ir.push_str("declare void @ema_release(ptr)\n");

        if target == CompilationTarget::Native {
            ir.push_str("declare ptr @ema_fs_read(ptr)\n");
            ir.push_str("declare void @ema_fs_write(ptr, ptr)\n");
        } else {
            // WASM DOM Imports
            ir.push_str("declare ptr @ema_dom_create_element(ptr)\n");
            ir.push_str("declare void @ema_dom_append_child(ptr, ptr)\n");
            ir.push_str("declare void @ema_dom_set_text(ptr, ptr)\n");
        }

        ir.push_str("@.str.int = private unnamed_addr constant [4 x i8] c\"%d\\0A\\00\", align 1\n");
        ir.push_str("@.str.string = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n\n");
        
        LlvmCompiler {
            ir_code: ir,
            tmp_counter: 1,
            scopes: vec![HashMap::new()],
            target,
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        if let Some(scope) = self.scopes.pop() {
            // ARC: Release all heap-allocated variables in this scope
            for (name, ema_type) in scope {
                if self.is_heap_type(&ema_type) {
                    self.ir_code.push_str(&format!("  %ptr_{} = load ptr, ptr %{}, align 8\n", name, name));
                    self.ir_code.push_str(&format!("  call void @ema_release(ptr %ptr_{})\n", name));
                }
            }
        }
    }

    fn is_heap_type(&self, t: &EmaType) -> bool {
        match t {
            EmaType::Str | EmaType::Model { .. } | EmaType::Function { .. } => true,
            _ => false,
        }
    }

    fn define(&mut self, name: String, t: EmaType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, t);
        }
    }

    fn get_type(&self, name: &str) -> Option<EmaType> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t.clone());
            }
        }
        None
    }

    fn new_tmp(&mut self) -> String {
        let name = format!("%{}", self.tmp_counter);
        self.tmp_counter += 1;
        name
    }

    fn new_label(&mut self) -> String {
        let name = format!("L{}", self.tmp_counter);
        self.tmp_counter += 1;
        name
    }

    pub fn compile(&mut self, program: &Program) -> String {
        self.ir_code.push_str("define dso_local i32 @main() {\n");
        self.ir_code.push_str("entry:\n");

        for stmt in &program.statements {
            self.compile_stmt(stmt);
        }

        self.ir_code.push_str("  ret i32 0\n");
        self.ir_code.push_str("}\n");
        self.ir_code.clone()
    }

    fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { name, value, .. } => {
                let ema_type = self.infer_type(value);
                let val_reg = self.compile_expr(value);
                let llvm_type = if self.is_heap_type(&ema_type) { "ptr" } else { "i64" };
                
                self.ir_code.push_str(&format!("  %{} = alloca {}, align 8\n", name, llvm_type));
                
                // ARC: If assigning from another variable, retain it.
                // (Literals are created with RC=1 already in compile_expr)
                if let Expr::Identifier(..) = value {
                    if self.is_heap_type(&ema_type) {
                        self.ir_code.push_str(&format!("  call void @ema_retain(ptr {})\n", val_reg));
                    }
                }
                
                self.ir_code.push_str(&format!("  store {} {}, ptr %{}, align 8\n", llvm_type, val_reg, name));
                self.define(name.clone(), ema_type);
            }
            Stmt::AssignStmt { name, value, .. } => {
                let ema_type = self.infer_type(value);
                let val_reg = self.compile_expr(value);
                
                // ARC: release old value
                let old_type = self.get_type(name).unwrap_or(EmaType::Int);
                if self.is_heap_type(&old_type) {
                     self.ir_code.push_str(&format!("  %old_ptr_{} = load ptr, ptr %{}, align 8\n", name, name));
                     self.ir_code.push_str(&format!("  call void @ema_release(ptr %old_ptr_{})\n", name));
                }

                if let Expr::Identifier(..) = value {
                    if self.is_heap_type(&ema_type) {
                        self.ir_code.push_str(&format!("  call void @ema_retain(ptr {})\n", val_reg));
                    }
                }

                let llvm_type = if self.is_heap_type(&ema_type) { "ptr" } else { "i64" };
                self.ir_code.push_str(&format!("  store {} {}, ptr %{}, align 8\n", llvm_type, val_reg, name));
            }
            Stmt::PrintStmt(expr, _) => {
                let ema_type = self.infer_type(expr);
                let val_reg = self.compile_expr(expr);
                let tmp = self.new_tmp();
                if ema_type == EmaType::Str {
                    // ARC string: data starts at offset 8 (after RC header)
                    let data_ptr = self.new_tmp();
                    self.ir_code.push_str(&format!("  {} = getelementptr i8, ptr {}, i64 8\n", data_ptr, val_reg));
                    self.ir_code.push_str(&format!("  {} = call i32 (ptr, ...) @printf(ptr @.str.string, ptr {})\n", tmp, data_ptr));
                } else {
                    self.ir_code.push_str(&format!("  {} = call i32 (ptr, ...) @printf(ptr @.str.int, i64 {})\n", tmp, val_reg));
                }
            }
            Stmt::ExprStmt(expr, _) => {
                self.compile_expr(expr);
            }
            Stmt::FnDecl { name, body, .. } => {
                self.ir_code.push_str(&format!("\ndefine i32 @{}(i32 %0) {{\n", name));
                self.enter_scope();
                for s in body {
                    self.compile_stmt(s);
                }
                self.exit_scope();
                self.ir_code.push_str("  ret i32 0\n}\n");
            }
            Stmt::ReturnStmt(expr_opt, _) => {
                let val = if let Some(e) = expr_opt { self.compile_expr(e) } else { "0".to_string() };
                self.ir_code.push_str(&format!("  ret i32 {}\n", val));
            }
            Stmt::IfStmt { condition, then_branch, else_branch, .. } => {
                let cond_reg = self.compile_expr(condition);
                let cond_i1 = self.new_tmp();
                self.ir_code.push_str(&format!("  {} = icmp ne i64 {}, 0\n", cond_i1, cond_reg));
                
                let then_label = self.new_label();
                let else_label = self.new_label();
                let merge_label = self.new_label();

                if else_branch.is_some() {
                    self.ir_code.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_i1, then_label, else_label));
                } else {
                    self.ir_code.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_i1, then_label, merge_label));
                }

                self.ir_code.push_str(&format!("\n{}:\n", then_label));
                self.enter_scope();
                for s in then_branch {
                    self.compile_stmt(s);
                }
                self.exit_scope();
                self.ir_code.push_str(&format!("  br label %{}\n", merge_label));

                if let Some(else_stmts) = else_branch {
                    self.ir_code.push_str(&format!("\n{}:\n", else_label));
                    self.enter_scope();
                    for s in else_stmts {
                        self.compile_stmt(s);
                    }
                    self.exit_scope();
                    self.ir_code.push_str(&format!("  br label %{}\n", merge_label));
                }

                self.ir_code.push_str(&format!("\n{}:\n", merge_label));
            }
            Stmt::WhileStmt { condition, body, .. } => {
                let cond_label = self.new_label();
                let body_label = self.new_label();
                let end_label = self.new_label();

                self.ir_code.push_str(&format!("  br label %{}\n", cond_label));
                self.ir_code.push_str(&format!("\n{}:\n", cond_label));

                let cond_reg = self.compile_expr(condition);
                let cond_i1 = self.new_tmp();
                self.ir_code.push_str(&format!("  {} = icmp ne i64 {}, 0\n", cond_i1, cond_reg));
                self.ir_code.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_i1, body_label, end_label));

                self.ir_code.push_str(&format!("\n{}:\n", body_label));
                self.enter_scope();
                for s in body {
                    self.compile_stmt(s);
                }
                self.exit_scope();
                self.ir_code.push_str(&format!("  br label %{}\n", cond_label));

                self.ir_code.push_str(&format!("\n{}:\n", end_label));
            }
            Stmt::ServerBlock(stmts, _) => {
                for s in stmts {
                    self.compile_stmt(s);
                }
            }
            _ => {
                self.ir_code.push_str(&format!("  ; Stmt not yet converted to LLVM IR: {:?}\n", stmt));
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Ternary { condition, then_expr, else_expr, .. } => {
                let _cond = self.compile_expr(condition);
                let t = self.compile_expr(then_expr);
                let _e = self.compile_expr(else_expr);
                t // Simplified placeholder
            }
            Expr::IntLit(val, _) => {
                format!("{}", val)
            }
            Expr::FloatLit(val, _) => {
                format!("{}", val)
            }
            Expr::StringLit(val, _) => {
                let ptr = self.new_tmp();
                let size = val.len() + 9; // 8 bytes RC header + 1 null term (simplified)
                self.ir_code.push_str(&format!("  {} = call ptr @ema_malloc(i64 {})\n", ptr, size));
                self.ir_code.push_str(&format!("  store i64 1, ptr {}, align 8\n", ptr));
                ptr
            }
            Expr::Identifier(name, _) => {
                let ema_type = self.get_type(name).unwrap_or(EmaType::Int);
                let llvm_type = if self.is_heap_type(&ema_type) { "ptr" } else { "i64" };
                let tmp = self.new_tmp();
                self.ir_code.push_str(&format!("  {} = load {}, ptr %{}, align 8\n", tmp, llvm_type, name));
                tmp
            }
            Expr::Binary { left, op, right, span: _ } => {
                let left_val = self.compile_expr(left);
                let right_val = self.compile_expr(right);
                let tmp = self.new_tmp();
                
                let instruction = match op {
                    BinaryOp::Add => "add nsw",
                    BinaryOp::Sub => "sub nsw",
                    BinaryOp::Mul => "mul nsw",
                    BinaryOp::Div => "sdiv",
                    _ => "add nsw", // Only basic operations for this stage
                };
                
                self.ir_code.push_str(&format!("  {} = {} i64 {}, {}\n", tmp, instruction, left_val, right_val));
                tmp
            }
            Expr::Member { .. } => {
                "0 ; Member access not yet implemented in LLVM backend".to_string()
            }
            Expr::Await(inner, _) => {
                // In LLVM bootstrap for now, ignore the async marker and compile synchronously.
                self.compile_expr(inner)
            }
            Expr::NamespaceCall { namespace, method, args, .. } => {
                match (namespace.as_str(), method.as_str()) {
                    ("std::fs", "read") => {
                        let path = self.compile_expr(&args[0]);
                        let res = self.new_tmp();
                        self.ir_code.push_str(&format!("  {} = call ptr @ema_fs_read(ptr {})\n", res, path));
                        res
                    }
                    ("std::fs", "write") => {
                        let path = self.compile_expr(&args[0]);
                        let content = self.compile_expr(&args[1]);
                        self.ir_code.push_str(&format!("  call void @ema_fs_write(ptr {}, ptr {})\n", path, content));
                        "0".to_string()
                    }
                    _ => {
                        format!("0 ; [LLVM] Unsupported NamespaceCall: {}::{}", namespace, method)
                    }
                }
            }
            Expr::ClientScript(_, _) | Expr::ServerScript(_, _) => {
                "0 ; Scripts are not compiled to native LLVM in this version".to_string()
            }
            _ => {
                "0 ; Support for this expression in LLVM is forthcoming".to_string()
            }
        }
    }

    fn infer_type(&self, expr: &Expr) -> EmaType {
        match expr {
            Expr::IntLit(..) => EmaType::Int,
            Expr::FloatLit(..) => EmaType::Float,
            Expr::StringLit(..) => EmaType::Str,
            Expr::BoolLit(..) => EmaType::Bool,
            Expr::Identifier(name, _) => self.get_type(name).unwrap_or(EmaType::Int),
            Expr::Binary { left, .. } => self.infer_type(left), // Simplified: assume same as left
            Expr::Call { .. } => EmaType::Int,
            Expr::NamespaceCall { namespace, method, .. } => {
                if namespace == "std::fs" && method == "read" { EmaType::Str }
                else { EmaType::Int }
            }
            Expr::ClientScript(..) | Expr::ServerScript(..) => EmaType::Void,
            _ => EmaType::Void,
        }
    }
}
