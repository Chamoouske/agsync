//! Secrets: `${VAR}` references, resolved from the process environment, then `secrets.env`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod file;

#[cfg(windows)]
mod acl;

/// Where a secret value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The user's environment: agents that expand `${VAR}` get the reference.
    Env,
    /// Only `secrets.env`: agents always get the value.
    File,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Origin::Env => "env",
            Origin::File => "file",
        }
    }
}

/// Secret lookup: process environment first, then the `secrets.env` values.
#[derive(Debug, Default, Clone)]
pub struct Secrets {
    file_values: BTreeMap<String, String>,
}

impl Secrets {
    /// Reads `secrets.env`; a missing file means no file secrets.
    pub fn load(path: &Path) -> anyhow::Result<Secrets> {
        let file_values = match std::fs::read_to_string(path) {
            Ok(text) => parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(anyhow::anyhow!("ler {}: {e}", path.display())),
        };
        Ok(Secrets { file_values })
    }

    /// Lookup over `file_values` only, for tests.
    pub fn from_file_values(file_values: BTreeMap<String, String>) -> Secrets {
        Secrets { file_values }
    }

    /// Value of `name` and where it came from, if defined anywhere.
    pub fn lookup(&self, name: &str) -> Option<(Origin, String)> {
        if let Some(value) = std::env::var_os(name) {
            return Some((Origin::Env, value.to_string_lossy().to_string()));
        }
        let value = self.file_values.get(name)?;
        Some((Origin::File, value.clone()))
    }
}

/// `secrets.env` inside the local config dir.
pub fn file_path(config_dir: &Path) -> PathBuf {
    config_dir.join("secrets.env")
}

/// Parses `NAME=value` lines; blank lines and lines starting with `#` are skipped.
pub fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(parse_line)
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// Name and value of one `NAME=value` line, or `None` for blanks, comments and junk.
pub fn parse_line(line: &str) -> Option<(&str, &str)> {
    let line = line.trim_end_matches('\r');
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (name, value) = trimmed.split_once('=')?;
    let name = name.trim();
    if !valid_name(name) {
        return None;
    }
    Some((name, value))
}

/// Letters, digits and `_`, not starting with a digit.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let first_ok = first.is_ascii_alphabetic() || first == '_';
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Names of every `${NAME}` reference in `text`, in order, without repeats.
pub fn refs(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for name in ref_names(text) {
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names
}

/// `NAME` when the whole text is one `${NAME}` reference.
pub fn whole_ref(text: &str) -> Option<&str> {
    let name = text.strip_prefix("${")?.strip_suffix('}')?;
    valid_name(name).then_some(name)
}

/// Replaces each `${NAME}` for which `value` returns `Some`; others stay as written.
pub fn substitute(text: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let name = after.find('}').map(|end| &after[..end]);
        match name
            .filter(|n| valid_name(n))
            .and_then(|n| value(n).map(|v| (n, v)))
        {
            Some((name, v)) => {
                out.push_str(&v);
                rest = &after[name.len() + 1..];
            }
            None => {
                out.push_str("${");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn ref_names(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        match after.find('}').map(|end| &after[..end]) {
            Some(name) if valid_name(name) => {
                names.push(name);
                rest = &after[name.len() + 1..];
            }
            _ => rest = after,
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_env_file() {
        let text = "# comment\n\nA=1\r\n  B = two words \nbad line\n1X=no\nC=x=y\nEMPTY=\n";
        let values = parse(text);
        assert_eq!(values["A"], "1");
        assert_eq!(values["B"], " two words ");
        assert_eq!(values["C"], "x=y");
        assert_eq!(values["EMPTY"], "");
        assert_eq!(values.len(), 4);
    }

    #[test]
    fn finds_refs() {
        assert_eq!(refs("a ${X} ${Y_2}/${X} $Z ${} ${1A} ${open"), ["X", "Y_2"]);
        assert!(refs("plain").is_empty());
    }

    #[test]
    fn whole_ref_is_one_reference_and_nothing_else() {
        assert_eq!(whole_ref("${TOKEN}"), Some("TOKEN"));
        assert_eq!(whole_ref("Bearer ${TOKEN}"), None);
        assert_eq!(whole_ref("${A}${B}"), None);
        assert_eq!(whole_ref("${1A}"), None);
    }

    #[test]
    fn substitutes_known_refs_only() {
        let value = |name: &str| (name == "X").then(|| "v".to_string());
        assert_eq!(substitute("a${X}b${Y}c${X}", value), "avb${Y}cv");
        assert_eq!(substitute("${ open ${X", value), "${ open ${X");
        assert_eq!(substitute("$${X}}", value), "$v}");
    }

    #[test]
    fn env_wins_over_file() {
        let name = "AGSYNC_TEST_SECRET_ENV_WINS";
        let file = BTreeMap::from([
            (name.to_string(), "from-file".to_string()),
            ("ONLY_FILE_AGSYNC".to_string(), "f".to_string()),
        ]);
        let secrets = Secrets::from_file_values(file);
        assert_eq!(
            secrets.lookup(name),
            Some((Origin::File, "from-file".into()))
        );
        unsafe { std::env::set_var(name, "from-env") };
        assert_eq!(secrets.lookup(name), Some((Origin::Env, "from-env".into())));
        unsafe { std::env::remove_var(name) };
        assert_eq!(
            secrets.lookup("ONLY_FILE_AGSYNC"),
            Some((Origin::File, "f".into()))
        );
        assert_eq!(secrets.lookup("AGSYNC_TEST_MISSING_SECRET"), None);
    }
}
