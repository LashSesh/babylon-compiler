//! Lowering pass from HanLan (CN++) AST to glyph IR.

use glyph_canon::content_hash_node_id;
use glyph_ir::{EdgeKind, IrDocument, IrEdge, IrNode, LiteralValue, NodeKind};

use super::parser::{BinOp, Expr, Function, LitValue, Program, Statement};

/// Lower a parsed HanLan program into an IR document.
pub fn lower(program: &Program, source_digest: &str) -> IrDocument {
    let mut doc = IrDocument::new("hanlan", source_digest);
    let mut ctx = LowerCtx::new(&mut doc);

    let module_id = content_hash_node_id("Module", "root", "/");
    ctx.add_node(IrNode::new(&module_id, NodeKind::Module, "root"));

    for (i, func) in program.functions.iter().enumerate() {
        let func_id = ctx.lower_function(func, &module_id);
        ctx.add_edge(IrEdge::new(&module_id, &func_id, EdgeKind::Contains).with_ordinal(i as i64));
        if i > 0 {
            let prev_func_id = ctx.function_ids[i - 1].clone();
            ctx.add_edge(IrEdge::new(&prev_func_id, &func_id, EdgeKind::Next));
        }
    }

    doc.canonicalize();
    doc
}

struct LowerCtx<'a> {
    doc: &'a mut IrDocument,
    function_ids: Vec<String>,
    counter: usize,
}

impl<'a> LowerCtx<'a> {
    fn new(doc: &'a mut IrDocument) -> Self {
        LowerCtx {
            doc,
            function_ids: Vec::new(),
            counter: 0,
        }
    }

    fn add_node(&mut self, node: IrNode) {
        self.doc.nodes.push(node);
    }

    fn add_edge(&mut self, edge: IrEdge) {
        self.doc.edges.push(edge);
    }

    fn next_id(&mut self) -> usize {
        let id = self.counter;
        self.counter += 1;
        id
    }

    fn lower_function(&mut self, func: &Function, _parent_id: &str) -> String {
        let scope_path = format!("/{}", func.name);
        let func_id = content_hash_node_id("Function", &func.name, "/");

        let mut func_node = IrNode::new(&func_id, NodeKind::Function, &func.name);
        if !func.params.is_empty() {
            func_node.params = Some(func.params.clone());
        }
        self.add_node(func_node);
        self.function_ids.push(func_id.clone());

        // Create block node for function body
        let block_name = format!("block{}", self.next_id());
        let block_scope = format!("{}/{}", scope_path, block_name);
        let block_id = content_hash_node_id("Block", &block_name, &scope_path);
        self.add_node(IrNode::new(&block_id, NodeKind::Block, &block_name));
        self.add_edge(IrEdge::new(&func_id, &block_id, EdgeKind::Contains));

        self.lower_statements(&func.body, &block_id, &block_scope);

        func_id
    }

    fn lower_statements(&mut self, stmts: &[Statement], parent_id: &str, scope_path: &str) {
        let mut prev_id: Option<String> = None;
        for (i, stmt) in stmts.iter().enumerate() {
            let stmt_id = self.lower_statement(stmt, parent_id, scope_path, i);
            self.add_edge(
                IrEdge::new(parent_id, &stmt_id, EdgeKind::Contains).with_ordinal(i as i64),
            );
            if let Some(prev) = &prev_id {
                self.add_edge(IrEdge::new(prev, &stmt_id, EdgeKind::Next));
            }
            prev_id = Some(stmt_id);
        }
    }

