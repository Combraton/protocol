//! Strict value domain (ENCODING section 1), canonical form (section 2) and digests.
//!
//! Hand-written on purpose: the conformance runner uses serde_json plus a vetted
//! RFC 8785 crate, so the two sides share no parser or canonicalizer code.

use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

pub const SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const MAX_NESTING: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFault {
    InvalidUtf8,
    ParseError,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Laxness {
    /// Mutant `accept-duplicate-members`: keep the last duplicate member.
    pub duplicate_members: bool,
    /// Mutant `lax-numbers`: accept fractions, exponents and out-of-range integers.
    pub numbers: bool,
    /// Mutant `accept-lone-surrogates`: replace unpaired surrogate escapes instead of refusing.
    pub surrogates: bool,
    /// Mutant `accept-noncharacters`.
    pub noncharacters: bool,
}

pub fn parse_frame(bytes: &[u8], lax: Laxness) -> Result<Value, FrameFault> {
    let text = std::str::from_utf8(bytes).map_err(|_| FrameFault::InvalidUtf8)?;
    let mut parser = Parser {
        s: text.as_bytes(),
        i: 0,
        depth: 0,
        lax,
    };
    parser.whitespace();
    let value = parser.value()?;
    parser.whitespace();
    if parser.i != parser.s.len() {
        return Err(FrameFault::ParseError);
    }
    Ok(value)
}

pub fn is_blank(bytes: &[u8]) -> bool {
    bytes.iter().all(|b| matches!(b, b' ' | b'\t' | b'\r'))
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
    lax: Laxness,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.i += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), FrameFault> {
        if self.peek() == Some(byte) {
            self.i += 1;
            Ok(())
        } else {
            Err(FrameFault::ParseError)
        }
    }

    fn value(&mut self) -> Result<Value, FrameFault> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'n') => self.literal(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(FrameFault::ParseError),
        }
    }

    fn literal(&mut self, word: &[u8], value: Value) -> Result<Value, FrameFault> {
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(value)
        } else {
            Err(FrameFault::ParseError)
        }
    }

    fn enter(&mut self) -> Result<(), FrameFault> {
        self.depth += 1;
        if self.depth > MAX_NESTING {
            Err(FrameFault::ParseError)
        } else {
            Ok(())
        }
    }

    fn object(&mut self) -> Result<Value, FrameFault> {
        self.enter()?;
        self.i += 1;
        let mut map = Map::new();
        self.whitespace();
        if self.peek() == Some(b'}') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.whitespace();
            if self.peek() != Some(b'"') {
                return Err(FrameFault::ParseError);
            }
            let key = self.string()?;
            self.whitespace();
            self.expect(b':')?;
            self.whitespace();
            let member = self.value()?;
            if map.contains_key(&key) && !self.lax.duplicate_members {
                return Err(FrameFault::ParseError);
            }
            map.insert(key, member);
            self.whitespace();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(FrameFault::ParseError),
            }
        }
        self.depth -= 1;
        Ok(Value::Object(map))
    }

    fn array(&mut self) -> Result<Value, FrameFault> {
        self.enter()?;
        self.i += 1;
        let mut items = Vec::new();
        self.whitespace();
        if self.peek() == Some(b']') {
            self.i += 1;
            self.depth -= 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.whitespace();
            items.push(self.value()?);
            self.whitespace();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(FrameFault::ParseError),
            }
        }
        self.depth -= 1;
        Ok(Value::Array(items))
    }

    fn hex4(&mut self) -> Result<u32, FrameFault> {
        let digits = self
            .s
            .get(self.i..self.i + 4)
            .ok_or(FrameFault::ParseError)?;
        let text = std::str::from_utf8(digits).map_err(|_| FrameFault::ParseError)?;
        let value = u32::from_str_radix(text, 16).map_err(|_| FrameFault::ParseError)?;
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err(FrameFault::ParseError);
        }
        self.i += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, FrameFault> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while let Some(byte) = self.peek() {
                if byte == b'"' || byte == b'\\' || byte < 0x20 {
                    break;
                }
                self.i += 1;
            }
            // Safe: the input was validated as UTF-8 and we only stop at ASCII bytes.
            out.push_str(
                std::str::from_utf8(&self.s[start..self.i]).map_err(|_| FrameFault::ParseError)?,
            );
            match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    break;
                }
                Some(b'\\') => {
                    self.i += 1;
                    let escape = self.peek().ok_or(FrameFault::ParseError)?;
                    self.i += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.hex4()?;
                            let lone = (0xDC00..=0xDFFF).contains(&unit)
                                || ((0xD800..=0xDBFF).contains(&unit)
                                    && self.s.get(self.i..self.i + 2) != Some(b"\\u"));
                            if lone && self.lax.surrogates {
                                out.push('\u{FFFD}');
                                continue;
                            }
                            let code = if (0xD800..=0xDBFF).contains(&unit) {
                                if self.s.get(self.i..self.i + 2) != Some(b"\\u") {
                                    return Err(FrameFault::ParseError);
                                }
                                self.i += 2;
                                let low = self.hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&low) {
                                    return Err(FrameFault::ParseError);
                                }
                                0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
                            } else if (0xDC00..=0xDFFF).contains(&unit) {
                                return Err(FrameFault::ParseError);
                            } else {
                                unit
                            };
                            out.push(char::from_u32(code).ok_or(FrameFault::ParseError)?);
                        }
                        _ => return Err(FrameFault::ParseError),
                    }
                }
                _ => return Err(FrameFault::ParseError),
            }
        }
        if out.chars().any(is_noncharacter) && !self.lax.noncharacters {
            return Err(FrameFault::ParseError);
        }
        Ok(out)
    }

    fn number(&mut self) -> Result<Value, FrameFault> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return Err(FrameFault::ParseError),
        }
        let integer_end = self.i;
        let mut fractional = false;
        if self.peek() == Some(b'.') {
            fractional = true;
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(FrameFault::ParseError);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            fractional = true;
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(FrameFault::ParseError);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let token =
            std::str::from_utf8(&self.s[start..self.i]).map_err(|_| FrameFault::ParseError)?;
        if self.lax.numbers {
            if let Ok(value) = token.parse::<i64>() {
                return Ok(Value::Number(value.into()));
            }
            let float: f64 = token.parse().map_err(|_| FrameFault::ParseError)?;
            return Number::from_f64(float)
                .map(Value::Number)
                .ok_or(FrameFault::ParseError);
        }
        if fractional || token == "-0" {
            return Err(FrameFault::ParseError);
        }
        let integer = &token[..integer_end - start];
        if integer.trim_start_matches('-').len() > 16 {
            return Err(FrameFault::ParseError);
        }
        let value: i64 = integer.parse().map_err(|_| FrameFault::ParseError)?;
        if !(-SAFE_INTEGER..=SAFE_INTEGER).contains(&value) {
            return Err(FrameFault::ParseError);
        }
        Ok(Value::Number(value.into()))
    }
}

