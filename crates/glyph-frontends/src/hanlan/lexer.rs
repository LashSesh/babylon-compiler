//! Lexer for the HanLan (CN++) language.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordKind {
    Han,      // 函 — function
    Ruo,      // 若 — if
    Fouze,    // 否则 — else
    Fan,      // 返 — return
    Ling,     // 令 — let/assignment
    Shi,      // 示 — print
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
    // CJK Unified Ideographs U+4E00–U+9FFF
    // Devanagari range U+0900–U+097F (Tier-1 alias support)
    // ASCII alphanumeric, underscore
    matches!(c, '\u{4E00}'..='\u{9FFF}')
        || matches!(c, '\u{0900}'..='\u{097F}')
        || c.is_ascii_alphanumeric()
        || c == '_'
}

fn is_ident_start(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}')
        || matches!(c, '\u{0900}'..='\u{097F}')
        || c.is_ascii_alphabetic()
        || c == '_'
}

fn keyword_for(word: &str) -> Option<Token> {
    match word {
        // Tier-0: HanLan (CN++) keywords
        "函" => Some(Token::Keyword(KeywordKind::Han)),
        "若" => Some(Token::Keyword(KeywordKind::Ruo)),
        "否则" => Some(Token::Keyword(KeywordKind::Fouze)),
        "返" => Some(Token::Keyword(KeywordKind::Fan)),
        "令" => Some(Token::Keyword(KeywordKind::Ling)),
        "示" => Some(Token::Keyword(KeywordKind::Shi)),
        "真" => Some(Token::BoolLit(true)),
        "假" => Some(Token::BoolLit(false)),
        // Tier-1: Sanskroot aliases for mixed-script compatibility
        "कार्य" => Some(Token::Keyword(KeywordKind::Han)),
        "यदि" => Some(Token::Keyword(KeywordKind::Ruo)),
        "अन्यथा" => Some(Token::Keyword(KeywordKind::Fouze)),
        "दर्शय" => Some(Token::Keyword(KeywordKind::Shi)),
        "निवृत्ति" => Some(Token::Keyword(KeywordKind::Fan)),
        "मान" => Some(Token::Keyword(KeywordKind::Ling)),
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

        // Line comments
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
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
    fn test_lex_tv1_minimal_print() {
        let src = "函 主() {\n    示(\"你好\");\n}";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Han));
        assert_eq!(tokens[1], Token::Identifier("主".to_string()));
        assert_eq!(tokens[2], Token::LParen);
        assert_eq!(tokens[3], Token::RParen);
        assert_eq!(tokens[4], Token::LBrace);
        assert_eq!(tokens[5], Token::Keyword(KeywordKind::Shi));
        assert_eq!(tokens[6], Token::LParen);
        assert_eq!(tokens[7], Token::StringLit("你好".to_string()));
        assert_eq!(tokens[8], Token::RParen);
        assert_eq!(tokens[9], Token::Semicolon);
        assert_eq!(tokens[10], Token::RBrace);
        assert_eq!(tokens[11], Token::Eof);
    }

    #[test]
    fn test_lex_boolean_literals() {
        let tokens = lex("真 假").unwrap();
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

    #[test]
    fn test_lex_tier1_sanskroot_aliases() {
        let src = "कार्य यदि अन्यथा दर्शय निवृत्ति मान सत्य असत्य";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Han));   // कार्य → 函
        assert_eq!(tokens[1], Token::Keyword(KeywordKind::Ruo));   // यदि → 若
        assert_eq!(tokens[2], Token::Keyword(KeywordKind::Fouze)); // अन्यथा → 否则
        assert_eq!(tokens[3], Token::Keyword(KeywordKind::Shi));   // दर्शय → 示
        assert_eq!(tokens[4], Token::Keyword(KeywordKind::Fan));   // निवृत्ति → 返
        assert_eq!(tokens[5], Token::Keyword(KeywordKind::Ling));  // मान → 令
        assert_eq!(tokens[6], Token::BoolLit(true));               // सत्य → 真
        assert_eq!(tokens[7], Token::BoolLit(false));              // असत्य → 假
    }

    #[test]
    fn test_lex_line_comments() {
        let src = "// this is a comment\n函 主() {}";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Han));
        assert_eq!(tokens[1], Token::Identifier("主".to_string()));
    }

    #[test]
    fn test_lex_cjk_identifiers() {
        let tokens = lex("检 和 主").unwrap();
        assert_eq!(tokens[0], Token::Identifier("检".to_string()));
        assert_eq!(tokens[1], Token::Identifier("和".to_string()));
        assert_eq!(tokens[2], Token::Identifier("主".to_string()));
    }
}
