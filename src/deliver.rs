//! Turning an MCP entry with `${VAR}` references into what one agent gets.

use crate::hash::hash_bytes;
use crate::mcp::McpSpec;
use crate::plan::Want;
use crate::secret::{Origin, Secrets, substitute};
use crate::state::{Tracked, state_key};

/// One MCP server as agsync wants it in one agent.
#[derive(Debug, Clone)]
pub struct McpWant {
    /// Entry with `${VAR}` still unresolved: safe to show and to hash.
    pub shown: McpSpec,
    /// Entry written to the agent: may hold secret values.
    pub write: McpSpec,
    /// State hash: `shown` plus where each secret comes from, never the values.
    pub hash: String,
    /// Commit of the git source, if any.
    pub sha: Option<String>,
}

impl McpWant {
    /// Resolves every `${VAR}`. A variable from the user's environment stays a reference
    /// when the agent expands references itself; otherwise its value is written.
    /// Returns the missing variable names when any is undefined.
    pub fn new(
        shown: McpSpec,
        secrets: &Secrets,
        expands_refs: bool,
        sha: Option<String>,
    ) -> Result<McpWant, Vec<String>> {
        let mut origins = Vec::new();
        let mut missing = Vec::new();
        for name in shown.secret_refs() {
            match secrets.lookup(&name) {
                Some((origin, _)) => origins.push((name, origin)),
                None => missing.push(name),
            }
        }
        if !missing.is_empty() {
            return Err(missing);
        }
        let write = shown.map_strings(|text| {
            substitute(text, |name| match secrets.lookup(name) {
                Some((Origin::Env, _)) if expands_refs => None,
                Some((_, value)) => Some(value),
                None => None,
            })
        });
        let hash = state_hash(&shown, &origins);
        Ok(McpWant {
            shown,
            write,
            hash,
            sha,
        })
    }

    pub fn tracked(&self) -> Tracked {
        Tracked {
            hash: self.hash.clone(),
            mode: None,
            sha: self.sha.clone(),
        }
    }

    pub fn want(&self) -> Want {
        Want {
            key: state_key(&self.hash, self.sha.as_deref()),
            write: self.write.hash(),
        }
    }
}

/// Hint for a missing secret.
pub fn missing_hint(name: &str) -> String {
    format!("segredo {name} ausente; defina com `agsync secret set {name}` ou exporte {name}")
}

fn state_hash(shown: &McpSpec, origins: &[(String, Origin)]) -> String {
    if origins.is_empty() {
        return shown.hash();
    }
    let mut text = shown.to_json();
    for (name, origin) in origins {
        text.push_str(&format!("\n{name}={}", origin.label()));
    }
    hash_bytes(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const ENV_VAR: &str = "AGSYNC_TEST_DELIVER_ENV";

    fn spec() -> McpSpec {
        McpSpec::Stdio {
            command: "run".into(),
            args: vec![format!("--a=${{{ENV_VAR}}}"), "--b=${FILE_ONLY}".into()],
            env: BTreeMap::from([("T".to_string(), "${FILE_ONLY}".to_string())]),
        }
    }

    fn secrets(value: &str) -> Secrets {
        let file = BTreeMap::from([("FILE_ONLY".to_string(), value.to_string())]);
        Secrets::from_file_values(file)
    }

    #[test]
    fn env_refs_stay_for_expanding_agents_and_file_values_are_written() {
        unsafe { std::env::set_var(ENV_VAR, "env-value") };
        let want = McpWant::new(spec(), &secrets("file-value"), true, None).unwrap();
        let json = want.write.to_json();
        assert!(json.contains(&format!("--a=${{{ENV_VAR}}}")), "{json}");
        assert!(json.contains("--b=file-value"), "{json}");
        assert!(json.contains(r#""T":"file-value""#), "{json}");

        let plain = McpWant::new(spec(), &secrets("file-value"), false, None).unwrap();
        assert!(plain.write.to_json().contains("--a=env-value"));

        let rotated = McpWant::new(spec(), &secrets("other-value"), true, None).unwrap();
        assert_eq!(rotated.hash, want.hash);
        assert_ne!(rotated.want().write, want.want().write);
        assert!(!want.hash.contains("file-value"));
        unsafe { std::env::remove_var(ENV_VAR) };
    }

    #[test]
    fn missing_secrets_are_listed() {
        let missing = McpWant::new(spec(), &Secrets::default(), true, None).unwrap_err();
        assert!(missing.contains(&"FILE_ONLY".to_string()));
        assert!(missing_hint("X").contains("agsync secret set X"));
    }

    #[test]
    fn no_refs_keeps_plain_hash_and_sha_goes_in_key() {
        let plain = McpSpec::Http {
            url: "https://x".into(),
            headers: BTreeMap::new(),
        };
        let want = McpWant::new(plain.clone(), &Secrets::default(), true, Some("abc".into()));
        let want = want.unwrap();
        assert_eq!(want.hash, plain.hash());
        assert_eq!(want.want().key, format!("{}@abc", plain.hash()));
    }
}
