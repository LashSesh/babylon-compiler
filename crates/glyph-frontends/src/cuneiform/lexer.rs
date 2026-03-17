//! Cuneiform lexer — tokenizes COL source using cuneiform Unicode signs (U+12000–U+1254F)
//! and ASCII transliteration aliases.

use std::fmt;

/// Cuneiform keyword kinds mapped from cuneiform signs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordKind {
    /// 𒀀 (U+12000, A) — function definition
    Karya,
    /// 𒅗 (U+12157, TUKUL) — if
    Yadi,
    /// 𒀸 (U+12038, BAL) — else
    Anyatha,
    /// 𒀭 (U+1202D, AŠ) — return
    Nivrtti,
    /// 𒃻 (U+120FB, NAM) — let/assignment
    Mana,
    /// 𒅎 (U+1214E, TA) — print
    Darshaya,
}

impl fmt::Display for KeywordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeywordKind::Karya => write!(f, "𒀀"),
            KeywordKind::Yadi => write!(f, "𒅗"),
            KeywordKind::Anyatha => write!(f, "𒀸"),
            KeywordKind::Nivrtti => write!(f, "𒀭"),
            KeywordKind::Mana => write!(f, "𒃻"),
            KeywordKind::Darshaya => write!(f, "𒅎"),
        }
    }
}

/// Token types produced by the cuneiform lexer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Keyword(KeywordKind),
    Ident(String),
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
    BangEq,
    Eq,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Eof,
}

/// Lexer errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    UnexpectedChar(char),
    UnterminatedString,
    InvalidInt(String),
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::UnexpectedChar(c) => write!(f, "unexpected character: '{}'", c),
            LexError::UnterminatedString => write!(f, "unterminated string literal"),
            LexError::InvalidInt(s) => write!(f, "invalid integer: {}", s),
        }
    }
}

/// Check if a character can start an identifier.
/// Supports cuneiform (U+12000–U+1254F), ASCII alpha, and underscore.
fn is_ident_start(c: char) -> bool {
    let cp = c as u32;
    c.is_ascii_alphabetic()
        || c == '_'
        || (0x12000..=0x1254F).contains(&cp)
}

/// Check if a character can continue an identifier.
fn is_ident_char(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit()
}

/// Map a string (cuneiform sign or transliteration) to a keyword or boolean literal.
fn keyword_for(s: &str) -> Option<Token> {
    match s {
        // Native cuneiform signs
        "𒀀" => Some(Token::Keyword(KeywordKind::Karya)),      // U+12000 A — fn
        "𒅗" => Some(Token::Keyword(KeywordKind::Yadi)),       // U+12157 TUKUL — if
        "𒀸" => Some(Token::Keyword(KeywordKind::Anyatha)),    // U+12038 BAL — else
        "𒀭" => Some(Token::Keyword(KeywordKind::Nivrtti)),    // U+1202D AŠ — return
        "𒃻" => Some(Token::Keyword(KeywordKind::Mana)),       // U+120FB NAM — let
        "𒅎" => Some(Token::Keyword(KeywordKind::Darshaya)),   // U+1214E TA — print
        "𒁺" => Some(Token::BoolLit(true)),                    // U+12079 DU — true
        "𒁺𒂵" => Some(Token::BoolLit(false)),                 // U+12080 DUG — false

        // ASCII transliteration aliases
        "a" => Some(Token::Keyword(KeywordKind::Karya)),
        "tukul" => Some(Token::Keyword(KeywordKind::Yadi)),
        "bal" => Some(Token::Keyword(KeywordKind::Anyatha)),
        "ash" => Some(Token::Keyword(KeywordKind::Nivrtti)),
        "nam" => Some(Token::Keyword(KeywordKind::Mana)),
        "ta" => Some(Token::Keyword(KeywordKind::Darshaya)),
        "du" => Some(Token::BoolLit(true)),
        "dug" => Some(Token::BoolLit(false)),

        // Devanagari/Sanskroot aliases (cross-frontend compatibility)
        "कार्य" => Some(Token::Keyword(KeywordKind::Karya)),
        "यदि" => Some(Token::Keyword(KeywordKind::Yadi)),
        "अन्यथा" => Some(Token::Keyword(KeywordKind::Anyatha)),
        "निवृत्ति" => Some(Token::Keyword(KeywordKind::Nivrtti)),
        "मान" => Some(Token::Keyword(KeywordKind::Mana)),
        "दर्शय" => Some(Token::Keyword(KeywordKind::Darshaya)),
        "सत्य" => Some(Token::BoolLit(true)),
        "असत्य" => Some(Token::BoolLit(false)),

        // Chinese/HanLan aliases (cross-frontend compatibility)
        "函" => Some(Token::Keyword(KeywordKind::Karya)),
        "若" => Some(Token::Keyword(KeywordKind::Yadi)),
        "否则" => Some(Token::Keyword(KeywordKind::Anyatha)),
        "返" => Some(Token::Keyword(KeywordKind::Nivrtti)),
        "令" => Some(Token::Keyword(KeywordKind::Mana)),
        "示" => Some(Token::Keyword(KeywordKind::Darshaya)),
        "真" => Some(Token::BoolLit(true)),
        "假" => Some(Token::BoolLit(false)),

        _ => None,
    }
}

