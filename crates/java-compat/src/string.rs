//! `java.lang.String` / `java.lang.Long` behaviour that affects seeds.

use crate::digit_table::DIGIT_RANGES;

/// `String.hashCode()` — computed over UTF-16 code units, not Unicode scalars.
pub fn hash_code(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// `String.trim()` — strips chars `<= U+0020` only. Unlike `str::trim`, it does
/// NOT strip Unicode whitespace such as U+00A0 or U+3000.
pub fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= '\u{20}')
}

/// `Character.digit(ch, 10)` for a UTF-16 code unit. Java accepts every Unicode
/// decimal digit (Arabic-Indic, fullwidth, Devanagari, ...), not only ASCII.
pub fn digit10(c: u16) -> Option<u32> {
    let idx = DIGIT_RANGES.partition_point(|&(start, _)| start <= c);
    let (start, zero_value) = *DIGIT_RANGES.get(idx.checked_sub(1)?)?;
    let v = zero_value as u32 + (c - start) as u32;
    // each range is a run of consecutive digits ending at value 9
    (v <= 9).then_some(v)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberFormatError;

/// `Long.parseLong(s)` (radix 10), including its Unicode-digit and `+`/`-`
/// sign handling and overflow detection.
pub fn parse_long(s: &str) -> Result<i64, NumberFormatError> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let (&first, rest) = units.split_first().ok_or(NumberFormatError)?;

    let (negative, digits) = if first < b'0' as u16 {
        match first {
            0x2D => (true, rest),  // '-'
            0x2B => (false, rest), // '+'
            _ => return Err(NumberFormatError),
        }
    } else {
        (false, &units[..])
    };
    if digits.is_empty() {
        return Err(NumberFormatError);
    }

    // Accumulate negatively like the JDK so i64::MIN is representable.
    let mut result: i64 = 0;
    for &c in digits {
        let d = digit10(c).ok_or(NumberFormatError)? as i64;
        result = result
            .checked_mul(10)
            .and_then(|r| r.checked_sub(d))
            .ok_or(NumberFormatError)?;
    }
    if negative {
        Ok(result)
    } else {
        result.checked_neg().ok_or(NumberFormatError)
    }
}