    fn lower_statement(
        &mut self,
        stmt: &Statement,
        _parent_id: &str,
        scope_path: &str,
        _index: usize,
    ) -> String {
        match stmt {
            Statement::If {
                condition,
                then_body,
                else_body,
            } => {
                let if_name = format!("if{}", self.next_id());
                let if_id = content_hash_node_id("If", &if_name, scope_path);
                self.add_node(IrNode::new(&if_id, NodeKind::If, &if_name));

                // Condition
                let cond_id = self.lower_expr(condition, scope_path);
                self.add_edge(IrEdge::new(&if_id, &cond_id, EdgeKind::Condition));

                // Then branch
                let then_name = format!("then{}", self.next_id());
                let then_scope = format!("{}/{}", scope_path, then_name);
                let then_id = content_hash_node_id("Block", &then_name, scope_path);
                self.add_node(IrNode::new(&then_id, NodeKind::Block, &then_name));
                self.add_edge(IrEdge::new(&if_id, &then_id, EdgeKind::ThenBranch));
                self.lower_statements(then_body, &then_id, &then_scope);

                // Else branch
                if let Some(else_stmts) = else_body {
                    let else_name = format!("else{}", self.next_id());
                    let else_scope = format!("{}/{}", scope_path, else_name);
                    let else_id = content_hash_node_id("Block", &else_name, scope_path);
                    self.add_node(IrNode::new(&else_id, NodeKind::Block, &else_name));
                    self.add_edge(IrEdge::new(&if_id, &else_id, EdgeKind::ElseBranch));
                    self.lower_statements(else_stmts, &else_id, &else_scope);
                }

                if_id
            }
            Statement::Return(expr) => {
                let ret_name = format!("ret{}", self.next_id());
                let ret_id = content_hash_node_id("Return", &ret_name, scope_path);
                self.add_node(IrNode::new(&ret_id, NodeKind::Return, &ret_name));

                let val_id = self.lower_expr(expr, scope_path);
                self.add_edge(IrEdge::new(&ret_id, &val_id, EdgeKind::Value));

                ret_id
            }
            Statement::Assignment { name, value } => {
                let assign_name = format!("assign{}_{}", self.next_id(), name);
                let assign_id =
                    content_hash_node_id("Assignment", &assign_name, scope_path);
                let mut node = IrNode::new(&assign_id, NodeKind::Assignment, &assign_name);
                // Store the target variable name
                node.name = name.clone();
                self.add_node(node);

                let val_id = self.lower_expr(value, scope_path);
                self.add_edge(IrEdge::new(&assign_id, &val_id, EdgeKind::Value));

                // Create an identifier node for the assignment target
                let target_id = content_hash_node_id(
                    "Identifier",
                    name,
                    &format!("{}/target", scope_path),
                );
                let target_node =
                    IrNode::new(&target_id, NodeKind::Identifier, name);
                self.add_node(target_node);
                self.add_edge(IrEdge::new(&assign_id, &target_id, EdgeKind::Target));

                assign_id
            }
            Statement::ExprStmt(expr) => self.lower_expr(expr, scope_path),
        }
    }

    fn lower_expr(&mut self, expr: &Expr, scope_path: &str) -> String {
        match expr {
            Expr::BinaryOp { op, left, right } => {
                let op_str = match op {
                    BinOp::Add => "add",
                    BinOp::Sub => "sub",
                    BinOp::Mul => "mul",
                    BinOp::Div => "div",
                    BinOp::Gt => "gt",
                    BinOp::Lt => "lt",
                    BinOp::GtEq => "gte",
                    BinOp::LtEq => "lte",
                    BinOp::Eq => "eq",
                    BinOp::NotEq => "neq",
                };
                let binop_name = format!("binop{}_{}", self.next_id(), op_str);
                let binop_id = content_hash_node_id("BinaryOp", &binop_name, scope_path);
                let mut node = IrNode::new(&binop_id, NodeKind::BinaryOp, &binop_name);
                node.op = Some(op_str.to_string());
                self.add_node(node);

                let left_id = self.lower_expr(left, scope_path);
                let right_id = self.lower_expr(right, scope_path);
                self.add_edge(IrEdge::new(&binop_id, &left_id, EdgeKind::Left));
                self.add_edge(IrEdge::new(&binop_id, &right_id, EdgeKind::Right));

                binop_id
            }
            Expr::Call { callee, args } => {
                let call_name = format!("call{}_{}", self.next_id(), callee);
                let call_id = content_hash_node_id("Call", &call_name, scope_path);
                let mut node = IrNode::new(&call_id, NodeKind::Call, &call_name);
                node.callee = Some(callee.clone());
                self.add_node(node);

                // Callee ref: create an identifier node for the callee
                let callee_ident_id = content_hash_node_id(
                    "Identifier",
                    callee,
                    &format!("{}/callee", scope_path),
                );
                self.add_node(IrNode::new(
                    &callee_ident_id,
                    NodeKind::Identifier,
                    callee,
                ));
                self.add_edge(IrEdge::new(&call_id, &callee_ident_id, EdgeKind::CalleeRef));

                for (i, arg) in args.iter().enumerate() {
                    let arg_id = self.lower_expr(arg, scope_path);
                    self.add_edge(
                        IrEdge::new(&call_id, &arg_id, EdgeKind::Argument)
                            .with_ordinal(i as i64),
                    );
                }

                call_id
            }
            Expr::Literal(lit) => {
                let (val, lit_type, name) = match lit {
                    LitValue::Int(v) => (
                        LiteralValue::Int(*v),
                        "int".to_string(),
                        v.to_string(),
                    ),
                    LitValue::String(s) => (
                        LiteralValue::String(s.clone()),
                        "string".to_string(),
                        s.clone(),
                    ),
                    LitValue::Bool(b) => (
                        LiteralValue::Bool(*b),
                        "bool".to_string(),
                        b.to_string(),
                    ),
                };
                let lit_unique = format!("lit{}_{}", self.next_id(), name);
                let lit_id = content_hash_node_id("Literal", &lit_unique, scope_path);
                let mut node = IrNode::new(&lit_id, NodeKind::Literal, &lit_unique);
                node.value = Some(val);
                node.literal_type = Some(lit_type);
                self.add_node(node);
                lit_id
            }
            Expr::Identifier(name) => {
                let ident_unique = format!("ident{}_{}", self.next_id(), name);
                let ident_id = content_hash_node_id("Identifier", &ident_unique, scope_path);
                self.add_node(IrNode::new(&ident_id, NodeKind::Identifier, name));
                ident_id
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hanlan::lexer::lex;
    use crate::hanlan::parser::parse;

    fn lower_source(src: &str) -> IrDocument {
        let tokens = lex(src).unwrap();
        let program = parse(&tokens).unwrap();
        let digest = glyph_canon::digest_bytes(src.as_bytes());
        lower(&program, &digest)
    }

    #[test]
    fn test_tv1_minimal_print() {
        let src = "函 主() {\n    示(\"你好\");\n}";
        let doc = lower_source(src);

        let module_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Module).collect();
        assert_eq!(module_nodes.len(), 1);

        let func_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
        assert_eq!(func_nodes.len(), 1);
        assert_eq!(func_nodes[0].name, "主");

        let call_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Call).collect();
        assert_eq!(call_nodes.len(), 1);
        assert_eq!(call_nodes[0].callee, Some("print".to_string()));

        let lit_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Literal).collect();
        assert_eq!(lit_nodes.len(), 1);
        assert_eq!(lit_nodes[0].value, Some(LiteralValue::String("你好".into())));

        let contains_edges: Vec<_> = doc.edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
        assert!(contains_edges.len() >= 2);

        let callee_edges: Vec<_> = doc.edges.iter().filter(|e| e.kind == EdgeKind::CalleeRef).collect();
        assert_eq!(callee_edges.len(), 1);

        assert_eq!(doc.source_language, "hanlan");
    }