/// Lex cuneiform source code into tokens.
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
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
            let mut s = String::new();
            loop {
                if i >= len {
                    return Err(LexError::UnterminatedString);
                }
                let sc = chars[i];
                if sc == '"' {
                    i += 1;
                    break;
                }
                if sc == '\\' && i + 1 < len {
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
                    i += 1;
                    continue;
                }
                s.push(sc);
                i += 1;
            }
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
            let val = num_str.parse::<i64>().map_err(|_| LexError::InvalidInt(num_str))?;
            tokens.push(Token::IntLit(val));
            continue;
        }

        // Identifiers and keywords (including cuneiform signs)
        if is_ident_start(c) {
            let start = i;
            while i < len && is_ident_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if let Some(tok) = keyword_for(&word) {
                tokens.push(tok);
            } else {
                tokens.push(Token::Ident(word));
            }
            continue;
        }

        // Two-character operators
        if i + 1 < len {
            match (c, chars[i + 1]) {
                ('>', '=') => { tokens.push(Token::GtEq); i += 2; continue; }
                ('<', '=') => { tokens.push(Token::LtEq); i += 2; continue; }
                ('=', '=') => { tokens.push(Token::EqEq); i += 2; continue; }
                ('!', '=') => { tokens.push(Token::BangEq); i += 2; continue; }
                _ => {}
            }
        }

        // Single-character tokens
        match c {
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => tokens.push(Token::Star),
            '/' => tokens.push(Token::Slash),
            '>' => tokens.push(Token::Gt),
            '<' => tokens.push(Token::Lt),
            '=' => tokens.push(Token::Eq),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            '{' => tokens.push(Token::LBrace),
            '}' => tokens.push(Token::RBrace),
            ',' => tokens.push(Token::Comma),
            ';' => tokens.push(Token::Semi),
            _ => return Err(LexError::UnexpectedChar(c)),
        }
        i += 1;
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimal_print_cuneiform() {
        // 𒀀 main() { 𒅎("hello"); }
        let src = "𒀀 main() { 𒅎(\"hello\"); }";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Karya));
        assert_eq!(tokens[1], Token::Ident("main".to_string()));
        assert_eq!(tokens[5], Token::Keyword(KeywordKind::Darshaya));
    }

    #[test]
    fn test_transliteration_aliases() {
        let src = "a main() { ta(\"hello\"); }";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Karya));
        assert_eq!(tokens[5], Token::Keyword(KeywordKind::Darshaya));
    }

    #[test]
    fn test_boolean_literals() {
        let src = "du dug";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::BoolLit(true));
        assert_eq!(tokens[1], Token::BoolLit(false));
    }

    #[test]
    fn test_operators() {
        let src = "+ - * / > < >= <= == !=";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Plus);
        assert_eq!(tokens[4], Token::Gt);
        assert_eq!(tokens[6], Token::GtEq);
        assert_eq!(tokens[9], Token::BangEq);
    }

    #[test]
    fn test_comments() {
        let src = "a main() { // comment\nta(\"hi\"); }";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Karya));
        assert_eq!(tokens[5], Token::Keyword(KeywordKind::Darshaya));
    }

    #[test]
    fn test_if_else_cuneiform() {
        let src = "𒅗 (x > 0) { } 𒀸 { }";
        let tokens = lex(src).unwrap();
        assert_eq!(tokens[0], Token::Keyword(KeywordKind::Yadi));
        assert_eq!(tokens[8], Token::Keyword(KeywordKind::Anyatha));
    }
}
