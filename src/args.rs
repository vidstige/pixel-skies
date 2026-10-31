//! Command line options of the form `--key value`.

use std::collections::HashMap;
use std::str::FromStr;

pub struct Args(HashMap<String, String>);

impl Args {
    pub fn parse(mut words: impl Iterator<Item = String>) -> Self {
        let mut options = HashMap::new();
        while let Some(word) = words.next() {
            let key = word
                .strip_prefix("--")
                .unwrap_or_else(|| panic!("expected --option, got {word}"));
            let value = words.next().unwrap_or_else(|| panic!("--{key} needs a value"));
            options.insert(key.to_string(), value);
        }
        Self(options)
    }

    pub fn get<T: FromStr>(&self, key: &str, default: T) -> T {
        let parse = |v: &String| v.parse().unwrap_or_else(|_| panic!("bad --{key}: {v}"));
        self.0.get(key).map_or(default, parse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Args {
        Args::parse(line.split_whitespace().map(String::from))
    }

    #[test]
    fn options_parse_with_defaults_for_the_missing() {
        let a = args("--elevation -4.5 --out sky.png");
        assert_eq!(a.get("elevation", 10.0), -4.5);
        assert_eq!(a.get("out", String::new()), "sky.png");
        assert_eq!(a.get("width", 640), 640);
    }
}
