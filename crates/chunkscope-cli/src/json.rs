//! Just enough JSON to print results: objects built by hand, strings escaped.
//! The command line has no dependencies for this on purpose.

use std::fmt::Write;

/// A JSON string literal.
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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
    out
}

/// An object from `(key, already-encoded value)` pairs, on one line.
pub fn object(fields: &[(&str, String)]) -> String {
    let body: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}:{v}", string(k)))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// An array from already-encoded values.
pub fn array(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

/// A number with `decimals` digits (distances), never `NaN` or infinite.
pub fn float(x: f64, decimals: usize) -> String {
    if x.is_finite() {
        format!("{x:.decimals$}")
    } else {
        "null".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string("a\"b\\c\n\u{1}"), r#""a\"b\\c\n\u0001""#);
    }

    #[test]
    fn objects_and_arrays() {
        let o = object(&[("a", "1".into()), ("b", array(&["2".into(), string("x")]))]);
        assert_eq!(o, r#"{"a":1,"b":[2,"x"]}"#);
        assert_eq!(array(&[]), "[]");
        assert_eq!(float(f64::NAN, 1), "null");
        assert_eq!(float(12.345, 1), "12.3");
    }
}
