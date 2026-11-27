//! Turning an MCP entry with `${VAR}` references into what one agent gets.

use crate::hash::hash_bytes;
use crate::mcp::{McpSpec, RefSpot};
use crate::plan::Want;
use crate::secret::{Origin, Secrets, substitute};
use crate::state::{Tracked, state_key};

/// The agent's answer to "may a `${VAR}` at this spot stay a reference?"
pub type KeepsRef<'a> = &'a dyn Fn(RefSpot<'_>) -> bool;

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
    /// at the spots `keeps_ref` accepts; otherwise its value is written.
    /// Returns the missing variable names when any is undefined.
    pub fn new(
        shown: McpSpec,
        secrets: &Secrets,
        keeps_ref: KeepsRef,
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
        let write = written(&shown, secrets, keeps_ref);
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
            plugin: None,
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

/// The entry with each `${VAR}` either kept (user environment, at a spot the agent
/// accepts a reference) or resolved to its value.
fn written(shown: &McpSpec, secrets: &Secrets, keeps_ref: KeepsRef) -> McpSpec {
    let values = |text: &str| substitute(text, |name| secrets.lookup(name).map(|(_, v)| v));
    let refs = |text: &str| {
        substitute(text, |name| match secrets.lookup(name) {
            Some((Origin::Env, _)) => None,
            other => other.map(|(_, v)| v),
        })
    };
    shown.map_spots(|spot| match keeps_ref(spot) {
        true => refs(spot.text()),
        false => values(spot.text()),
    })
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
    use crate::agent::codex;

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
        let want = McpWant::new(spec(), &secrets("file-value"), &|_| true, None).unwrap();
        let json = want.write.to_json();
        assert!(json.contains(&format!("--a=${{{ENV_VAR}}}")), "{json}");
        assert!(json.contains("--b=file-value"), "{json}");
        assert!(json.contains(r#""T":"file-value""#), "{json}");

        let codex = McpWant::new(spec(), &secrets("file-value"), &codex::keeps_ref, None).unwrap();
        assert!(codex.write.to_json().contains("--a=env-value"));

        let rotated = McpWant::new(spec(), &secrets("other-value"), &|_| true, None).unwrap();
        assert_eq!(rotated.hash, want.hash);
        assert_ne!(rotated.want().write, want.want().write);
        assert!(!want.hash.contains("file-value"));
        unsafe { std::env::remove_var(ENV_VAR) };
    }

    #[test]
    fn missing_secrets_are_listed() {
        let missing = McpWant::new(spec(), &Secrets::default(), &|_| true, None).unwrap_err();
        assert!(missing.contains(&"FILE_ONLY".to_string()));
        assert!(missing_hint("X").contains("agsync secret set X"));
    }

    #[test]
    fn no_refs_keeps_plain_hash_and_sha_goes_in_key() {
        let plain = McpSpec::Http {
            url: "https://x".into(),
            headers: BTreeMap::new(),
        };
        let want = McpWant::new(
            plain.clone(),
            &Secrets::default(),
            &|_| true,
            Some("abc".into()),
        );
        let want = want.unwrap();
        assert_eq!(want.hash, plain.hash());
        assert_eq!(want.want().key, format!("{}@abc", plain.hash()));
    }

    #[test]
    fn codex_keeps_refs_only_in_its_native_spots() {
        let var = "AGSYNC_TEST_DELIVER_CODEX";
        unsafe { std::env::set_var(var, "env-value") };
        let r = format!("${{{var}}}");
        let stdio = McpSpec::Stdio {
            command: "run".into(),
            args: vec![r.clone()],
            env: BTreeMap::from([
                (var.to_string(), r.clone()),
                ("OTHER".to_string(), r.clone()),
                ("FILE".to_string(), "${FILE_ONLY}".to_string()),
            ]),
        };
        let want = McpWant::new(stdio, &secrets("file-value"), &codex::keeps_ref, None).unwrap();
        let McpSpec::Stdio { args, env, .. } = &want.write else {
            panic!("stdio expected");
        };
        assert_eq!(args, &["env-value"]);
        assert_eq!(env[var], r);
        assert_eq!(env["OTHER"], "env-value");
        assert_eq!(env["FILE"], "file-value");

        let http = McpSpec::Http {
            url: format!("https://x/{r}"),
            headers: BTreeMap::from([
                ("Authorization".to_string(), format!("Bearer {r}")),
                ("X-Key".to_string(), r.clone()),
                ("X-Mixed".to_string(), format!("k={r}")),
                ("X-File".to_string(), "${FILE_ONLY}".to_string()),
            ]),
        };
        let want = McpWant::new(http, &secrets("file-value"), &codex::keeps_ref, None).unwrap();
        let McpSpec::Http { url, headers } = &want.write else {
            panic!("http expected");
        };
        assert_eq!(url, "https://x/env-value");
        assert_eq!(headers["Authorization"], format!("Bearer {r}"));
        assert_eq!(headers["X-Key"], r);
        assert_eq!(headers["X-Mixed"], "k=env-value");
        assert_eq!(headers["X-File"], "file-value");
        unsafe { std::env::remove_var(var) };
    }
}
