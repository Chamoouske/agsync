//! Agent-neutral MCP server model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::Mcp;

/// An MCP server as agents see it. Serializes to the Claude Code JSON shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpSpec {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl McpSpec {
    /// Builds the spec from a validated manifest entry.
    pub fn from_manifest(mcp: &Mcp) -> McpSpec {
        match (&mcp.command, &mcp.url) {
            (Some(command), _) => McpSpec::Stdio {
                command: command.clone(),
                args: mcp.args.clone().unwrap_or_default(),
                env: mcp.env.clone().unwrap_or_default(),
            },
            (None, url) => McpSpec::Http {
                url: url.clone().unwrap_or_default(),
                headers: mcp.headers.clone().unwrap_or_default(),
            },
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Hash of the canonical JSON form.
    pub fn hash(&self) -> String {
        crate::hash::hash_bytes(self.to_json().as_bytes())
    }

    /// Human summary that shows env and header keys, never their values.
    pub fn describe(&self) -> String {
        match self {
            McpSpec::Stdio { command, args, env } => {
                let mut text = format!("stdio: {command}");
                for arg in args {
                    text.push(' ');
                    text.push_str(arg);
                }
                if !env.is_empty() {
                    text.push_str(&format!("; env: {}", keys(env)));
                }
                text
            }
            McpSpec::Http { url, headers } => {
                let mut text = format!("http: {url}");
                if !headers.is_empty() {
                    text.push_str(&format!("; headers: {}", keys(headers)));
                }
                text
            }
        }
    }
}

/// Hash of an entry read from an agent config; unknown shapes hash their raw JSON.
pub fn hash_value(value: &serde_json::Value) -> String {
    match serde_json::from_value::<McpSpec>(value.clone()) {
        Ok(spec) => spec.hash(),
        Err(_) => crate::hash::hash_bytes(value.to_string().as_bytes()),
    }
}

fn keys(map: &BTreeMap<String, String>) -> String {
    map.keys().cloned().collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    const YAML: &str = r#"
version: 1
mcps:
  local:
    command: npx
    args: ["-y", "pkg"]
    env: {TOKEN: secret-value}
  remote:
    url: https://example.com/mcp
    headers: {Authorization: secret-header}
"#;

    #[test]
    fn json_matches_claude_shape() {
        let m = Manifest::parse(YAML).unwrap();
        let local = McpSpec::from_manifest(&m.mcps["local"].value);
        assert_eq!(
            local.to_json(),
            r#"{"type":"stdio","command":"npx","args":["-y","pkg"],"env":{"TOKEN":"secret-value"}}"#
        );
        let remote = McpSpec::from_manifest(&m.mcps["remote"].value);
        assert_eq!(
            remote.to_json(),
            r#"{"type":"http","url":"https://example.com/mcp","headers":{"Authorization":"secret-header"}}"#
        );
    }

    #[test]
    fn describe_hides_values() {
        let m = Manifest::parse(YAML).unwrap();
        let local = McpSpec::from_manifest(&m.mcps["local"].value).describe();
        assert_eq!(local, "stdio: npx -y pkg; env: TOKEN");
        let remote = McpSpec::from_manifest(&m.mcps["remote"].value).describe();
        assert!(!remote.contains("secret"));
        assert!(remote.contains("Authorization"));
    }

    #[test]
    fn read_hash_ignores_defaults_and_extra_fields() {
        let spec = McpSpec::Stdio {
            command: "x".into(),
            args: vec![],
            env: BTreeMap::new(),
        };
        let stored = serde_json::json!({"type": "stdio", "command": "x", "extra": 1});
        assert_eq!(hash_value(&stored), spec.hash());
        let odd = serde_json::json!({"type": "sse", "url": "u"});
        assert_ne!(hash_value(&odd), spec.hash());
    }
}
