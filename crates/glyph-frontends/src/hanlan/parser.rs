//! Parser for the HanLan (CN++) language.

use super::lexer::{KeywordKind, Token};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LitValue {
    Int(i64),
    String(String),
    Bool(bool),
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token {0:?} at position {1}")]
    UnexpectedToken(Token, usize),
    #[error("expected {0} but found {1:?} at position {2}")]
    Expected(String, Token, usize),
    #[error("unexpected end of input")]
    UnexpectedEof,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
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

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        let tok = self.advance();
        if &tok == expected {
            Ok(())
        } else {
            Err(ParseError::Expected(
                format!("{:?}", expected),
                tok,
                self.pos - 1,
            ))
        }
    }

    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut functions = Vec::new();
        while *self.peek() != Token::Eof {
            functions.push(self.parse_function()?);
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<Function, ParseError> {
        self.expect(&Token::Keyword(KeywordKind::Han))?;
        let name = match self.advance() {
            Token::Identifier(n) => n,
            tok => return Err(ParseError::Expected("identifier".into(), tok, self.pos - 1)),
        };
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        self.expect(&Token::RBrace)?;
        Ok(Function { name, params, body })
    }

    fn parse_params(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();
        if let Token::Identifier(_) = self.peek() {
            if let Token::Identifier(n) = self.advance() {
                params.push(n);
            }
            while *self.peek() == Token::Comma {
                self.advance();
                match self.advance() {
                    Token::Identifier(n) => params.push(n),
                    tok => {
                        return Err(ParseError::Expected(
                            "parameter name".into(),
                            tok,
                            self.pos - 1,
                        ))
                    }
                }
            }
        }
        Ok(params)
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, ParseError> {
        let mut stmts = Vec::new();
        while *self.peek() != Token::RBrace && *self.peek() != Token::Eof {
            stmts.push(self.parse_statement()?);
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek().clone() {
            Token::Keyword(KeywordKind::Ruo) => self.parse_if(),
            Token::Keyword(KeywordKind::Fan) => self.parse_return(),
            Token::Keyword(KeywordKind::Ling) => self.parse_assignment(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_if(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 若
        self.expect(&Token::LParen)?;
        let condition = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::LBrace)?;
        let then_body = self.parse_block()?;
        self.expect(&Token::RBrace)?;
        let else_body = if *self.peek() == Token::Keyword(KeywordKind::Fouze) {
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

    fn parse_return(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 返
        let expr = self.parse_expr()?;
        self.expect(&Token::Semicolon)?;
        Ok(Statement::Return(expr))
    }

    fn parse_assignment(&mut self) -> Result<Statement, ParseError> {
        self.advance(); // consume 令
        let name = match self.advance() {
            Token::Identifier(n) => n,
            tok => return Err(ParseError::Expected("identifier".into(), tok, self.pos - 1)),
        };
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        self.expect(&Token::Semicolon)?;
        Ok(Statement::Assignment { name, value })
    }

    fn parse_expr_stmt(&mut self) -> Result<Statement, ParseError> {
        let expr = self.parse_expr()?;
        self.expect(&Token::Semicolon)?;
        Ok(Statement::ExprStmt(expr))
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_addition()?;
        let op = match self.peek() {
            Token::Gt => Some(BinOp::Gt),
            Token::Lt => Some(BinOp::Lt),
            Token::GtEq => Some(BinOp::GtEq),
            Token::LtEq => Some(BinOp::LtEq),
            Token::EqEq => Some(BinOp::Eq),
            Token::NotEq => Some(BinOp::NotEq),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let right = self.parse_addition()?;
            Ok(Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }

    fn parse_addition(&mut self) -> Result<Expr, ParseError> {
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

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
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

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::IntLit(v) => {
                self.advance();
                Ok(Expr::Literal(LitValue::Int(v)))
            }
            Token::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::Literal(LitValue::String(s)))
            }
            Token::BoolLit(b) => {
                self.advance();
                Ok(Expr::Literal(LitValue::Bool(b)))
            }
            Token::Keyword(KeywordKind::Shi) => {
                self.advance();
                self.expect(&Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(&Token::RParen)?;
                Ok(Expr::Call {
                    callee: "print".to_string(),
                    args,
                })
            }
            Token::Identifier(_) => {
                let name = match self.advance() {
                    Token::Identifier(n) => n,
                    _ => unreachable!(),
                };
                // Check if this is a function call
                if *self.peek() == Token::LParen {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(&Token::RParen)?;
                    Ok(Expr::Call {
                        callee: name,
                        args,
                    })
                } else {
                    Ok(Expr::Identifier(name))
                }
            }
            tok => Err(ParseError::UnexpectedToken(tok, self.pos)),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if *self.peek() == Token::RParen {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while *self.peek() == Token::Comma {
            self.advance();
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }
}

pub fn parse(tokens: &[Token]) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens.to_vec());
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hanlan::lexer::lex;

    #[test]
    fn test_parse_tv1_minimal_print() {
        let src = "函 主() {\n    示(\"你好\");\n}";
        let tokens = lex(src).unwrap();
        let program = parse(&tokens).unwrap();
        assert_eq!(program.functions.len(), 1);
        let f = &program.functions[0];
        assert_eq!(f.name, "主");
        assert!(f.params.is_empty());
        assert_eq!(f.body.len(), 1);
        match &f.body[0] {
            Statement::ExprStmt(Expr::Call { callee, args }) => {
                assert_eq!(callee, "print");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], Expr::Literal(LitValue::String("你好".into())));
            }
            _ => panic!("expected ExprStmt(Call)"),
        }
    }

    #[test]
    fn test_parse_tv2_conditional() {
        let src = "函 检(x) {\n    若 (x > 0) {\n        示(\"正\");\n    } 否则 {\n        示(\"非正\");\n    }\n}";
        let tokens = lex(src).unwrap();
        let program = parse(&tokens).unwrap();
        assert_eq!(program.functions.len(), 1);
        let f = &program.functions[0];
        assert_eq!(f.name, "检");
        assert_eq!(f.params, vec!["x"]);
        assert_eq!(f.body.len(), 1);
        match &f.body[0] {
            Statement::If {
                then_body,
                else_body,
                ..
            } => {
                assert_eq!(then_body.len(), 1);
                assert!(else_body.is_some());
                assert_eq!(else_body.as_ref().unwrap().len(), 1);
            }
            _ => panic!("expected If statement"),
        }
    }

    #[test]
    fn test_parse_tv3_multi_function() {
        let src = "函 和(a, b) {\n    返 a + b;\n}\n函 主() {\n    令 r = 和(3, 4);\n    示(r);\n}";
        let tokens = lex(src).unwrap();
        let program = parse(&tokens).unwrap();
        assert_eq!(program.functions.len(), 2);
        assert_eq!(program.functions[0].name, "和");
        assert_eq!(program.functions[1].name, "主");
    }
}
