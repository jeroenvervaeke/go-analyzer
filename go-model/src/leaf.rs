use serde::{Deserialize, Serialize};

use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn synthetic(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            span: Span::synthetic(),
        }
    }

    pub fn is_exported(&self) -> bool {
        // Go spec: exported if first char is Unicode uppercase letter (class Lu)
        self.name.chars().next().is_some_and(|c| c.is_uppercase())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringLit {
    pub raw: String,
    pub span: Span,
}

impl StringLit {
    /// Strip surrounding quotes and unescape the string value.
    /// Handles all Go escape sequences: \a \b \f \n \r \t \v \\ \" \'
    /// \xNN (hex byte), \uNNNN (unicode), \UNNNNNNNN (unicode), \NNN (octal).
    pub fn value(&self) -> String {
        let s = &self.raw;
        if s.len() < 2 {
            return String::new();
        }
        let inner = &s[1..s.len() - 1];
        let mut result = String::with_capacity(inner.len());
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('a') => result.push('\x07'),
                    Some('b') => result.push('\x08'),
                    Some('f') => result.push('\x0C'),
                    Some('n') => result.push('\n'),
                    Some('r') => result.push('\r'),
                    Some('t') => result.push('\t'),
                    Some('v') => result.push('\x0B'),
                    Some('\\') => result.push('\\'),
                    Some('"') => result.push('"'),
                    Some('\'') => result.push('\''),
                    Some('x') => {
                        // \xNN — two hex digits, one byte
                        let hex: String = chars.by_ref().take(2).collect();
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                        }
                    }
                    Some('u') => {
                        // \uNNNN — four hex digits, unicode codepoint
                        let hex: String = chars.by_ref().take(4).collect();
                        if let Ok(cp) = u32::from_str_radix(&hex, 16)
                            && let Some(c) = char::from_u32(cp)
                        {
                            result.push(c);
                        }
                    }
                    Some('U') => {
                        // \UNNNNNNNN — eight hex digits, unicode codepoint
                        let hex: String = chars.by_ref().take(8).collect();
                        if let Ok(cp) = u32::from_str_radix(&hex, 16)
                            && let Some(c) = char::from_u32(cp)
                        {
                            result.push(c);
                        }
                    }
                    Some(d @ '0'..='7') => {
                        // \NNN — three octal digits (first already consumed)
                        let mut oct = String::new();
                        oct.push(d);
                        for _ in 0..2 {
                            if chars.peek().is_some_and(|c| ('0'..='7').contains(c)) {
                                oct.push(chars.next().unwrap());
                            }
                        }
                        if let Ok(byte) = u8::from_str_radix(&oct, 8) {
                            result.push(byte as char);
                        }
                    }
                    Some(other) => {
                        result.push('\\');
                        result.push(other);
                    }
                    None => result.push('\\'),
                }
            } else {
                result.push(c);
            }
        }
        result
    }

    pub fn from_value(s: &str) -> Self {
        let mut escaped = String::with_capacity(s.len() + 2);
        escaped.push('"');
        for c in s.chars() {
            match c {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                '\x07' => escaped.push_str("\\a"),
                '\x08' => escaped.push_str("\\b"),
                '\x0C' => escaped.push_str("\\f"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                '\x0B' => escaped.push_str("\\v"),
                c if c.is_control() => {
                    // Use \xNN for single-byte control chars, \uNNNN for multi-byte
                    let cp = c as u32;
                    if cp <= 0xFF {
                        escaped.push_str(&format!("\\x{cp:02x}"));
                    } else {
                        escaped.push_str(&format!("\\u{cp:04x}"));
                    }
                }
                c => escaped.push(c),
            }
        }
        escaped.push('"');
        Self {
            raw: escaped,
            span: Span::synthetic(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawStringLit {
    pub raw: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntLit {
    pub raw: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloatLit {
    pub raw: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImaginaryLit {
    pub raw: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuneLit {
    pub raw: String,
    pub span: Span,
}
