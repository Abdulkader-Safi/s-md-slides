//! Directive lines (`<!-- kind key=value ... -->`): tokenizing, quoting, number formatting.

use serde::Serialize;
use std::ops::Range;

/// Directive kinds the format knows. Any other kind counts only when it has `id=`.
pub const KNOWN_KINDS: &[&str] = &[
    "slide",
    "master",
    "layout",
    "placeholder",
    "notes",
    "text",
    "image",
    "shape",
    "code",
    "table",
    "diagram",
    "math",
    "video",
    "embed",
    "group",
];

/// One attribute token, in file order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Attr {
    /// Attribute name as written, such as `x` or `stroke-width`.
    pub key: String,
    /// Unescaped value. `None` for a bare key such as `locked`.
    pub value: Option<String>,
    /// The whole `key=value` token in the source.
    #[serde(skip)]
    pub span: Range<usize>,
    /// The value part (after `=`, quotes included) in the source.
    #[serde(skip)]
    pub value_span: Option<Range<usize>>,
}

/// A parsed directive line. Offsets are absolute byte offsets into the source.
#[derive(Debug, Clone)]
pub struct Directive {
    /// Directive kind, the first word after `<!--`, such as `text` or `slide`.
    pub kind: String,
    /// Attribute tokens in file order, duplicates included.
    pub attrs: Vec<Attr>,
    /// Where a new attribute goes: right after the last good token.
    pub insert_at: usize,
    /// Byte offset of the first token that failed to tokenize, if any.
    pub error_at: Option<usize>,
}

impl Directive {
    /// The last value for `key` (duplicate keys: the last one wins).
    pub fn get(&self, key: &str) -> Option<&Attr> {
        self.attrs.iter().rev().find(|a| a.key == key)
    }
}

fn is_key_byte(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-'
}

/// Parse a whole line as a directive. Returns `None` when the line is ordinary text:
/// not shaped like `<!-- kind ... -->`, or an unknown kind without `id=`.
/// `base` is the byte offset of the line start.
pub fn parse_directive(line: &str, base: usize) -> Option<Directive> {
    let t = line.trim_end();
    if !t.starts_with("<!--") || !t.ends_with("-->") || t.len() < 7 {
        return None;
    }
    let end = t.len() - 3;
    let b = t.as_bytes();
    let mut i = 4;
    while i < end && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    let kind_start = i;
    if i >= end || !b[i].is_ascii_lowercase() {
        return None;
    }
    while i < end && (b[i].is_ascii_lowercase() || b[i].is_ascii_digit() || b[i] == b'-') {
        i += 1;
    }
    if i < end && b[i] != b' ' && b[i] != b'\t' {
        return None;
    }
    let kind = t[kind_start..i].to_string();
    let mut insert_at = i;
    let mut attrs = Vec::with_capacity(8);
    let mut error_at = None;
    'tokens: loop {
        while i < end && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if i >= end {
            break;
        }
        let tok_start = i;
        if !b[i].is_ascii_lowercase() {
            error_at = Some(base + i);
            break;
        }
        while i < end && is_key_byte(b[i]) {
            i += 1;
        }
        let key = t[tok_start..i].to_string();
        let mut value = None;
        let mut value_span = None;
        if i < end && b[i] == b'=' {
            i += 1;
            let value_start = i;
            if i < end && b[i] == b'"' {
                i += 1;
                let mut v = String::new();
                loop {
                    if i >= end {
                        error_at = Some(base + tok_start);
                        break 'tokens;
                    }
                    match b[i] {
                        b'\\' if i + 1 < end && (b[i + 1] == b'"' || b[i + 1] == b'\\') => {
                            v.push(b[i + 1] as char);
                            i += 2;
                        }
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => {
                            // Safe: `i` is always on a char boundary here.
                            let ch = t[i..].chars().next().unwrap_or('\u{fffd}');
                            v.push(ch);
                            i += ch.len_utf8();
                        }
                    }
                }
                value = Some(v);
            } else {
                while i < end && b[i] != b' ' && b[i] != b'\t' {
                    i += 1;
                }
                value = Some(t[value_start..i].to_string());
            }
            value_span = Some(base + value_start..base + i);
        }
        if i < end && b[i] != b' ' && b[i] != b'\t' {
            error_at = Some(base + tok_start);
            break;
        }
        attrs.push(Attr {
            key,
            value,
            span: base + tok_start..base + i,
            value_span,
        });
        insert_at = i;
    }
    let counts = KNOWN_KINDS.contains(&kind.as_str()) || attrs.iter().any(|a| a.key == "id");
    counts.then_some(Directive {
        kind,
        attrs,
        insert_at: base + insert_at,
        error_at,
    })
}

/// Write a value bare when that is safe, otherwise double-quoted with `\"` and `\\`.
pub fn quote(v: &str) -> String {
    let bare = !v.is_empty()
        && !v.contains(|c: char| c.is_whitespace() || c == '"' || c == '\\')
        && !v.contains("-->");
    if bare {
        v.to_string()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

/// Integers when whole, otherwise up to two decimals with no trailing zeros.
pub fn fmt_num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.2}").trim_end_matches('0').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_quoted_bare_and_flags() {
        let d = parse_directive(r#"<!-- text id=t1 title="a \" b" locked x=-20.5 -->"#, 0).unwrap();
        assert_eq!(d.kind, "text");
        let v: Vec<_> = d
            .attrs
            .iter()
            .map(|a| (a.key.as_str(), a.value.as_deref()))
            .collect();
        assert_eq!(
            v,
            [
                ("id", Some("t1")),
                ("title", Some("a \" b")),
                ("locked", None),
                ("x", Some("-20.5"))
            ]
        );
        assert!(d.error_at.is_none());
    }

    #[test]
    fn ordinary_comments_are_text() {
        assert!(parse_directive("<!-- fix this later -->", 0).is_none());
        assert!(parse_directive("<!-- widget a=1 -->", 0).is_none());
        assert!(parse_directive("<!-- widget id=w1 -->", 0).is_some());
        assert!(parse_directive("<!--text-->", 0).is_some());
        assert!(parse_directive("text <!-- text -->", 0).is_none());
    }

    #[test]
    fn bad_token_is_reported_not_fatal() {
        let d = parse_directive("<!-- text x=1 =oops y=2 -->", 10).unwrap();
        assert_eq!(d.attrs.len(), 1);
        assert_eq!(d.error_at, Some(10 + 14));
    }

    #[test]
    fn numbers_and_quotes() {
        assert_eq!(fmt_num(10.0), "10");
        assert_eq!(fmt_num(-0.5), "-0.5");
        assert_eq!(fmt_num(1.257), "1.26");
        assert_eq!(fmt_num(2.10), "2.1");
        assert_eq!(quote("abc"), "abc");
        assert_eq!(quote("a b"), "\"a b\"");
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote(r#"x"y"#), r#""x\"y""#);
    }
}
