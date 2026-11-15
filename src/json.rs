//! A small, dependency-free JSON reader and writer.
//!
//! The collector emits one JSON document produced by `ConvertTo-Json` in
//! Windows PowerShell 5.1. This module parses it into [`Value`] and offers
//! forgiving accessors, because PowerShell sometimes emits a single object
//! where an array was expected.

use std::fmt::Write as _;

/// A parsed JSON value. Object keys keep their original order.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// Any JSON number.
    Number(f64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Self>),
    /// An object, as ordered key/value pairs.
    Object(Vec<(String, Self)>),
}

/// The shared `null` returned by lookups that find nothing.
static NULL: Value = Value::Null;

/// An error from [`parse`], with the byte offset where parsing stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset into the input.
    pub offset: usize,
    /// What went wrong.
    pub message: &'static str,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid JSON at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

impl Value {
    /// Looks up `key` in an object. Returns `Null` for anything else.
    /// Key matching is case-insensitive, as PowerShell property names are.
    #[must_use]
    pub fn get(&self, key: &str) -> &Self {
        if let Self::Object(pairs) = self {
            pairs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map_or(&NULL, |(_, v)| v)
        } else {
            &NULL
        }
    }

    /// Follows a path of object keys.
    #[must_use]
    pub fn path(&self, keys: &[&str]) -> &Self {
        keys.iter().fold(self, |v, k| v.get(k))
    }

    /// True for `Null`.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// The string value, if this is a non-empty string after trimming.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => {
                let t = s.trim();
                if t.is_empty() { None } else { Some(t) }
            }
            _ => None,
        }
    }

    /// A display string for strings, numbers and booleans.
    #[must_use]
    pub fn as_text(&self) -> Option<String> {
        match self {
            Self::String(_) => self.as_str().map(ToOwned::to_owned),
            Self::Number(n) => Some(format_number(*n)),
            Self::Bool(b) => Some(if *b { "Yes" } else { "No" }.to_owned()),
            _ => None,
        }
    }

    /// The value as a float. Numeric strings are accepted.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    /// The value as an unsigned integer, if it is a non-negative whole number.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        let n = self.as_f64()?;
        if n >= 0.0 && n.fract() == 0.0 && n <= 9_007_199_254_740_992.0 {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "range and fraction checked above"
            )]
            Some(n as u64)
        } else {
            None
        }
    }

    /// The value as a boolean. Accepts JSON booleans and 0/1 numbers.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Number(n) => Some(*n != 0.0),
            Self::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    /// Treats the value as a list: arrays yield their items, `Null` yields
    /// nothing, and any other value yields itself once.
    #[must_use]
    pub fn items(&self) -> Vec<&Self> {
        match self {
            Self::Array(items) => items.iter().collect(),
            Self::Null => Vec::new(),
            other => vec![other],
        }
    }

    /// Serialises the value as compact JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out, None, 0);
        out
    }

    /// Serialises the value as indented JSON.
    #[must_use]
    pub fn to_json_pretty(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out, Some(2), 0);
        out.push('\n');
        out
    }

    fn write_json(&self, out: &mut String, indent: Option<usize>, depth: usize) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Number(n) => out.push_str(&format_number(*n)),
            Self::String(s) => write_string(out, s),
            Self::Array(items) => {
                write_container(out, '[', ']', items.len(), indent, depth, |out, i| {
                    if let Some(item) = items.get(i) {
                        item.write_json(out, indent, depth + 1);
                    }
                });
            }
            Self::Object(pairs) => {
                write_container(out, '{', '}', pairs.len(), indent, depth, |out, i| {
                    if let Some((k, v)) = pairs.get(i) {
                        write_string(out, k);
                        out.push(':');
                        if indent.is_some() {
                            out.push(' ');
                        }
                        v.write_json(out, indent, depth + 1);
                    }
                });
            }
        }
    }
}

fn write_container(
    out: &mut String,
    open: char,
    close: char,
    len: usize,
    indent: Option<usize>,
    depth: usize,
    mut item: impl FnMut(&mut String, usize),
) {
    out.push(open);
    if len == 0 {
        out.push(close);
        return;
    }
    for i in 0..len {
        if i > 0 {
            out.push(',');
        }
        if let Some(width) = indent {
            out.push('\n');
            out.push_str(&" ".repeat(width * (depth + 1)));
        }
        item(out, i);
    }
    if let Some(width) = indent {
        out.push('\n');
        out.push_str(&" ".repeat(width * depth));
    }
    out.push(close);
}

/// Formats a number the way JSON expects: integers without a fraction.
#[must_use]
pub fn format_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{n:.0}")
    } else if n.is_finite() {
        format!("{n}")
    } else {
        "null".to_owned()
    }
}

