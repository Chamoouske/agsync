//! Agent-neutral MCP server model.

use std::collections::BTreeMap;
use std::path::Path;

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

    /// Same entry with each command, arg, env value, url and header value replaced by
    /// `f` of its spot. Keys stay as they are.
    pub fn map_spots(&self, f: impl Fn(RefSpot<'_>) -> String) -> McpSpec {
        match self {
            McpSpec::Stdio { command, args, env } => McpSpec::Stdio {
                command: f(RefSpot::Command(command)),
                args: args.iter().map(|a| f(RefSpot::Arg(a))).collect(),
                env: env
                    .iter()
                    .map(|(key, value)| (key.clone(), f(RefSpot::Env { key, value })))
                    .collect(),
            },
            McpSpec::Http { url, headers } => McpSpec::Http {
                url: f(RefSpot::Url(url)),
                headers: headers
                    .iter()
                    .map(|(key, value)| (key.clone(), f(RefSpot::Header { key, value })))
                    .collect(),
            },
        }
    }

    /// Replaces `{dir}` in command, args and env values with the checkout folder.
    pub fn with_dir(&self, dir: &Path) -> McpSpec {
        let dir = dir.display().to_string();
        match self {
            McpSpec::Stdio { command, args, env } => McpSpec::Stdio {
                command: command.replace("{dir}", &dir),
                args: args.iter().map(|a| a.replace("{dir}", &dir)).collect(),
                env: env
                    .iter()
                    .map(|(k, v)| (k.clone(), v.replace("{dir}", &dir)))
                    .collect(),
            },
            McpSpec::Http { .. } => self.clone(),
        }
    }

    /// Names of the `${VAR}` references, in order, without repeats.
    pub fn secret_refs(&self) -> Vec<String> {
        let texts: Vec<&str> = match self {
            McpSpec::Stdio { command, args, env } => std::iter::once(command)
                .chain(args)
                .chain(env.values())
                .map(String::as_str)
                .collect(),
            McpSpec::Http { url, headers } => std::iter::once(url)
                .chain(headers.values())
                .map(String::as_str)
                .collect(),
        };
        crate::secret::refs(&texts.join("\n"))
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

/// Where a string sits in an MCP entry, with the string itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefSpot<'a> {
    Command(&'a str),
    Arg(&'a str),
    Env { key: &'a str, value: &'a str },
    Url(&'a str),
    Header { key: &'a str, value: &'a str },
}

impl RefSpot<'_> {
    /// The string at this spot.
    pub fn text(&self) -> &str {
        match self {
            RefSpot::Command(text) | RefSpot::Arg(text) | RefSpot::Url(text) => text,
            RefSpot::Env { value, .. } | RefSpot::Header { value, .. } => value,
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
    fn finds_refs_in_every_substituted_field() {
        let stdio = McpSpec::Stdio {
            command: "${A}".into(),
            args: vec!["${B}".into()],
            env: BTreeMap::from([("${NOT_A_REF_KEY}".to_string(), "${C}".to_string())]),
        };
        assert_eq!(stdio.secret_refs(), ["A", "B", "C"]);
        let http = McpSpec::Http {
            url: "https://x/${U}".into(),
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer ${H}".to_string())]),
        };
        assert_eq!(http.secret_refs(), ["U", "H"]);
    }

    #[test]
    fn dir_is_replaced_in_command_args_and_env_only() {
        let spec = McpSpec::Stdio {
            command: "{dir}/bin/srv".into(),
            args: vec!["--root={dir}".into(), "plain".into()],
            env: BTreeMap::from([("{dir}".to_string(), "{dir}/data".to_string())]),
        };
        let dir = Path::new("/abs/sources/srv/abc");
        let McpSpec::Stdio { command, args, env } = spec.with_dir(dir) else {
            panic!("stdio expected");
        };
        assert_eq!(command, "/abs/sources/srv/abc/bin/srv");
        assert_eq!(args, ["--root=/abs/sources/srv/abc", "plain"]);
        assert_eq!(env["{dir}"], "/abs/sources/srv/abc/data");
        let http = McpSpec::Http {
            url: "https://x/{dir}".into(),
            headers: BTreeMap::new(),
        };
        assert_eq!(http.with_dir(dir), http);
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
