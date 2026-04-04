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
        self.name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringLit {
    pub raw: String,
    pub span: Span,
}

impl StringLit {
    /// Strip surrounding quotes and unescape the string value.
    pub fn value(&self) -> String {
        let s = &self.raw;
        if s.len() < 2 {
            return String::new();
        }
        let inner = &s[1..s.len() - 1];
        // Basic unescaping for common sequences
        let mut result = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    Some('\\') => result.push('\\'),
                    Some('"') => result.push('"'),
                    Some('\'') => result.push('\''),
                    Some('0') => result.push('\0'),
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
        let raw = format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        Self {
            raw,
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
