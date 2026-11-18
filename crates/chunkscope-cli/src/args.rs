//! Argument parsing: `--flag`, `--option value` (or `--option=value`) and
//! positional words, in any order.

use std::collections::HashMap;

#[derive(Debug)]
pub struct Args {
    pub positional: Vec<String>,
    flags: Vec<String>,
    options: HashMap<String, String>,
}

impl Args {
    /// Parse `raw`. `flags` take no value; `options` take one. Anything else
    /// starting with `--` is an error. A lone `-` or a negative number is a
    /// positional word (coordinates and seeds can be negative).
    pub fn parse(raw: &[String], flags: &[&str], options: &[&str]) -> Result<Args, String> {
        let mut args = Args {
            positional: Vec::new(),
            flags: Vec::new(),
            options: HashMap::new(),
        };
        let mut it = raw.iter();
        while let Some(a) = it.next() {
            let Some(name) = a.strip_prefix("--") else {
                args.positional.push(a.clone());
                continue;
            };
            let (name, inline) = match name.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (name, None),
            };
            if flags.contains(&name) {
                if inline.is_some() {
                    return Err(format!("--{name} takes no value"));
                }
                args.flags.push(name.to_owned());
            } else if options.contains(&name) {
                let value = match inline {
                    Some(v) => v,
                    None => it
                        .next()
                        .cloned()
                        .ok_or_else(|| format!("--{name} needs a value"))?,
                };
                args.options.insert(name.to_owned(), value);
            } else {
                return Err(format!("unknown option --{name}"));
            }
        }
        Ok(args)
    }

    pub fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    pub fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }

    /// An integer option, or `default` when absent.
    pub fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
        match self.option(name) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|_| format!("--{name}: {v:?} is not a valid number")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(words: &[&str]) -> Result<Args, String> {
        let raw: Vec<String> = words.iter().map(|s| s.to_string()).collect();
        Args::parse(&raw, &["json"], &["dimension", "count"])
    }

    #[test]
    fn flags_options_and_words_in_any_order() {
        let a = parse(&["--json", "42", "--dimension", "nether", "-10", "--count=5"]).unwrap();
        assert!(a.flag("json"));
        assert_eq!(a.option("dimension"), Some("nether"));
        assert_eq!(a.positional, ["42", "-10"]);
        assert_eq!(a.number("count", 0u64), Ok(5));
        assert_eq!(a.number("missing", 7u64), Ok(7));
    }

    #[test]
    fn mistakes_are_explained() {
        assert!(parse(&["--nope"]).unwrap_err().contains("unknown option"));
        assert!(
            parse(&["--dimension"])
                .unwrap_err()
                .contains("needs a value")
        );
        assert!(parse(&["--json=1"]).unwrap_err().contains("no value"));
        let a = parse(&["--count", "x"]).unwrap();
        assert!(a.number("count", 0u64).unwrap_err().contains("not a valid"));
    }
}
