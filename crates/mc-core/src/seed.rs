//! How Minecraft: Java Edition turns the "Seed for the World Generator" text
//! box into a 64-bit world seed (`WorldOptions.parseSeed`, 1.18+):
//!
//! ```java
//! String s = input.trim();
//! if (s.isEmpty()) return OptionalLong.empty();          // random seed
//! try { return OptionalLong.of(Long.parseLong(s)); }
//! catch (NumberFormatException e) { return OptionalLong.of(s.hashCode()); }
//! ```

use java_compat::string;

/// Returns `None` when the game would pick a random seed (blank input).
pub fn parse_seed(input: &str) -> Option<i64> {
    let s = string::trim(input);
    if s.is_empty() {
        return None;
    }
    Some(string::parse_long(s).unwrap_or_else(|_| string::hash_code(s) as i64))
}