    #[test]
    fn test_tv2_conditional() {
        let src = "函 检(x) {\n    若 (x > 0) {\n        示(\"正\");\n    } 否则 {\n        示(\"非正\");\n    }\n}";
        let doc = lower_source(src);

        let if_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::If).collect();
        assert_eq!(if_nodes.len(), 1);

        let then_edges: Vec<_> = doc.edges.iter().filter(|e| e.kind == EdgeKind::ThenBranch).collect();
        assert_eq!(then_edges.len(), 1);
        let else_edges: Vec<_> = doc.edges.iter().filter(|e| e.kind == EdgeKind::ElseBranch).collect();
        assert_eq!(else_edges.len(), 1);

        let cond_edges: Vec<_> = doc.edges.iter().filter(|e| e.kind == EdgeKind::Condition).collect();
        assert_eq!(cond_edges.len(), 1);

        let binop_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::BinaryOp).collect();
        assert_eq!(binop_nodes.len(), 1);
        assert_eq!(binop_nodes[0].op, Some("gt".into()));
    }

    #[test]
    fn test_tv3_multi_function() {
        let src = "函 和(a, b) {\n    返 a + b;\n}\n函 主() {\n    令 r = 和(3, 4);\n    示(r);\n}";
        let doc = lower_source(src);

        let func_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Function).collect();
        assert_eq!(func_nodes.len(), 2);

        let ret_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Return).collect();
        assert_eq!(ret_nodes.len(), 1);

        let assign_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Assignment).collect();
        assert_eq!(assign_nodes.len(), 1);

        let next_edges: Vec<_> = doc.edges.iter().filter(|e| e.kind == EdgeKind::Next).collect();
        assert!(next_edges.len() >= 1);

        let call_nodes: Vec<_> = doc.nodes.iter().filter(|n| n.kind == NodeKind::Call).collect();
        assert_eq!(call_nodes.len(), 2);

        assert_eq!(doc.source_language, "hanlan");
    }

    #[test]
    fn test_lower_deterministic() {
        let src = "函 主() {\n    示(\"你好\");\n}";
        let doc1 = lower_source(src);
        let doc2 = lower_source(src);
        assert_eq!(doc1.digest(), doc2.digest());
    }
}