fn is_noncharacter(ch: char) -> bool {
    let code = ch as u32;
    (0xFDD0..=0xFDEF).contains(&code) || (code & 0xFFFE) == 0xFFFE
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalFlaws {
    /// Mutant `canonical-code-point-order`: sort members by code point instead of UTF-16 units.
    pub code_point_order: bool,
    /// Mutant `canonical-ascii-escape`: escape non-ASCII characters.
    pub ascii_escape: bool,
}

pub fn canonical(value: &Value, flaws: CanonicalFlaws) -> Vec<u8> {
    let mut out = String::new();
    emit(value, flaws, &mut out);
    out.into_bytes()
}

fn emit(value: &Value, flaws: CanonicalFlaws, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&number.to_string()),
        Value::String(text) => emit_string(text, flaws, out),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                emit(item, flaws, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            if flaws.code_point_order {
                keys.sort();
            } else {
                keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            }
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                emit_string(key, flaws, out);
                out.push(':');
                emit(&map[key], flaws, out);
            }
            out.push('}');
        }
    }
}

fn emit_string(text: &str, flaws: CanonicalFlaws, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if flaws.ascii_escape && (c as u32) > 0x7E => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

pub fn encode_frame(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("serializable");
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_bytes(text: &str) -> Vec<u8> {
        hex::decode(text).unwrap()
    }

    /// The published vectors are the only coupling to other implementations.
    #[test]
    fn encoding_vectors() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../vectors/encoding.json");
        let vectors: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for case in vectors["canonical"].as_array().unwrap() {
            let input = hex_bytes(case["input_hex"].as_str().unwrap());
            let value = parse_frame(&input, Laxness::default())
                .unwrap_or_else(|e| panic!("{}: {e:?}", case["id"]));
            let canon = canonical(&value, CanonicalFlaws::default());
            assert_eq!(
                hex::encode(&canon),
                case["canonical_hex"].as_str().unwrap(),
                "{}",
                case["id"]
            );
            assert_eq!(
                sha256_digest(&canon),
                case["digest"].as_str().unwrap(),
                "{}",
                case["id"]
            );
        }
        for case in vectors["rejected"].as_array().unwrap() {
            let input = hex_bytes(case["input_hex"].as_str().unwrap());
            assert!(
                parse_frame(&input, Laxness::default()).is_err(),
                "accepted {}",
                case["id"]
            );
        }
    }
}
