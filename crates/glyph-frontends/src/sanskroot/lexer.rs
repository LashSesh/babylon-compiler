//! Lexer for the Sanskroot language.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordKind {
    Karya,    // कार्य — function
    Yadi,     // यदि — if
    Anyatha,  // अन्यथा — else
    Darshaya, // दर्शय — print
    Nivrtti,  // निवृत्ति — return
    Mana,     // मान — let/assignment
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Keyword(KeywordKind),
    Identifier(String),
    IntLit(i64),
    StringLit(String),
    BoolLit(bool),
    Plus,
    Minus,
    Star,
    Slash,
    Gt,
    Lt,
    GtEq,
    LtEq,
    EqEq,
    NotEq,
    Eq,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Eof,
}

#[derive(Debug, Error)]
pub enum LexError {
    #[error("unexpected character '{0}' at position {1}")]
    UnexpectedChar(char, usize),
    #[error("unterminated string literal at position {0}")]
    UnterminatedString(usize),
    #[error("invalid integer literal at position {0}")]
    InvalidInt(usize),
}

fn is_ident_char(c: char) -> bool {
    // Devanagari range U+0900–U+097F, ASCII alphanumeric, underscore
    matches!(c, '\u{0900}'..='\u{097F}') || c.is_ascii_alphanumeric() || c == '_'
}

fn is_ident_start(c: char) -> bool {
    matches!(c, '\u{0900}'..='\u{097F}') || c.is_ascii_alphabetic() || c == '_'
}

fn keyword_for(word: &str) -> Option<Token> {
    match word {
        "कार्य" => Some(Token::Keyword(KeywordKind::Karya)),
        "यदि" => Some(Token::Keyword(KeywordKind::Yadi)),
        "अन्यथा" => Some(Token::Keyword(KeywordKind::Anyatha)),
        "दर्शय" => Some(Token::Keyword(KeywordKind::Darshaya)),
        "निवृत्ति" => Some(Token::Keyword(KeywordKind::Nivrtti)),
        "मान" => Some(Token::Keyword(KeywordKind::Mana)),
        "सत्य" => Some(Token::BoolLit(true)),
        "असत्य" => Some(Token::BoolLit(false)),
        _ => None,
    }
}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // String literals
        if c == '"' {
            i += 1;
            let start = i;
            let mut s = String::new();
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    i += 1;
                    match chars[i] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        other => {
                            s.push('\\');
                            s.push(other);
                        }
                    }
                } else {
                    s.push(chars[i]);
                }
                i += 1;
            }
            if i >= len {
                return Err(LexError::UnterminatedString(start));
            }
            i += 1; // skip closing quote
            tokens.push(Token::StringLit(s));
            continue;
        }

        // Integer literals
        if c.is_ascii_digit() {
            let start = i;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            let val = num_str
                .parse::<i64>()
                .map_err(|_| LexError::InvalidInt(start))?;
            tokens.push(Token::IntLit(val));
            continue;
        }

        // Identifiers and keywords
        if is_ident_start(c) {
            let start = i;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if let Some(tok) = keyword_for(&word) {
                tokens.push(tok);
            } else {
                tokens.push(Token::Identifier(word));
            }
            continue;
        }

        // Two-char operators
        if i + 1 < len {
            let next = chars[i + 1];
            match (c, next) {
                ('>', '=') => {
                    tokens.push(Token::GtEq);
                    i += 2;
                    continue;
                }
                ('<', '=') => {
                    tokens.push(Token::LtEq);
                    i += 2;
                    continue;
                }
                ('=', '=') => {
                    tokens.push(Token::EqEq);
                    i += 2;
                    continue;
                }
                ('!', '=') => {
                    tokens.push(Token::NotEq);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        // Single-char tokens
        let tok = match c {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '>' => Token::Gt,
            '<' => Token::Lt,
            '=' => Token::Eq,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            _ => return Err(LexError::UnexpectedChar(c, i)),
        };
        tokens.push(tok);
        i += 1;
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_minimal_print() {
        let src = "कार्य मुख्य() {\n    दर्शय(\"नमस्ते\");\n}";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Karya));
        assert_eq!(tokens[1], Token::Identifier("मुख्य".to_string()));
        assert_eq!(tokens[2], Token::LParen);
        assert_eq!(tokens[3], Token::RParen);
        assert_eq!(tokens[4], Token::LBrace);
        assert_eq!(tokens[5], Token::Keyword(KeywordKind::Darshaya));
        assert_eq!(tokens[6], Token::LParen);
        assert_eq!(tokens[7], Token::StringLit("नमस्ते".to_string()));
        assert_eq!(tokens[8], Token::RParen);
        assert_eq!(tokens[9], Token::Semicolon);
        assert_eq!(tokens[10], Token::RBrace);
        assert_eq!(tokens[11], Token::Eof);
    }

    #[test]
    fn test_lex_boolean_literals() {
        let tokens = lex("सत्य असत्य").unwrap();
        assert_eq!(tokens[0], Token::BoolLit(true));
        assert_eq!(tokens[1], Token::BoolLit(false));
    }

    #[test]
    fn test_lex_operators() {
        let tokens = lex(">= <= == != > < + - * /").unwrap();
        assert_eq!(tokens[0], Token::GtEq);
        assert_eq!(tokens[1], Token::LtEq);
        assert_eq!(tokens[2], Token::EqEq);
        assert_eq!(tokens[3], Token::NotEq);
        assert_eq!(tokens[4], Token::Gt);
        assert_eq!(tokens[5], Token::Lt);
        assert_eq!(tokens[6], Token::Plus);
        assert_eq!(tokens[7], Token::Minus);
        assert_eq!(tokens[8], Token::Star);
        assert_eq!(tokens[9], Token::Slash);
    }
}
