use crate::ast::{Program, Stmt, Expr, BinaryOp};

pub struct LlvmCompiler {
    ir_code: String,
    tmp_counter: usize,
}

impl LlvmCompiler {
    pub fn new() -> Self {
        let mut ir = String::new();
        // LLVM Module Target definitions (x86_64 default)
        ir.push_str("; ModuleID = 'ema_master_module'\n");
        ir.push_str("source_filename = \"ema_program.ema\"\n");
        ir.push_str("target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128\"\n");
        ir.push_str("target triple = \"x86_64-pc-windows-msvc\"\n\n");
        // Printf declaration for stdlib integration
        ir.push_str("declare i32 @printf(ptr, ...)\n");
        ir.push_str("@.str.int = private unnamed_addr constant [4 x i8] c\"%d\\0A\\00\", align 1\n");
        ir.push_str("@.str.string = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n\n");
        
        LlvmCompiler {
            ir_code: ir,
            tmp_counter: 1,
        }
    }

    fn new_tmp(&mut self) -> String {
        let name = format!("%{}", self.tmp_counter);
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
            Stmt::VarDecl { name, value, span: _ } => {
                // For LLVM, we allocate stack space then store the evaluated value
                self.ir_code.push_str(&format!("  %{} = alloca i64, align 8\n", name));
                let val_reg = self.compile_expr(value);
                self.ir_code.push_str(&format!("  store i64 {}, ptr %{}, align 8\n", val_reg, name));
            }
            Stmt::PrintStmt(expr, _) => {
                let val_reg = self.compile_expr(expr);
                // Hardcoded to integer print since strict typing isn't resolved deeply yet in this bootstrap
                let tmp = self.new_tmp();
                self.ir_code.push_str(&format!("  {} = call i32 (ptr, ...) @printf(ptr @.str.int, i64 {})\n", tmp, val_reg));
            }
            Stmt::ExprStmt(expr, _) => {
                self.compile_expr(expr);
            }
            _ => {
                self.ir_code.push_str(&format!("  ; Henuz LLVM IR'a cevrilmeyen Stmt: {:?}\n", stmt));
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::IntLit(val, _) => {
                format!("{}", val)
            }
            Expr::Identifier(name, _) => {
                let tmp = self.new_tmp();
                self.ir_code.push_str(&format!("  {} = load i64, ptr %{}, align 8\n", tmp, name));
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
                    _ => "add nsw", // Sadece temel islemler bu asama icin
                };
                
                self.ir_code.push_str(&format!("  {} = {} i64 {}, {}\n", tmp, instruction, left_val, right_val));
                tmp
            }
            _ => {
                "0 ; Unsupported Expr".to_string()
            }
        }
    }
}