/// Appends `s` as a quoted, escaped JSON string.
pub fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses a JSON document. A UTF-8 byte-order mark is ignored.
///
/// # Errors
///
/// Returns [`ParseError`] if the input is not valid JSON.
pub fn parse(input: &str) -> Result<Value, ParseError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut p = Parser {
        bytes: input.as_bytes(),
        pos: 0,
        depth: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos == p.bytes.len() {
        Ok(v)
    } else {
        Err(p.err("trailing characters"))
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 128;

impl Parser<'_> {
    const fn err(&self, message: &'static str) -> ParseError {
        ParseError {
            offset: self.pos,
            message,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn expect_literal(&mut self, lit: &[u8], v: Value) -> Result<Value, ParseError> {
        if self.bytes.get(self.pos..self.pos + lit.len()) == Some(lit) {
            self.pos += lit.len();
            Ok(v)
        } else {
            Err(self.err("unknown literal"))
        }
    }

    fn value(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.expect_literal(b"true", Value::Bool(true)),
            Some(b'f') => self.expect_literal(b"false", Value::Bool(false)),
            Some(b'n') => self.expect_literal(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
            None => Err(self.err("unexpected end of input")),
        }
    }

    const fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(self.err("nesting too deep"))
        } else {
            Ok(())
        }
    }

    fn object(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.pos += 1;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Value::Object(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected object key"));
            }
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.pos += 1;
            self.skip_ws();
            let v = self.value()?;
            pairs.push((key, v));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Value::Object(pairs));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.pos += 1;
        }
        let text = self
            .bytes
            .get(start..self.pos)
            .and_then(|b| std::str::from_utf8(b).ok())
            .ok_or_else(|| self.err("invalid number"))?;
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.err("invalid number"))
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let digits = self
            .bytes
            .get(self.pos..self.pos + 4)
            .and_then(|b| std::str::from_utf8(b).ok())
            .ok_or_else(|| self.err("truncated \\u escape"))?;
        let v = u32::from_str_radix(digits, 16).map_err(|_| self.err("invalid \\u escape"))?;
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            self.pos += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated escape"));
                    };
                    self.pos += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let c = self.unicode_escape()?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                _ => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("invalid UTF-8 in string"))
    }

    fn unicode_escape(&mut self) -> Result<char, ParseError> {
        let first = self.hex4()?;
        if (0xD800..0xDC00).contains(&first) {
            if self.bytes.get(self.pos..self.pos + 2) == Some(b"\\u") {
                self.pos += 2;
                let second = self.hex4()?;
                if (0xDC00..0xE000).contains(&second) {
                    let combined = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
                    return Ok(char::from_u32(combined).unwrap_or('\u{fffd}'));
                }
            }
            return Ok('\u{fffd}');
        }
        Ok(char::from_u32(first).unwrap_or('\u{fffd}'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_documents() {
        let v =
            parse(r#"{"a": [1, 2.5, -3e2], "b": {"c": "x\"y"}, "d": null, "e": true}"#).unwrap();
        assert_eq!(v.get("a").items().len(), 3);
        assert_eq!(v.path(&["b", "c"]).as_str(), Some("x\"y"));
        assert!(v.get("d").is_null());
        assert_eq!(v.get("E").as_bool(), Some(true));
        assert_eq!(v.get("a").items()[2].as_f64(), Some(-300.0));
    }

    #[test]
    fn decodes_unicode_escapes_and_surrogates() {
        let v = parse(r#""<TM> ® 🚀""#).unwrap();
        assert_eq!(v.as_str(), Some("<TM> \u{ae} \u{1f680}"));
    }

    #[test]
    fn single_object_is_treated_as_one_item_list() {
        let v = parse(r#"{"x": {"Name": "only"}}"#).unwrap();
        assert_eq!(v.get("x").items().len(), 1);
        assert_eq!(v.get("missing").items().len(), 0);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("{").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("tru").is_err());
        assert!(parse("1 2").is_err());
        assert!(parse(&"[".repeat(500)).is_err());
    }

    #[test]
    fn round_trips() {
        let src = r#"{"name":"A \"b\"\n","list":[1,2,{"z":false}],"n":null,"f":1.5}"#;
        let v = parse(src).unwrap();
        assert_eq!(v.to_json(), src);
        assert_eq!(parse(&v.to_json_pretty()).unwrap(), v);
    }

    #[test]
    fn numbers_and_text() {
        assert_eq!(
            parse("4000787030016").unwrap().as_u64(),
            Some(4_000_787_030_016)
        );
        assert_eq!(parse("-1").unwrap().as_u64(), None);
        assert_eq!(parse(r#"" 42 ""#).unwrap().as_u64(), Some(42));
        assert_eq!(parse("3").unwrap().as_text().as_deref(), Some("3"));
        assert_eq!(parse(r#""   ""#).unwrap().as_str(), None);
    }

    #[test]
    fn ignores_byte_order_mark() {
        assert_eq!(parse("\u{feff}[]").unwrap(), Value::Array(vec![]));
    }
}
