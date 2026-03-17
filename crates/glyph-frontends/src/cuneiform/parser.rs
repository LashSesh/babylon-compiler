//! Cuneiform parser — recursive-descent parser producing the same AST as Sanskroot/HanLan.

use super::lexer::{KeywordKind, Token};

// ── AST types (identical to Sanskroot/HanLan) ──────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    If {
        condition: Expr,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    Return(Expr),
    Assignment {
        name: String,
        value: Expr,
    },
    ExprStmt(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Gt,
    Lt,
    GtEq,
    LtEq,
    Eq,
    NotEq,
}

#[derive(Debug, Clone)]
pub enum Expr {
    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    Literal(LitValue),
    Identifier(String),
}

#[derive(Debug, Clone)]
pub enum LitValue {
    Int(i64),
    Str(String),
    Bool(bool),
}

// ── Parser ─────────────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let tok = self.advance();
        if &tok == expected {
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", expected, tok))
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut functions = Vec::new();
        while *self.peek() != Token::Eof {
            functions.push(self.parse_function()?);
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<Function, String> {
        // Expect function keyword
        match self.advance() {
            Token::Keyword(KeywordKind::Karya) => {}
            other => return Err(format!("expected function keyword, got {:?}", other)),
        }

        // Function name
        let name = match self.advance() {
            Token::Ident(n) => n,
            other => return Err(format!("expected function name, got {:?}", other)),
        };

        // Parameters
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;

        // Body
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        self.expect(&Token::RBrace)?;

        Ok(Function { name, params, body })
    }

    fn parse_params(&mut self) -> Result<Vec<String>, String> {
        let mut params = Vec::new();
        if *self.peek() == Token::RParen {
            return Ok(params);
        }
        loop {
            match self.advance() {
                Token::Ident(name) => params.push(name),
                other => return Err(format!("expected parameter name, got {:?}", other)),
            }
            if *self.peek() == Token::Comma {
                self.advance();
            } else {
                break;
            }
        }
        Ok(params)
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, String> {
        let mut stmts = Vec::new();
        while *self.peek() != Token::RBrace && *self.peek() != Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        match self.peek().clone() {
            Token::Keyword(KeywordKind::Yadi) => self.parse_if(),
            Token::Keyword(KeywordKind::Nivrtti) => self.parse_return(),
            Token::Keyword(KeywordKind::Mana) => self.parse_assignment(),
            Token::Keyword(KeywordKind::Darshaya) => self.parse_expr_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_if(&mut self) -> Result<Statement, String> {
        self.advance(); // consume if keyword
        self.expect(&Token::LParen)?;
        let condition = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::LBrace)?;
        let then_body = self.parse_block()?;
        self.expect(&Token::RBrace)?;

        let else_body = if *self.peek() == Token::Keyword(KeywordKind::Anyatha) {
            self.advance();
            self.expect(&Token::LBrace)?;
            let body = self.parse_block()?;
            self.expect(&Token::RBrace)?;
            Some(body)
        } else {
            None
        };

        Ok(Statement::If {
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_return(&mut self) -> Result<Statement, String> {
        self.advance(); // consume return keyword
        let expr = self.parse_expr()?;
        self.expect(&Token::Semi)?;
        Ok(Statement::Return(expr))
    }

    fn parse_assignment(&mut self) -> Result<Statement, String> {
        self.advance(); // consume let keyword
        let name = match self.advance() {
            Token::Ident(n) => n,
            other => return Err(format!("expected variable name, got {:?}", other)),
        };
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        self.expect(&Token::Semi)?;
        Ok(Statement::Assignment { name, value })
    }

    fn parse_expr_stmt(&mut self) -> Result<Statement, String> {
        let expr = self.parse_expr()?;
        self.expect(&Token::Semi)?;
        Ok(Statement::ExprStmt(expr))
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_addition()?;
        loop {
            let op = match self.peek() {
                Token::Gt => BinOp::Gt,
                Token::Lt => BinOp::Lt,
                Token::GtEq => BinOp::GtEq,
                Token::LtEq => BinOp::LtEq,
                Token::EqEq => BinOp::Eq,
                Token::BangEq => BinOp::NotEq,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_addition(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_term()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_factor()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_factor()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::IntLit(n) => {
                self.advance();
                Ok(Expr::Literal(LitValue::Int(n)))
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(Expr::Literal(LitValue::Str(s)))
            }
            Token::BoolLit(b) => {
                self.advance();
                Ok(Expr::Literal(LitValue::Bool(b)))
            }
            Token::Keyword(KeywordKind::Darshaya) => {
                self.advance();
                self.expect(&Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(&Token::RParen)?;
                Ok(Expr::Call {
                    callee: "print".to_string(),
                    args,
                })
            }
            Token::Ident(name) => {
                self.advance();
                if *self.peek() == Token::LParen {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(&Token::RParen)?;
                    Ok(Expr::Call { callee: name, args })
                } else {
                    Ok(Expr::Identifier(name))
                }
            }
            other => Err(format!("unexpected token in expression: {:?}", other)),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if *self.peek() == Token::RParen {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if *self.peek() == Token::Comma {
                self.advance();
            } else {
                break;
            }
        }
        Ok(args)
    }
}

/// Parse a token slice into a cuneiform Program (matches Sanskroot/HanLan API).
pub fn parse(tokens: &[Token]) -> Result<Program, String> {
    let mut parser = Parser::new(tokens.to_vec());
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cuneiform::lexer::lex;

    #[test]
    fn test_minimal_print() {
        let tokens = lex("𒀀 main() { 𒅎(\"hello\"); }").unwrap();
        let mut parser = Parser::new(tokens);
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.functions.len(), 1);
        assert_eq!(prog.functions[0].name, "main");
        assert_eq!(prog.functions[0].body.len(), 1);
    }

    #[test]
    fn test_conditional() {
        let tokens = lex("𒀀 check(x) { 𒅗 (x > 0) { 𒅎(\"pos\"); } 𒀸 { 𒅎(\"neg\"); } }").unwrap();
        let mut parser = Parser::new(tokens);
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.functions.len(), 1);
        match &prog.functions[0].body[0] {
            Statement::If { else_body, .. } => assert!(else_body.is_some()),
            _ => panic!("expected if statement"),
        }
    }

    #[test]
    fn test_multi_function() {
        let src = "𒀀 add(x, y) { 𒀭 x + y; } 𒀀 main() { 𒃻 result = add(3, 4); 𒅎(result); }";
        let tokens = lex(src).unwrap();
        let mut parser = Parser::new(tokens);
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.functions.len(), 2);
        assert_eq!(prog.functions[0].name, "add");
        assert_eq!(prog.functions[0].params.len(), 2);
    }

    #[test]
    fn test_transliteration_parses_identically() {
        let cuneiform_tokens = lex("𒀀 main() { 𒅎(\"hi\"); }").unwrap();
        let translit_tokens = lex("a main() { ta(\"hi\"); }").unwrap();
        let mut p1 = Parser::new(cuneiform_tokens);
        let mut p2 = Parser::new(translit_tokens);
        let prog1 = p1.parse_program().unwrap();
        let prog2 = p2.parse_program().unwrap();
        assert_eq!(prog1.functions.len(), prog2.functions.len());
        assert_eq!(prog1.functions[0].name, prog2.functions[0].name);
    }
}
